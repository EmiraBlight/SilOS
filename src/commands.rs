use crate::alloc::string::ToString;
use crate::pong::PongGame;
use crate::println;
use crate::programReturn::ProcessError;
use crate::programReturn::Success;
use crate::shell::SHELL;
use crate::vga_buffer::WRITER;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::AtomicBool;
use lazy_static::lazy_static;
use spin::Mutex;
pub static COMMAND_PENDING: AtomicBool = AtomicBool::new(false);

type CommandFn = fn(Vec<String>) -> Result<Success, ProcessError>;

fn clear(args: Vec<String>) -> Result<Success, ProcessError> {
    WRITER.lock().clear();
    Ok(Success {
        success_code: "worked".to_string(),
        print_code: false,
    })
}

fn pong(args: Vec<String>) -> Result<Success, ProcessError> {
    crate::interrupts::LAUNCH_PONG.swap(true, core::sync::atomic::Ordering::Relaxed);
    let mut game = PongGame::new();
    let result = game.run();
    crate::interrupts::LAUNCH_PONG.swap(false, core::sync::atomic::Ordering::Relaxed);
    result
}

fn history(args: Vec<String>) -> Result<Success, ProcessError> {
    let s = SHELL.lock();
    s.history();
    Ok(Success {
        success_code: "worked".to_string(),
        print_code: false,
    })
}

fn echo(args: Vec<String>) -> Result<Success, ProcessError> {
    if args.len() < 2 {
        return Err(ProcessError {
            error_code: "Not enough arguments!".to_string(),
        });
    }

    println!("{}", args[1]);

    Ok(Success {
        success_code: "worked".to_string(),
        print_code: false,
    })
}
use crate::parser::interpret;
pub fn init_cmds() {
    let mut c = COMMANDS.lock();
    c.insert(String::from("pong"), pong);
    c.insert(String::from("clear"), clear);
    c.insert(String::from("history"), history);
    c.insert(String::from("echo"), echo);
    c.insert(String::from("parse"), interpret);
}

pub fn run_cmd(cmd: Vec<String>) -> Result<Success, ProcessError> {
    let command_fn = {
        let lock = COMMANDS.lock();
        lock.get(&cmd[0]).cloned() // Clone the function pointer/handler
    };

    match command_fn {
        None => {
            let f = format!("'{}' command not found", cmd[0]);
            Err(ProcessError {
                error_code: String::from(f),
            })
        }

        Some(f) => {
            unsafe { COMMANDS.force_unlock() };
            return f(cmd);
        }
    }
}

pub fn get_command_list() -> Vec<String> {
    let lock = COMMANDS.lock();

    lock.keys().cloned().collect()
}

lazy_static! {
    pub static ref COMMANDS: Mutex<BTreeMap<String, CommandFn>> = Mutex::new(BTreeMap::new());
}
