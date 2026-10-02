pub const SECTOR_SIZE: usize = 512;

pub trait BlockDevice {
    fn read_sector(&mut self, sector: usize, buf: &mut [u8; SECTOR_SIZE]);

    fn write_sector(&mut self, sector: usize, buf: &[u8; SECTOR_SIZE]);
}

pub struct MemoryDisk<'a> {
    data: &'a mut [u8],
}

impl<'a> MemoryDisk<'a> {
    pub fn new(data: &'a mut [u8]) -> Self {
        Self { data }
    }
}

impl BlockDevice for MemoryDisk<'_> {
    fn read_sector(&mut self, sector: usize, buf: &mut [u8; SECTOR_SIZE]) {
        let start = sector * SECTOR_SIZE;
        let end = start + SECTOR_SIZE;
        buf.copy_from_slice(&self.data[start..end]);
    }

    fn write_sector(&mut self, sector: usize, buf: &[u8; SECTOR_SIZE]) {
        let start = sector * SECTOR_SIZE;
        let end = start + SECTOR_SIZE;
        self.data[start..end].copy_from_slice(buf);
    }
}