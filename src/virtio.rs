use core::ptr::read_volatile;

const VIRTIO_BASE: usize = 0x1000_1000;
const VIRTIO_SIZE: usize = 0x1000;
const VIRTIO_COUNT: usize = 8;

const MAGIC_VALUE: usize = 0x000;
const VERSION: usize = 0x004;
const DEVICE_ID: usize = 0x008;

fn read_reg(base: usize, offset: usize) -> u32 {
    unsafe {
        read_volatile((base + offset) as *const u32)
    }
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
