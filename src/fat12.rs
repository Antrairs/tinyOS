use crate::block::{BlockDevice, SECTOR_SIZE};

fn le16(data: &[u8], pos: usize) -> usize {
    u16::from_le_bytes([data[pos], data[pos + 1]]) as usize
}

pub struct Fat12<'a> {
    device: &'a mut dyn BlockDevice,

    bytes_per_sector: usize,
    sectors_per_cluster: usize,
    reserved_sectors: usize,
    fat_count: usize,
    root_entry_count: usize,
    sectors_per_fat: usize,
}

impl<'a> Fat12<'a> {
    pub fn new(device: &'a mut dyn BlockDevice) -> Self {
        let mut boot = [0u8; SECTOR_SIZE];

        device.read_sector(0, &mut boot);

        let bytes_per_sector = le16(&boot, 11);
        let sectors_per_cluster = boot[13] as usize;
        let reserved_sectors = le16(&boot, 14);
        let fat_count = boot[16] as usize;
        let root_entry_count = le16(&boot, 17);
        let sectors_per_fat = le16(&boot, 22);

        Self {
            device,
            bytes_per_sector,
            sectors_per_cluster,
            reserved_sectors,
            fat_count,
            root_entry_count,
            sectors_per_fat,
        }
    }

    fn root_sector(&self) -> usize {
        self.reserved_sectors + self.fat_count * self.sectors_per_fat
    }

    pub fn list_root_name<F>(&mut self, mut f: F)
    where
        F: FnMut(&[u8]),
    {
        let root = self.root_sector();

        let root_sectors = (self.root_entry_count * 32 + SECTOR_SIZE - 1) / SECTOR_SIZE;

        let mut buf = [0u8; SECTOR_SIZE];

        for sector in 0..root_sectors {
            self.device.read_sector(root + sector, &mut buf);

            for i in 0..16 {
                let offset = i * 32;
                let entry = &buf[offset..offset + 32];

                if entry[0] == 0x00 {
                    return;
                }

                if entry[0] == 0xE5 {
                    continue;
                }

                f(&entry[0..11]);
            }
        }
    }

    pub fn create_file(&mut self, name: &[u8; 11]) -> bool {
        let root = self.root_sector();

        let root_sectors = (self.root_entry_count * 32 + SECTOR_SIZE - 1) / SECTOR_SIZE;

        let mut buf = [0u8; SECTOR_SIZE];

        for sector in 0..root_sectors {
            self.device.read_sector(root + sector, &mut buf);

            // 一个 sector 有 16 个目录项 512 / 32 = 16
            for i in 0..16 {
                let offset = i * 32;
                let first = buf[offset];

                if first != 0x00 && first != 0xE5 {
                    continue;
                }

                let enrty = &mut buf[offset..offset + 32];

                enrty.fill(0);
                enrty[0..11].copy_from_slice(name);
                enrty[11] = 0x20; // 属性

                self.device.write_sector(root + sector, &buf);
                return true;
            }
        }
        false
    }

    fn find_file(&mut self, name: &[u8; 11]) -> Option<(usize, usize)> {
        let root = self.root_sector();

        let root_sectors = (self.root_entry_count * 32 + SECTOR_SIZE - 1) / SECTOR_SIZE;

        let mut buf = [0u8; SECTOR_SIZE];

        for sector in 0..root_sectors {
            let disk_sector = root + sector;
            self.device.read_sector(disk_sector, &mut buf);
            for i in 0..16 {
                let offset = i * 32;

                if buf[offset] == 0x00 {
                    return None;
                }

                if &buf[offset..offset + 11] == name {
                    return Some((disk_sector, offset));
                }
            }
        }
        None
    }

    fn first_data_sector(&self) -> usize {
        let root_sectors =
            (self.root_entry_count * 32 + self.bytes_per_sector - 1) / self.bytes_per_sector;
        self.reserved_sectors + self.fat_count * self.sectors_per_fat + root_sectors
    }

    pub fn write_bytes(&mut self, name: &[u8; 11], data: &[u8]) -> bool {
        let (dir_sector, entry_offset) = match self.find_file(name) {
            Some(location) => location,
            None => return false,
        };

        if data.len() > SECTOR_SIZE {
            return false;
        }

        let cluster = 2u16;
        let mut buf = [0u8; SECTOR_SIZE];

        for fat in 0..self.fat_count {
            let fat_sector = self.reserved_sectors + fat * self.sectors_per_fat;

            self.device.read_sector(fat_sector, &mut buf);
            buf[3] = 0xFF;
            buf[4] = (buf[4] & 0xF0) | 0x0F;
            self.device.write_sector(fat_sector, &buf);
        }

        // 写 cluster 2
        let data_sector = self.first_data_sector();
        self.device.read_sector(data_sector, &mut buf);
        buf[..data.len()].copy_from_slice(data);
        self.device.write_sector(data_sector, &buf);

        // 更新目录项
        self.device.read_sector(dir_sector, &mut buf);
        buf[entry_offset + 26..entry_offset + 28].copy_from_slice(&cluster.to_le_bytes());
        buf[entry_offset + 28..entry_offset + 32]
            .copy_from_slice(&(data.len() as u32).to_le_bytes());
        self.device.write_sector(dir_sector, &buf);

        true
    }

    // pub fn read_bytes(&self, name: &[u8; 11]) -> Option<&[u8]> {
    //     let entry = match self.find_file(name) {
    //         Some(offset) => offset,
    //         None => return None,
    //     };

    //     // 读取 first_cluster
    //     let cluster = u16::from_le_bytes([self.image[entry + 26], self.image[entry + 27]]) as usize;

    //     // 读取 file_size
    //     let size = u32::from_le_bytes([
    //         self.image[entry + 28],
    //         self.image[entry + 29],
    //         self.image[entry + 30],
    //         self.image[entry + 31],
    //     ]) as usize;

    //     if cluster < 2 {
    //         return None;
    //     }

    //     let cluster_size = self.bytes_per_sector * self.sectors_per_cluster;

    //     if size > cluster_size {
    //         return None;
    //     }

    //     let root_sectors =
    //         (self.root_entry_count * 32 + self.bytes_per_sector - 1) / self.bytes_per_sector;

    //     let first_data_sector =
    //         self.reserved_sectors + self.fat_count * self.sectors_per_fat + root_sectors;

    //     let data_sector = first_data_sector + (cluster - 2) * self.sectors_per_cluster;

    //     let data_offset = data_sector * self.bytes_per_sector;

    //     if data_offset + size > self.image.len() {
    //         return None;
    //     }

    //     Some(&self.image[data_offset..data_offset + size])
    // }
}
