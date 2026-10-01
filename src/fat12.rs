fn le16(data: &[u8], pos: usize) -> usize {
    u16::from_le_bytes([data[pos], data[pos + 1]]) as usize
}

pub struct Fat12<'a> {
    image: &'a mut [u8],

    bytes_per_sector: usize,
    sectors_per_cluster: usize,
    reserved_sectors: usize,
    fat_count: usize,
    root_entry_count: usize,
    sectors_per_fat: usize,
}

impl<'a> Fat12<'a> {
    pub fn new(image: &'a mut [u8]) -> Self {
        let bytes_per_sector = le16(image, 11);
        let sectors_per_cluster = image[13] as usize;
        let reserved_sectors = le16(image, 14);
        let fat_count = image[16] as usize;
        let root_entry_count = le16(image, 17);
        let sectors_per_fat = le16(image, 22);

        Self {
            image,
            bytes_per_sector,
            sectors_per_cluster,
            reserved_sectors,
            fat_count,
            root_entry_count,
            sectors_per_fat,
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

    fn find_file(&self, name: &[u8; 11]) -> Option<usize> {
        let root = self.root_offset();
        for i in 0..self.root_entry_count {
            let offset = root + i * 32;
            if self.image[offset] == 0x00 {
                break;
            }
            if &self.image[offset..offset + 11] == name {
                return Some(offset);
            }
        }
        None
    }

    pub fn create_file(&mut self, name: &[u8; 11]) -> bool {
        let root = self.root_offset();

        for i in 0..self.root_entry_count {
            let offset = root + i * 32;
            let first = self.image[offset];

            if first != 0x00 && first != 0xE5 {
                continue;
            }

            let entry = &mut self.image[offset..offset + 32];
            entry.fill(0);
            entry[0..11].copy_from_slice(name);
            entry[11] = 0x20; // 属性
            return true;
        }
        false
    }

    pub fn write_bytes(&mut self, name: &[u8; 11], data: &[u8]) -> bool {
        let entry = match self.find_file(name) {
            Some(offset) => offset,
            None => return false,
        };

        let cluster_size = self.bytes_per_sector * self.sectors_per_cluster;

        if data.len() > cluster_size {
            return false;
        }

        // 当前阶段固定使用 cluster 2
        let cluster = 2u16;

        // FAT[2] = EOF
        for fat in 0..self.fat_count {
            let fat_start =
                (self.reserved_sectors + fat * self.sectors_per_fat) * self.bytes_per_sector;

            self.image[fat_start + 3] = 0xFF;

            self.image[fat_start + 4] = (self.image[fat_start + 4] & 0xF0) | 0x0F;
        }

        // 找到 Data Area
        let root_sectors =
            (self.root_entry_count * 32 + self.bytes_per_sector - 1) / self.bytes_per_sector;
        let first_data_sector =
            self.reserved_sectors + self.fat_count * self.sectors_per_fat + root_sectors;
        let data_offset = first_data_sector * self.bytes_per_sector;

        // 写数据
        self.image[data_offset..data_offset + data.len()].copy_from_slice(data);
        // first_cluster = 2
        self.image[entry + 26..entry + 28].copy_from_slice(&cluster.to_le_bytes());
        // file_size = data.len()
        self.image[entry + 28..entry + 32].copy_from_slice(&(data.len() as u32).to_le_bytes());

        true
    }
}
