pub const SECTOR_SIZE: usize = 512;

pub trait BlockDevice {
    fn read_sector(&mut self, sector: usize, buf: &mut [u8; SECTOR_SIZE]);

    fn write_sector(&mut self, sector: usize, buf: &[u8; SECTOR_SIZE]);
}
