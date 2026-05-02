use pc_keyboard::{DecodedKey, KeyCode};
use crate::fat16::Fat16FileSystem;
use crate::task::executor::yield_now;
use x86_64::instructions::port::Port;
use crate::vga_buffer::WRITER;
use crate::vga_buffer::ScreenChar;
use crate::fat16::{FS};
use crate::vga_buffer;
use crate::vga_buffer::BUFFER_WIDTH;
use crate::vga_buffer::BUFFER_HEIGHT;
use alloc::vec::Vec;


pub async fn run_editor(args: Vec<String>) {
    let name_arg = args.get(1).map(|s| s.as_str()).unwrap_or("UNNAMED");
    let ext_arg = args.get(2).map(|s| s.as_str()).unwrap_or("TXT");

    let (fat_name, fat_ext) = format_fat16_name(name_arg, ext_arg);

    let initial_data = if let Some(fs) = FS.lock().as_ref() {
        if let Some(entry) = fs.find_file(&fat_name, &fat_ext) {
            let file_bytes = fs.read_file(&entry);
            alloc::string::String::from_utf8_lossy(&file_bytes).into_owned()
        } else {
            alloc::string::String::new()
        }
    } else {
        alloc::string::String::new()
    };


    let mut editor = Editor::new(&initial_data);
    let mut terminal = VgaTerminal;

    let mut keyboard = pc_keyboard::Keyboard::new(
        pc_keyboard::ScancodeSet1::new(),
        pc_keyboard::layouts::Us104Key,
        pc_keyboard::HandleControl::Ignore,
    );

    editor.draw(&mut terminal);


    loop {
        let mut needs_redraw = false;

        while let Some(key_event) = crate::input::pop_key() {
            if let Some(key) = keyboard.process_keyevent(key_event) {
                match key {
                    DecodedKey::Unicode(character) => {
                        if character == '\u{1b}' {
                            crate::vga_buffer::clear_screen();

                            if let Some(fs) = FS.lock().as_ref() {
                                let file_contents = editor.lines.join("\n");
                                let bytes_to_write = file_contents.as_bytes();

                                if let Some(_entry) = fs.find_file(&fat_name, &fat_ext) {

                                    let _ = fs.overwrite_file(fat_name, fat_ext, bytes_to_write);

                                } else {

                                    let _ = fs.write_new_file(fat_name, fat_ext, bytes_to_write);

                                }
                            }

                            return;
                        } else if character == '\u{8}' {
                            editor.backspace();
                        } else {
                            editor.insert_char(character);
                        }
                        needs_redraw = true;
                    }
                    DecodedKey::RawKey(raw_key) => {

                        match raw_key {
                            KeyCode::ArrowLeft => editor.move_cursor_left(),
                            KeyCode::ArrowRight => editor.move_cursor_right(),
                            KeyCode::ArrowUp => editor.move_cursor_up(),
                            KeyCode::ArrowDown => editor.move_cursor_down(),
                            _ => {}
                        }
                        needs_redraw = true;
                    }
                }
            }
        }

        if needs_redraw {
            editor.draw(&mut terminal);
        }

        yield_now().await;
    }
}


pub fn format_fat16_name(name_str: &str, ext_str: &str) -> ([u8; 8], [u8; 3]) {
    let mut name = [0x20u8; 8]; // Pre-fill with spaces
    let mut ext = [0x20u8; 3];  // Pre-fill with spaces

    for (i, byte) in name_str.bytes().take(8).enumerate() {
        name[i] = byte.to_ascii_uppercase();
    }

    for (i, byte) in ext_str.bytes().take(3).enumerate() {
        ext[i] = byte.to_ascii_uppercase();
    }

    (name, ext)
}

use alloc::string::String;

pub trait TerminalDevice {

    fn dimensions(&self) -> (usize, usize);

    fn put_char(&mut self, x: usize, y: usize, c: char);

