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
    total_sectors: usize,
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
        let total_sectors = le16(&boot, 19);
        let sectors_per_fat = le16(&boot, 22);

        Self {
            device,
            bytes_per_sector,
            sectors_per_cluster,
            reserved_sectors,
            fat_count,
            root_entry_count,
            sectors_per_fat,
            total_sectors,
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

    fn cluster_count(&self) -> usize {
        let data_sectors = self.total_sectors - self.first_data_sector();
        data_sectors / self.sectors_per_cluster
    }

    // 从 fat 表中两个共享字节中取出自己的 12 bit
    fn fat_entry(&mut self, cluster: usize) -> u16 {
        let fat_offset = cluster + cluster / 2;

        let sector = self.reserved_sectors + fat_offset / SECTOR_SIZE;
        let offset = fat_offset % SECTOR_SIZE;

        let mut buf = [0u8; SECTOR_SIZE];
        self.device.read_sector(sector, &mut buf);

        let first = buf[offset];
        let second = if offset + 1 < SECTOR_SIZE {
            buf[offset + 1]
        } else {
            let mut next = [0u8; SECTOR_SIZE];
            self.device.read_sector(sector + 1, &mut next);
            next[0]
        };
        if cluster % 2 == 0 {
            (first as u16) | ((second as u16 & 0x0F) << 8)
        } else {
            ((first as u16) >> 4) | ((second as u16) << 4)
        }
    }

    // 修改自己的 12 bit
    fn set_fat_entry(&mut self, cluster: usize, value: u16) {
        let value = value & 0x0FFF;

        let fat_offset = cluster + cluster / 2;

        for fat in 0..self.fat_count {
            let fat_start = self.reserved_sectors + fat * self.sectors_per_fat;
            let sector = fat_start + fat_offset / SECTOR_SIZE;
            let offset = fat_offset % SECTOR_SIZE;

            let mut buf = [0u8; SECTOR_SIZE];
            self.device.read_sector(sector, &mut buf);

            if offset + 1 < SECTOR_SIZE {
                if cluster % 2 == 0 {
                    buf[offset] = value as u8;
                    buf[offset + 1] = (buf[offset + 1] & 0xF0) | ((value >> 8) as u8 & 0x0F);
                } else {
                    buf[offset] = (buf[offset] & 0x0F) | ((value << 4) as u8 & 0xF0);
                    buf[offset + 1] = (value >> 4) as u8;
                }

                self.device.write_sector(sector, &buf);
            } else {
                let mut next = [0u8; SECTOR_SIZE];
                self.device.read_sector(sector + 1, &mut next);

                if cluster % 2 == 0 {
                    buf[offset] = value as u8;
                    next[0] = (next[0] & 0xF0) | ((value >> 8) as u8 & 0x0F);
                } else {
                    buf[offset] = (buf[offset] & 0x0F) | ((value << 4) as u8 & 0xF0);
                    next[0] = (value >> 4) as u8;
                }
                self.device.write_sector(sector, &buf);
                self.device.write_sector(sector + 1, &next);
            }
        }
    }

    fn find_free_cluster(&mut self) -> Option<u16> {
        let max_cluster = self.cluster_count() + 1; // cluster 从 2 开始
        for cluster in 2..=max_cluster {
            if self.fat_entry(cluster) == 0 {
                return Some(cluster as u16);
            }
        }
        None
    }

    pub fn write_bytes(&mut self, name: &[u8; 11], data: &[u8]) -> bool {
        let (dir_sector, entry_offset) = match self.find_file(name) {
            Some(location) => location,
            None => return false,
        };

        if data.len() > SECTOR_SIZE {
            return false;
        }

        let cluster = match self.find_free_cluster() {
            Some(c) => c,
            None => return false,
        };
        let mut buf = [0u8; SECTOR_SIZE];

        self.set_fat_entry(cluster as usize, 0xFFF);

        // 找空闲 cluster
        let data_sector =
            self.first_data_sector() + (cluster as usize - 2) * self.sectors_per_cluster;
        buf.fill(0);
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

    pub fn read_bytes(&mut self, name: &[u8; 11], out: &mut [u8]) -> Option<usize> {
        let (dir_sector, entry_offset) = self.find_file(name)?;

        let mut buf = [0u8; SECTOR_SIZE];
        self.device.read_sector(dir_sector, &mut buf);

        let cluster = u16::from_le_bytes([buf[entry_offset + 26], buf[entry_offset + 27]]) as usize;

        let size = u32::from_le_bytes([
            buf[entry_offset + 28],
            buf[entry_offset + 29],
            buf[entry_offset + 30],
            buf[entry_offset + 31],
        ]) as usize;

        if cluster < 2 {
            return None;
        }

        if size > SECTOR_SIZE {
            return None;
        }
        // 当前只支持读取一个扇区的数据
        if size > out.len() {
            return None;
        }

        let data_sector = self.first_data_sector() + (cluster - 2) * self.sectors_per_cluster;

        self.device.read_sector(data_sector, &mut buf);
        out[..size].copy_from_slice(&buf[..size]);

        Some(size)
    }
}
