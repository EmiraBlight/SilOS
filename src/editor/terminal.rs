use alloc::vec::Vec;
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

impl File {

    pub fn read(&mut self, fs: &Fat16FileSystem, buffer: &mut [u8]) -> usize {

    }
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
