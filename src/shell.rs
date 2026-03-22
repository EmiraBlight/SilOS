use crate::println;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use spin::Mutex;

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shell {
    current_cmd: String,
    past_cmd: Vec<String>,
}

impl Shell {
    pub fn add(&mut self, char: u8) {
        self.current_cmd.push(char as char);
    }

    pub fn backspace(&mut self) {
        self.current_cmd.pop();
    }

    pub fn getcmd(&self) -> Option<String> {
        match self.current_cmd.as_str() {
            "" => None,
            s => Some(s.to_string()),
        }
    }

    pub fn clear(&mut self) {
        let buffer = self.getcmd();
        if let Some(curr_cmd) = buffer {
            self.past_cmd.append(&mut vec![curr_cmd]);
        }
        self.current_cmd.clear();
    }

    pub fn history(&self) {
        for i in self.past_cmd.clone() {
            println!("{}", i);
        }
    }
}

use lazy_static::lazy_static;
lazy_static! {
    pub static ref SHELL: Mutex<Shell> = Mutex::new(Shell {
        current_cmd: "".to_string(),
        past_cmd: vec![]
    });
}
