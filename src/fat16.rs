use crate::ide::IDE;
use crate::println;
use alloc::vec::Vec;
use core::cmp::min;
use lazy_static::lazy_static;
use spin::Mutex;
#[repr(C, packed)]
pub struct Bpb {
    jump: [u8; 3],
    oem_name: [u8; 8],
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    reserved_sectors: u16,
    fat_count: u8,
    root_entries: u16,
    total_sectors_16: u16,
    media_type: u8,
    sectors_per_fat: u16,
}

#[repr(C, packed)]
pub struct DirectoryEntry {
    filename: [u8; 8],
    extension: [u8; 3],
    attributes: u8,
    reserved: u8,
    creation_time_ms: u8,
    creation_time: u16,
    creation_date: u16,
    last_access_date: u16,
    cluster_high: u16,
    modified_time: u16,
    modified_date: u16,
    cluster_low: u16,
    file_size: u32,
}

pub struct Fat16FileSystem {
    bpb: Bpb,
}

lazy_static! {
    pub static ref FS: Mutex<Option<Fat16FileSystem>> = Mutex::new(None);
}

pub fn init() {
    let mut drive = IDE.lock();

    let buffer = drive.read_sector(0);

    let bpb: Bpb = unsafe { core::ptr::read(buffer.as_ptr() as *const Bpb) };

    if bpb.bytes_per_sector != 512 {
        println!("[FS] Error: Invalid BPB found. Disk might not be FAT16 or has an MBR.");
        return;
    }

    println!(
        "[FS] FAT16 Initialized. Sectors per cluster: {}",
        bpb.sectors_per_cluster
    );

    *FS.lock() = Some(Fat16FileSystem { bpb });
}

impl Fat16FileSystem {
    fn cluster_to_lba(&self, cluster: u16) -> u32 {
        let root_dir_sectors = ((self.bpb.root_entries * 32) + (self.bpb.bytes_per_sector - 1))
            / self.bpb.bytes_per_sector;
        let first_data_sector = self.bpb.reserved_sectors as u32
            + (self.bpb.fat_count as u32 * self.bpb.sectors_per_fat as u32)
            + root_dir_sectors as u32;

        first_data_sector + ((cluster as u32 - 2) * self.bpb.sectors_per_cluster as u32)
    }

    pub fn read_file(&self, entry: &DirectoryEntry) -> Vec<u8> {
        let mut file_data = Vec::with_capacity(entry.file_size as usize);
        let mut current_cluster = entry.cluster_low;
        let mut bytes_remaining = entry.file_size;

        while current_cluster < 0xFFF8 {
            let lba = self.cluster_to_lba(current_cluster);

            for i in 0..self.bpb.sectors_per_cluster {
                let sector_data = IDE.lock().read_sector(lba + i as u32);

                let to_copy = core::cmp::min(bytes_remaining as usize, 512);
                file_data.extend_from_slice(&sector_data[..to_copy]);

                bytes_remaining -= to_copy as u32;
                if bytes_remaining == 0 {
                    break;
                }
            }

            if bytes_remaining == 0 {
                break;
            }

            current_cluster = self.get_fat_entry(current_cluster);
        }

        file_data
    }

    fn get_fat_entry(&self, cluster: u16) -> u16 {
        let fat_offset = cluster as u32 * 2;
        let fat_sector = self.bpb.reserved_sectors as u32 + (fat_offset / 512);
        let ent_offset = (fat_offset % 512) as usize;

        let buffer = IDE.lock().read_sector(fat_sector);

        u16::from_le_bytes([buffer[ent_offset], buffer[ent_offset + 1]])
    }

    fn find_free_cluster(&self) -> Option<u16> {
        let mut drive = IDE.lock();
        let fat_start = self.bpb.reserved_sectors as u32;

        for s in 0..self.bpb.sectors_per_fat {
            let buffer = drive.read_sector(fat_start + s as u32);
            for i in (0..512).step_by(2) {
                let status = u16::from_le_bytes([buffer[i], buffer[i + 1]]);
                if status == 0x0000 {
                    return Some(((s * 256) + (i as u16 / 2)) as u16);
                }
            }
        }
        None
    }

