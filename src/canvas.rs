use crate::vga_buffer::{Color, ColorCode, ScreenChar};
use volatile::Volatile;

pub const SCREEN_WIDTH: usize = 80;
pub const SCREEN_HEIGHT: usize = 25;
pub const VGA_TEXT_ADDR: usize = 0xb8000;

pub struct TextCanvas {
    // Points directly to the VGA text buffer
    buffer: &'static mut [[Volatile<ScreenChar>; SCREEN_WIDTH]; SCREEN_HEIGHT],
}

impl TextCanvas {
    pub fn new() -> Self {
        unsafe {
            TextCanvas {
                buffer: &mut *(VGA_TEXT_ADDR as *mut _),
            }
        }
    }

    pub fn set_char(&mut self, x: usize, y: usize, character: u8, color: ColorCode) {
        if x < SCREEN_WIDTH && y < SCREEN_HEIGHT {
            self.buffer[y][x].write(ScreenChar {
                ascii_character: character,
                color_code: color,
            });
        }
    }

    pub fn clear(&mut self) {
        let blank = ScreenChar {
            ascii_character: b' ',
            color_code: ColorCode::new(Color::Black, Color::Black),
        };
        for row in 0..SCREEN_HEIGHT {
            for col in 0..SCREEN_WIDTH {
                self.buffer[row][col].write(blank);
            }
        }
    }
}