    fn move_cursor(&mut self, x: usize, y: usize);

    fn clear_screen(&mut self);
}


pub struct EditorModel {
    lines: Vec<String>,
    cursor_x: usize,
    cursor_y: usize,

    row_offset: usize,
    col_offset: usize,
}





pub struct Editor {
    lines: Vec<String>,
    cursor_x: usize,
    cursor_y: usize,
}

impl Editor {
    pub fn new(initial_data: &str) -> Self {
        let mut lines: Vec<String> = initial_data
            .lines()
            .map(|line| String::from(line))
            .collect();

        if lines.is_empty() {
            lines.push(String::new());
        }

        Self {
            lines,
            cursor_x: 0,
            cursor_y: 0,
        }
    }
}

impl Editor {
    pub fn insert_char(&mut self, c: char) {
        if c == '\n' {
            self.insert_newline();
            return;
        }

        self.lines[self.cursor_y].insert(self.cursor_x, c);
        self.cursor_x += 1;
    }

    pub fn insert_newline(&mut self) {
        let current_line = &mut self.lines[self.cursor_y];
        let new_line_content = current_line.split_off(self.cursor_x);

        self.cursor_y += 1;
        self.lines.insert(self.cursor_y, new_line_content);
        self.cursor_x = 0;
    }

    pub fn backspace(&mut self) {
        if self.cursor_x > 0 {
            self.cursor_x -= 1;
            self.lines[self.cursor_y].remove(self.cursor_x);
        } else if self.cursor_y > 0 {
            let current_line = self.lines.remove(self.cursor_y);
            self.cursor_y -= 1;

            let prev_line = &mut self.lines[self.cursor_y];
            self.cursor_x = prev_line.len();
            prev_line.push_str(&current_line);
        }
    }
}

impl Editor {
    pub fn draw(&self, terminal: &mut dyn TerminalDevice) {
        terminal.clear_screen();

        for (y, line) in self.lines.iter().enumerate() {
            for (x, c) in line.chars().enumerate() {
                terminal.put_char(x, y, c);
            }
        }

        terminal.move_cursor(self.cursor_x, self.cursor_y);
    }
}

impl Editor {
    pub fn move_cursor_left(&mut self) {
        if self.cursor_x > 0 {
            self.cursor_x -= 1;
        } else if self.cursor_y > 0 {
            self.cursor_y -= 1;
            self.cursor_x = self.lines[self.cursor_y].len();
        }
    }

    pub fn move_cursor_right(&mut self) {
        if self.cursor_x < self.lines[self.cursor_y].len() {
            self.cursor_x += 1;
        } else if self.cursor_y < self.lines.len() - 1 {
            // Move to the start of the next line
            self.cursor_y += 1;
            self.cursor_x = 0;
        }
    }

    pub fn move_cursor_up(&mut self) {
        if self.cursor_y > 0 {
            self.cursor_y -= 1;
            let line_len = self.lines[self.cursor_y].len();
            if self.cursor_x > line_len {
                self.cursor_x = line_len;
            }
        }
    }

    pub fn move_cursor_down(&mut self) {
        if self.cursor_y < self.lines.len() - 1 {
            self.cursor_y += 1;
            let line_len = self.lines[self.cursor_y].len();
            if self.cursor_x > line_len {
                self.cursor_x = line_len;
            }
        }
    }
}



pub struct VgaTerminal;

impl TerminalDevice for VgaTerminal {
    fn dimensions(&self) -> (usize, usize) {
        (80, 25)
    }

    fn put_char(&mut self, x: usize, y: usize, c: char) {

        crate::vga_buffer::write_char_at(x, y, c as u8, 0x0F);
    }

    fn move_cursor(&mut self, x: usize, y: usize) {
        crate::vga_buffer::update_cursor(x, y);
    }

    fn clear_screen(&mut self) {
        crate::vga_buffer::clear_screen();
    }
}