    fn set_fat_entry(&self, cluster: u16, next_cluster: u16) {
        let fat_offset = cluster as u32 * 2;
        let fat_sector_lba = self.bpb.reserved_sectors as u32 + (fat_offset / 512);
        let byte_offset = (fat_offset % 512) as usize;

        let mut drive = IDE.lock();
        let mut buffer = drive.read_sector(fat_sector_lba);

        let next_bytes = next_cluster.to_le_bytes();
        buffer[byte_offset] = next_bytes[0];
        buffer[byte_offset + 1] = next_bytes[1];

        drive.write_sector_bytes(fat_sector_lba, &buffer);
    }

    fn find_empty_root_slot(&self) -> Option<(u32, usize)> {
        let root_dir_start = self.bpb.reserved_sectors as u32
            + (self.bpb.fat_count as u32 * self.bpb.sectors_per_fat as u32);
        let root_dir_sectors = ((self.bpb.root_entries * 32) + 511) / 512;

        let mut drive = IDE.lock();

        for sector_offset in 0..root_dir_sectors as u32 {
            let lba = root_dir_start + sector_offset;
            let buffer = drive.read_sector(lba);

            for i in 0..16 {
                let offset = i * 32;
                let first_byte = buffer[offset];

                if first_byte == 0x00 || first_byte == 0xE5 {
                    return Some((lba, offset));
                }
            }
        }
        None
    }

    pub fn write_new_file(
        &self,
        filename: [u8; 8],
        ext: [u8; 3],
        data: &[u8],
    ) -> Result<(), &'static str> {
        let (dir_lba, dir_offset) = self.find_empty_root_slot().ok_or("Root directory full")?;

        let bytes_per_cluster = self.bpb.sectors_per_cluster as usize * 512;
        let mut bytes_written = 0;
        let mut previous_cluster: Option<u16> = None;
        let mut first_cluster: u16 = 0;

        while bytes_written < data.len() || data.is_empty() {
            let current_cluster = self.find_free_cluster().ok_or("Disk full")?;

            self.set_fat_entry(current_cluster, 0xFFFF);

            if previous_cluster.is_none() {
                first_cluster = current_cluster;
            } else {
                self.set_fat_entry(previous_cluster.unwrap(), current_cluster);
            }

            let cluster_lba = self.cluster_to_lba(current_cluster);
            let chunk_size = min(data.len() - bytes_written, bytes_per_cluster);
            let chunk = &data[bytes_written..bytes_written + chunk_size];

            let mut drive = IDE.lock();
            for sector_idx in 0..self.bpb.sectors_per_cluster as u32 {
                let sector_start = sector_idx as usize * 512;
                if sector_start >= chunk.len() {
                    break;
                }

                let mut buffer = [0u8; 512];
                let copy_len = min(512, chunk.len() - sector_start);
                buffer[..copy_len].copy_from_slice(&chunk[sector_start..sector_start + copy_len]);

                drive.write_sector_bytes(cluster_lba + sector_idx, &buffer);
            }

            bytes_written += chunk_size;
            previous_cluster = Some(current_cluster);

            if data.is_empty() {
                break;
            }
        }

        let mut entry = DirectoryEntry {
            filename,
            extension: ext,
            attributes: 0x20,
            reserved: 0,
            creation_time_ms: 0,
            creation_time: 0,
            creation_date: 0,
            last_access_date: 0,
            cluster_high: 0,
            modified_time: 0,
            modified_date: 0,
            cluster_low: first_cluster,
            file_size: data.len() as u32,
        };

        let mut drive = IDE.lock();
        let mut dir_buffer = drive.read_sector(dir_lba);

