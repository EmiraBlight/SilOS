use spin::Mutex;
use x86_64::instructions::port::Port;
pub struct AtaDrive {
    data: Port<u16>,
    error: Port<u8>,
    sector_count: Port<u8>,
    lba_low: Port<u8>,
    lba_mid: Port<u8>,
    lba_high: Port<u8>,
    device: Port<u8>,
    command: Port<u8>,
    status: Port<u8>,
}

impl AtaDrive {
    pub fn new() -> Self {
        Self {
            data: Port::new(0x1F0),
            error: Port::new(0x1F1),
            sector_count: Port::new(0x1F2),
            lba_low: Port::new(0x1F3),
            lba_mid: Port::new(0x1F4),
            lba_high: Port::new(0x1F5),
            device: Port::new(0x1F6),
            command: Port::new(0x1F7),
            status: Port::new(0x1F7),
        }
    }

    pub fn read_sector(&mut self, lba: u32) -> [u8; 512] {
        unsafe {
            self.device.write(0xF0 | ((lba >> 24) & 0x0F) as u8);
            self.sector_count.write(1);
            self.lba_low.write(lba as u8);
            self.lba_mid.write((lba >> 8) as u8);
            self.lba_high.write((lba >> 16) as u8);

            self.command.write(0x20);

            while (self.status.read() & 0x80) != 0 {}
            while (self.status.read() & 0x08) == 0 {}

            let mut buffer = [0u8; 512];
            for i in 0..256 {
                let word = self.data.read();
                buffer[i * 2] = word as u8;
                buffer[i * 2 + 1] = (word >> 8) as u8;
            }
            buffer
        }
    }

    pub fn get_max_lba(&mut self) -> u32 {
        unsafe {
            self.device.write(0xA0);

            self.command.write(0xEC);

            let status = self.status.read();
            if status == 0 {
                return 0; // No drive on this bus
            }

            let mut timeout = 0;
            while (self.status.read() & 0x80) != 0 {
                timeout += 1;
                if timeout > 1000000 {
                    return 0;
                }
            }

            if (self.status.read() & 0x01) != 0 {
                return 0;
            }

            while (self.status.read() & 0x08) == 0 {}

            let mut info = [0u16; 256];
            for i in 0..256 {
                info[i] = self.data.read();
            }

            let low = info[60] as u32;
            let high = info[61] as u32;
            (high << 16) | low
        }
    }

    pub fn write_sector_bytes(&mut self, lba: u32, bytes: &[u8; 512]) {
        unsafe {
            self.device.write(0xF0 | ((lba >> 24) & 0x0F) as u8);
            self.sector_count.write(1);
            self.lba_low.write(lba as u8);
            self.lba_mid.write((lba >> 8) as u8);
            self.lba_high.write((lba >> 16) as u8);

            self.command.write(0x30);

            while (self.status.read() & 0x80) != 0 {}
            while (self.status.read() & 0x08) == 0 {}

            for i in (0..512).step_by(2) {
                let word = (bytes[i + 1] as u16) << 8 | (bytes[i] as u16);
                self.data.write(word);
            }

            self.command.write(0xE7);

            while (self.status.read() & 0x80) != 0 {}
        }
    }
}
lazy_static::lazy_static! {
    pub static ref IDE: Mutex<AtaDrive> =
    Mutex::new(AtaDrive::new());
}
