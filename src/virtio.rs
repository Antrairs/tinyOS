use core::ptr::NonNull;

use crate::block::{BlockDevice, SECTOR_SIZE};

use virtio_drivers::{
    BufferDirection, Hal, PAGE_SIZE, PhysAddr,
    device::blk::VirtIOBlk,
    transport::{
        DeviceType, Transport,
        mmio::{MmioTransport, VirtIOHeader},
    },
};

use core::ptr::read_volatile;

const VIRTIO_BASE: usize = 0x1000_1000;
const VIRTIO_SIZE: usize = 0x1000;
const VIRTIO_COUNT: usize = 8;

const MAGIC_VALUE: usize = 0x000;
const VERSION: usize = 0x004;
const DEVICE_ID: usize = 0x008;

pub struct TinyHal;

const DMA_PAGES: usize = 16;

#[repr(align(4096))]
struct DmaPool([u8; DMA_PAGES * PAGE_SIZE]);

static mut DMA_POOL: DmaPool = DmaPool([0; DMA_PAGES * PAGE_SIZE]);

static mut DMA_NEXT: usize = 0;

unsafe impl Hal for TinyHal {
    fn dma_alloc(pages: usize, _direction: BufferDirection) -> (PhysAddr, NonNull<u8>) {
        let bytes = pages * PAGE_SIZE;

        unsafe {
            if DMA_NEXT + bytes > DMA_PAGES * PAGE_SIZE {
                return (0, NonNull::dangling());
            }

            let base = core::ptr::addr_of_mut!(DMA_POOL.0) as *mut u8;

            let ptr = base.add(DMA_NEXT);

            DMA_NEXT += bytes;

            core::ptr::write_bytes(ptr, 0, bytes);

            (ptr as usize as u64, NonNull::new_unchecked(ptr))
        }
    }

    unsafe fn dma_dealloc(_paddr: PhysAddr, _vaddr: NonNull<u8>, _pages: usize) -> i32 {
        0
    }

    unsafe fn mmio_phys_to_virt(paddr: PhysAddr, _size: usize) -> NonNull<u8> {
        unsafe { NonNull::new_unchecked(paddr as usize as *mut u8) }
    }

    unsafe fn share(buffer: NonNull<[u8]>, _direction: BufferDirection) -> PhysAddr {
        buffer.as_ptr() as *mut u8 as usize as u64
    }

    unsafe fn unshare(_paddr: PhysAddr, _buffer: NonNull<[u8]>, _direction: BufferDirection) {}
}

pub struct VirtioBlock {
    inner: VirtIOBlk<TinyHal, MmioTransport<'static>>,
}

impl VirtioBlock {
    pub fn new(base: usize) -> Option<Self> {
        let header = NonNull::new(base as *mut VirtIOHeader)?;

        let transport = unsafe {
            MmioTransport::new(header, VIRTIO_SIZE).ok()?
        };

        if transport.device_type() != DeviceType::Block {
            return None;
        }

        let inner = VirtIOBlk::<TinyHal, _>::new(transport).ok()?;

        Some(Self { inner })
    }
}

impl BlockDevice for VirtioBlock {
    fn read_sector(&mut self, sector: usize, buf: &mut [u8; SECTOR_SIZE]) {
        self.inner.read_blocks(sector, buf).expect("VirtIO read failed");
    }

    fn write_sector(&mut self, sector: usize, buf: &[u8; SECTOR_SIZE]) {
        self.inner.write_blocks(sector, buf).expect("VirtIO write failed");
    }
}

fn read_reg(base: usize, offset: usize) -> u32 {
    unsafe { read_volatile((base + offset) as *const u32) }
}

fn is_block_device(base: usize) -> bool {
    let magic = read_reg(base, MAGIC_VALUE);
    let version = read_reg(base, VERSION);
    let device_id = read_reg(base, DEVICE_ID);

    magic == 0x7472_6976 && version == 2 && device_id == 2
}

pub fn find_block_device() -> Option<usize> {
    for i in 0..VIRTIO_COUNT {
        let base = VIRTIO_BASE + i * VIRTIO_SIZE;

        if is_block_device(base) {
            return Some(base);
        }
    }
    None
}