        let entry_bytes: &[u8; 32] = unsafe { core::mem::transmute(&entry) };
        dir_buffer[dir_offset..dir_offset + 32].copy_from_slice(entry_bytes);

        drive.write_sector_bytes(dir_lba, &dir_buffer);

        Ok(())
    }

    pub fn format_drive() -> Result<(), &'static str> {
        let mut drive = IDE.lock();

        let bytes_per_sector: u16 = 512;
        let sectors_per_cluster: u8 = 4;
        let reserved_sectors: u16 = 1;
        let fat_count: u8 = 2;
        let root_entries: u16 = 512;
        let total_sectors_16: u16 = 65535;
        let media_type: u8 = 0xF8;
        let sectors_per_fat: u16 = 64;

        let mut boot_sector = [0u8; 512];

        boot_sector[0..3].copy_from_slice(&[0xEB, 0x3C, 0x90]);
        boot_sector[3..11].copy_from_slice(b"myOS    "); // OEM Name

        boot_sector[11..13].copy_from_slice(&bytes_per_sector.to_le_bytes());
        boot_sector[13] = sectors_per_cluster;
        boot_sector[14..16].copy_from_slice(&reserved_sectors.to_le_bytes());
        boot_sector[16] = fat_count;
        boot_sector[17..19].copy_from_slice(&root_entries.to_le_bytes());
        boot_sector[19..21].copy_from_slice(&total_sectors_16.to_le_bytes());
        boot_sector[21] = media_type;
        boot_sector[22..24].copy_from_slice(&sectors_per_fat.to_le_bytes());

        boot_sector[510] = 0x55;
        boot_sector[511] = 0xAA;

        drive.write_sector_bytes(0, &boot_sector);

        let fat_start_lba = reserved_sectors as u32;

        for fat_idx in 0..fat_count {
            let current_fat_start = fat_start_lba + (fat_idx as u32 * sectors_per_fat as u32);

            for offset in 0..sectors_per_fat as u32 {
                let mut fat_sector = [0u8; 512];

                if offset == 0 {
                    fat_sector[0] = media_type;
                    fat_sector[1] = 0xFF;
                    fat_sector[2] = 0xFF;
                    fat_sector[3] = 0xFF;
                }

                drive.write_sector_bytes(current_fat_start + offset, &fat_sector);
            }
        }

        // 4. Clear the Root Directory
        let root_dir_start = fat_start_lba + (fat_count as u32 * sectors_per_fat as u32);
        let root_dir_sectors = ((root_entries * 32) + 511) / 512;
        let empty_sector = [0u8; 512];

        for offset in 0..root_dir_sectors as u32 {
            drive.write_sector_bytes(root_dir_start + offset, &empty_sector);
        }

        unsafe {
            drive.command.write(0xE7);
        }
        println!("Formatted drive!");
        Ok(())
    }

    pub fn find_file(&self, filename: &[u8; 8], ext: &[u8; 3]) -> Option<DirectoryEntry> {
        let root_dir_start = self.bpb.reserved_sectors as u32
            + (self.bpb.fat_count as u32 * self.bpb.sectors_per_fat as u32);
        let root_dir_sectors = ((self.bpb.root_entries * 32) + 511) / 512;

        let mut drive = crate::ide::IDE.lock();

        for sector_offset in 0..root_dir_sectors as u32 {
            let buffer = drive.read_sector(root_dir_start + sector_offset);

            for i in 0..16 {
                let offset = i * 32;
                let first_byte = buffer[offset];

                if first_byte == 0x00 {
                    return None;
                }
                if first_byte == 0xE5 {
                    continue;
                }

                let is_match = &buffer[offset..offset + 8] == filename
                    && &buffer[offset + 8..offset + 11] == ext;

                if is_match {
                    let entry: DirectoryEntry = unsafe {
                        core::ptr::read(buffer[offset..].as_ptr() as *const DirectoryEntry)
                    };
                    return Some(entry);
                }
            }
        }
        None
    }
}
