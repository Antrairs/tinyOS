fn le16(data: &[u8], pos: usize) -> usize {
    u16::from_le_bytes([data[pos], data[pos + 1]]) as usize
}

pub struct Fat12<'a> {
    image: &'a [u8],

    bytes_per_sector: usize,
    reserved_sectors: usize,
    fat_count: usize,
    root_entry_count: usize,
    sectors_per_fat: usize,
}

impl<'a> Fat12<'a> {
    pub fn new(image: &'a [u8]) -> Self {
        Self {
            image,

            bytes_per_sector: le16(image, 11),
            reserved_sectors: le16(image, 14),
            fat_count: image[16] as usize,
            root_entry_count: le16(image, 17),
            sectors_per_fat: le16(image, 22),
        }
    }

    fn root_offset(&self) -> usize {
        let root_sector = self.reserved_sectors + self.fat_count * self.sectors_per_fat;

        root_sector * self.bytes_per_sector
    }

    pub fn list_root_name<F>(&self, mut f: F)
    where
        F: FnMut(&[u8]),
    {
        let root = self.root_offset();

        for i in 0..self.root_entry_count {
            let offset = root + i * 32;
            let entry = &self.image[offset..offset + 32];

            // 后面没有目录项了
            if entry[0] == 0x00 {
                break;
            }

            // 已删除
            if entry[0] == 0xE5 {
                continue;
            }

            f(&entry[0..11]);
        }
    }
}
