use crate::alloc::string::ToString;
use crate::alloc::sync::Arc;
use crate::ide::IDE;
use crate::parser::interpret;
use crate::pong::PongGame;
use crate::print;
use crate::println;
use crate::programReturn::ProcessError;
use crate::programReturn::Success;
use crate::shell::SHELL;
use crate::vga_buffer::WRITER;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::AtomicBool;
use spin::Mutex;
pub static COMMAND_PENDING: AtomicBool = AtomicBool::new(false);
use core::str;

fn read_as_string(args: Vec<String>) -> Result<Success, ProcessError> {
    if (args.len() != 2) {
        return Err(ProcessError {
            error_code: "Needs 2 parameters".to_string(),
        });
    }
    let mut loc: u32 = 0;
    let mut nan = false;
    match args[1].parse::<u32>() {
        Ok(n) => loc = n,
        Err(_) => {
            nan = true;
        }
    };

    if nan {
        return Err(ProcessError {
            error_code: "needs a number".to_string(),
        });
    } else {
        let buffer = IDE.lock().read_sector(loc);
        let s: &str = core::str::from_utf8(&buffer).unwrap().trim_matches('\0');
        println!("{}", s);

        Ok(Success {
            success_code: "Worked".to_string(),
            print_code: false,
        })
    }
}

fn read(args: Vec<String>) -> Result<Success, ProcessError> {
    if (args.len() != 2) {
        return Err(ProcessError {
            error_code: "Needs 2 parameters".to_string(),
        });
    }
    let mut loc: u32 = 0;
    let mut nan = false;
    match args[1].parse::<u32>() {
        Ok(n) => loc = n,
        Err(_) => {
            nan = true;
        }
    };

    if nan {
        return Err(ProcessError {
            error_code: "needs a number".to_string(),
        });
    } else {
        let res = IDE.lock().read_sector(loc);
        for i in (0..512) {
            print!("{} ", res[i]);
        }
        return Ok(Success {
            success_code: "worked".to_string(),
            print_code: false,
        });
    }
}

fn write(args: Vec<String>) -> Result<Success, ProcessError> {
    if args.len() != 3 {
        return Err(ProcessError {
            error_code: "Usage: write <sector_index> <string_data>".to_string(),
        });
    }

    let loc: u32 = args[1].parse::<u32>().map_err(|_| ProcessError {
        error_code: "Sector index must be a valid u32 number".to_string(),
    })?;

    let data_bytes = args[2].as_bytes();
    if data_bytes.len() > 512 {
        return Err(ProcessError {
            error_code: format!("Data too large: {} bytes (max 512)", data_bytes.len()),
        });
    }

    let mut buffer = [0u8; 512];
    buffer[..data_bytes.len()].copy_from_slice(data_bytes);

    IDE.lock().write_sector_bytes(loc, &buffer);

    Ok(Success {
        success_code: "Sector written successfully".to_string(),
        print_code: false,
    })
}

fn clear(_args: Vec<String>) -> Result<Success, ProcessError> {
    WRITER.lock().clear();
    Ok(Success {
        success_code: "worked".to_string(),
        print_code: false,
    })
}

fn bind(args: Vec<String>) -> Result<Success, ProcessError> {
    if args.len() < 2 {
        return Err(ProcessError {
            error_code: "invalid number of args".to_string(),
        });
    }

    let command_name = args[1].clone();

    let lisp_code = args[2].clone();

    let wrapper = move |runtime_args: Vec<String>| {
        let mut exec_expr = vec![String::from("exec"), lisp_code.clone()];

        if runtime_args.len() > 1 {
            for arg in &runtime_args[1..] {
                exec_expr.push(arg.clone());
            }
        }

        interpret(exec_expr)
    };

    let mut c = COMMANDS.lock();
    c.insert(command_name, Arc::new(wrapper));

    Ok(Success {
        success_code: "Bound successfully".to_string(),
        print_code: false,
    })
}

fn pong(_args: Vec<String>) -> Result<Success, ProcessError> {
    crate::interrupts::LAUNCH_PONG.swap(true, core::sync::atomic::Ordering::Relaxed);
    let mut game = PongGame::new();
    let result = game.run();
    crate::interrupts::LAUNCH_PONG.swap(false, core::sync::atomic::Ordering::Relaxed);
    result
}

fn history(_args: Vec<String>) -> Result<Success, ProcessError> {
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

pub fn init_cmds() {
    let mut c = COMMANDS.lock();
    c.insert(String::from("pong"), Arc::new(pong));
    c.insert(String::from("clear"), Arc::new(clear));
    c.insert(String::from("history"), Arc::new(history));
    c.insert(String::from("echo"), Arc::new(echo));
    c.insert(String::from("parse"), Arc::new(interpret));
    c.insert(String::from("bind"), Arc::new(bind));
    c.insert(String::from("read"), Arc::new(read));
    c.insert(String::from("write"), Arc::new(write));
    c.insert(String::from("show"), Arc::new(read_as_string));
}

pub fn run_cmd(cmd: Vec<String>) -> Result<Success, ProcessError> {
    if cmd.is_empty() {
        return Ok(Success {
            success_code: "".to_string(),
            print_code: false,
        });
    }

    let command_fn = {
        let lock = COMMANDS.lock();
        lock.get(&cmd[0]).cloned()
    };

    match command_fn {
        Some(f) => f(cmd),
        None => Err(ProcessError {
            error_code: format!("'{}' command not found", cmd[0]),
        }),
    }
}

pub fn get_command_list() -> Vec<String> {
    let lock = COMMANDS.lock();

    lock.keys().cloned().collect()
}

lazy_static::lazy_static! {
    static ref COMMANDS: Mutex<BTreeMap<String, Arc<dyn Fn(Vec<String>) -> Result<Success, ProcessError> + Send + Sync>>> =
        Mutex::new(BTreeMap::new());
}
