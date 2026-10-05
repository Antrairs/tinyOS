use crate::block::{BlockDevice, SECTOR_SIZE};

fn le16(data: &[u8], pos: usize) -> usize {
    u16::from_le_bytes([data[pos], data[pos + 1]]) as usize
}

pub fn to_83(input: &[u8]) -> Option<[u8; 11]> {
    let mut name = [b' '; 11];

    let mut base_len = 0usize;
    let mut ext_len = 0usize;
    let mut in_ext = false;

    for &c in input {
        if c == b'.' {
            if in_ext {
                return None;
            }
            in_ext = true;
            continue;
        }
        let c = if c >= b'a' && c <= b'z' { c - 32 } else { c };
        if !in_ext {
            if base_len >= 8 {
                return None;
            }
            name[base_len] = c;
            base_len += 1;
        } else {
            if ext_len >= 3 {
                return None;
            }
            name[8 + ext_len] = c;
            ext_len += 1;
        }
    }

    if base_len == 0 {
        return None;
    }

    Some(name)
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

    pub fn format(&mut self) {
        let zero = [0u8; SECTOR_SIZE];
        let mut boot = [0u8; SECTOR_SIZE];

        // Boot Sector + BPB
        boot[11..13].copy_from_slice(&512u16.to_le_bytes()); // 每扇区字节数 bytes_per_sector
        boot[13] = 1; // 每簇扇区数 sectors_per_cluster
        boot[14..16].copy_from_slice(&1u16.to_le_bytes()); // 保留扇区数 reserved_sectors
        boot[16] = 2; // FAT 副本数 fat_count
        boot[17..19].copy_from_slice(&224u16.to_le_bytes()); // 根目录入口数 root_entry_count
        boot[19..21].copy_from_slice(&2880u16.to_le_bytes()); // 总扇区数 total_sectors
        boot[21] = 0xF0; // 介质描述符 media type
        boot[22..24].copy_from_slice(&9u16.to_le_bytes()); // 每FAT扇区数 sectors_per_fat

        boot[38] = 0x29; // 扩展引导记录签名
        boot[43..54].copy_from_slice(b"NO NAME    ");

        // 引导扇区签名
        boot[510] = 0x55;
        boot[511] = 0xAA;
        self.device.write_sector(0, &boot);

        // 初始化 FAT 表
        for i in 1..=18 {
            self.device.write_sector(i, &zero);
        }

        // FAT12 前两个 cluster 是保留项
        let mut fat = [0u8; SECTOR_SIZE];
        fat[0] = 0xF0;
        fat[1] = 0xFF;
        fat[2] = 0xFF;
        self.device.write_sector(1, &fat);
        self.device.write_sector(10, &fat);

        for sector in 19..33 {
            self.device.write_sector(sector, &zero);
        }
    }

    fn root_sector(&self) -> usize {
        self.reserved_sectors + self.fat_count * self.sectors_per_fat
    }

    fn dir_sectors(&self, dir_cluster: u16) -> (usize, usize) {
        if dir_cluster == 0 {
            // 根目录
            let root = self.root_sector();
            let count = (self.root_entry_count * 32 + SECTOR_SIZE - 1) / SECTOR_SIZE;

            (root, count)
        } else {
            // 非根目录
            (self.cluster_to_sector(dir_cluster), 1)
        }
    }

    pub fn list_dir<F>(&mut self, dir_cluster: u16, mut f: F)
    where
        F: FnMut(&[u8]),
    {
        let (start_sector, sector_count) = self.dir_sectors(dir_cluster);

        let mut buf = [0u8; SECTOR_SIZE];

        for sector in 0..sector_count {
            self.device.read_sector(start_sector + sector, &mut buf);

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

    pub fn mkdir(&mut self, parent_cluster: u16, name: &[u8; 11]) -> bool {
        if self.sectors_per_cluster != 1 {
            return false;
        }

        // 不允许重名
        if self.find_file(parent_cluster, name).is_some() {
            return false;
        }

        // 保证根目录有空位置
        let (start_sector, sector_count) = self.dir_sectors(parent_cluster);

        let mut buf = [0u8; SECTOR_SIZE];
        let mut free_entry = None;

        for i in 0..sector_count {
            let sector = start_sector + i;
            self.device.read_sector(sector, &mut buf);

            for j in 0..16 {
                let offset = j * 32;
                if buf[offset] == 0x00 || buf[offset] == 0xE5 {
                    free_entry = Some((sector, offset));
                    break;
                }
            }
            if free_entry.is_some() {
                break;
            }
        }

        let (dir_sector, entry_offset) = match free_entry {
            Some(location) => location,
            None => return false,
        };

        // 给新目录分配 cluster
        let cluster = match self.find_free_cluster() {
            Some(cluster) => cluster,
            None => return false,
        };

        // 当前目录只占一个 cluster
        self.set_fat_entry(cluster as usize, 0xFFF);

        // 初始化目录内容
        let mut buf = [0u8; SECTOR_SIZE];
        buf[0..11].copy_from_slice(b".          ");
        buf[11] = 0x10;
        buf[26..28].copy_from_slice(&cluster.to_le_bytes());

        let parent_offset = 32;
        buf[parent_offset..parent_offset + 11].copy_from_slice(b"..         ");
        buf[parent_offset + 11] = 0x10;
        buf[parent_offset + 26..parent_offset + 28].copy_from_slice(&parent_cluster.to_le_bytes());

        let sector = self.cluster_to_sector(cluster);
        self.device.write_sector(sector, &buf);

        self.device.read_sector(dir_sector, &mut buf);
        let entry = &mut buf[entry_offset..entry_offset + 32];
        entry.fill(0);
        entry[0..11].copy_from_slice(name);
        entry[11] = 0x10;
        entry[26..28].copy_from_slice(&cluster.to_le_bytes());
        self.device.write_sector(dir_sector, &buf);

        true
    }

    pub fn create_file(&mut self, dir_cluster: u16, name: &[u8; 11]) -> bool {
        if self.find_file(dir_cluster, name).is_some() {
            return false;
        }

        let (sector_sector, sector_count) = self.dir_sectors(dir_cluster);

        let mut buf = [0u8; SECTOR_SIZE];

        for sector in 0..sector_count {
            let disk_sector = sector_sector + sector;

            self.device.read_sector(disk_sector, &mut buf);

            // 一个 sector 有 16 个目录项 512 / 32 = 16
            for i in 0..16 {
                let offset = i * 32;

                if buf[offset] != 0x00 && buf[offset] != 0xE5 {
                    continue;
                }

                let entry = &mut buf[offset..offset + 32];

                entry.fill(0);
                entry[0..11].copy_from_slice(name);
                entry[11] = 0x20; // 属性

                self.device.write_sector(disk_sector, &buf);
                return true;
            }
        }
        false
    }

    fn find_file(&mut self, dir_cluster: u16, name: &[u8; 11]) -> Option<(usize, usize)> {
        let (start_sector, sector_count) = self.dir_sectors(dir_cluster);

        let mut buf = [0u8; SECTOR_SIZE];

        for sector in 0..sector_count {
            let disk_sector = start_sector + sector;
            self.device.read_sector(disk_sector, &mut buf);
            for i in 0..16 {
                let offset = i * 32;

                if buf[offset] == 0x00 {
                    return None;
                }

                if buf[offset] == 0xE5 {
                    continue;
                }

                if &buf[offset..offset + 11] == name {
                    return Some((disk_sector, offset));
                }
            }
        }
        None
    }

    pub fn find_dir(&mut self, dir_cluster: u16, name: &[u8; 11]) -> Option<u16> {
        let (sector, offset) = self.find_file(dir_cluster, name)?;

        let mut buf = [0u8; SECTOR_SIZE];
        self.device.read_sector(sector, &mut buf);

        if buf[offset + 11] & 0x10 == 0 {
            return None;
        }

        Some(u16::from_le_bytes([buf[offset + 26], buf[offset + 27]]))
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

    fn next_cluster(&mut self, cluster: u16) -> Option<u16> {
        let value = self.fat_entry(cluster as usize);

        if value >= 0x002 && value <= 0xFEF {
            Some(value)
        } else {
            None
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

    fn cluster_to_sector(&self, cluster: u16) -> usize {
        self.first_data_sector() + (cluster as usize - 2) * self.sectors_per_cluster
    }

    fn free_chain(&mut self, first_cluster: u16) {
        if first_cluster < 2 {
            return;
        }

        let mut current = first_cluster;
        let mut steps = 0usize;

        loop {
            steps += 1;

            if steps > self.cluster_count() {
                return;
            }

            let next = self.fat_entry(current as usize);
            self.set_fat_entry(current as usize, 0x000);

            if next >= 0x002 && next <= 0xFEF {
                current = next;
            } else {
                break;
            }
        }
    }

    pub fn write_bytes(&mut self, dir_cluster: u16, name: &[u8; 11], data: &[u8]) -> bool {
        let (dir_sector, entry_offset) = match self.find_file(dir_cluster, name) {
            Some(location) => location,
            None => return false,
        };

        // 为简化当前实现支持 1 cluster = 1 sector
        if self.sectors_per_cluster != 1 {
            return false;
        }

        let mut buf = [0u8; SECTOR_SIZE];
        self.device.read_sector(dir_sector, &mut buf);

        let old_cluster = u16::from_le_bytes([buf[entry_offset + 26], buf[entry_offset + 27]]);

        if old_cluster >= 2 {
            self.free_chain(old_cluster);
        }

        if data.is_empty() {
            buf[entry_offset + 26..entry_offset + 28].copy_from_slice(&0u16.to_le_bytes());
            buf[entry_offset + 28..entry_offset + 32].copy_from_slice(&0u32.to_le_bytes());
            self.device.write_sector(dir_sector, &buf);
            return true;
        }

        let first_cluster = match self.find_free_cluster() {
            Some(cluster) => cluster,
            None => return false,
        };

        self.set_fat_entry(first_cluster as usize, 0xFFF); // 标记为已占用
        let mut buf = [0u8; SECTOR_SIZE];
        let mut current_cluster = first_cluster;
        let mut written = 0usize;
        while written < data.len() {
            let chunk_size = core::cmp::min(data.len() - written, SECTOR_SIZE);

            buf.fill(0);
            buf[..chunk_size].copy_from_slice(&data[written..written + chunk_size]);

            let sector = self.cluster_to_sector(current_cluster);
            self.device.write_sector(sector, &buf);
            written += chunk_size;

            if written == data.len() {
                break;
            }

            let next = match self.find_free_cluster() {
                Some(cluster) => cluster,
                None => return false,
            };

            self.set_fat_entry(next as usize, 0xFFF);
            self.set_fat_entry(current_cluster as usize, next);

            current_cluster = next;
        }

        // 更新目录项
        self.device.read_sector(dir_sector, &mut buf);
        buf[entry_offset + 26..entry_offset + 28].copy_from_slice(&first_cluster.to_le_bytes());
        buf[entry_offset + 28..entry_offset + 32]
            .copy_from_slice(&(data.len() as u32).to_le_bytes());
        self.device.write_sector(dir_sector, &buf);

        true
    }

    pub fn read_bytes(
        &mut self,
        dir_cluster: u16,
        name: &[u8; 11],
        out: &mut [u8],
    ) -> Option<usize> {
        let (dir_sector, entry_offset) = self.find_file(dir_cluster, name)?;

        let mut buf = [0u8; SECTOR_SIZE];
        self.device.read_sector(dir_sector, &mut buf);

        let cluster = u16::from_le_bytes([buf[entry_offset + 26], buf[entry_offset + 27]]);

        let size = u32::from_le_bytes([
            buf[entry_offset + 28],
            buf[entry_offset + 29],
            buf[entry_offset + 30],
            buf[entry_offset + 31],
        ]) as usize;

        if size == 0 {
            return Some(0);
        }

        if cluster < 2 {
            return None;
        }

        // 避免读取超过 out 的长度
        if size > out.len() {
            return None;
        }

        // 为简化当前实现支持 1 cluster = 1 sector
        if self.sectors_per_cluster != 1 {
            return None;
        }

        let mut current_cluster = cluster;
        let mut copied = 0usize;
        let mut steps = 0usize;

        while copied < size {
            steps += 1;

            // 防止损坏的 FAT 簇链导致无限循环
            if steps > self.cluster_count() {
                return None;
            }

            let sector = self.cluster_to_sector(current_cluster);
            self.device.read_sector(sector, &mut buf);

            let truck_size = core::cmp::min(size - copied, SECTOR_SIZE);

            out[copied..copied + truck_size].copy_from_slice(&buf[..truck_size]);

            copied += truck_size;

            if copied < size {
                current_cluster = self.next_cluster(current_cluster)?;
            };
        }
        Some(size)
    }
}
