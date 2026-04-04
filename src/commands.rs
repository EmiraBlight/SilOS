use crate::alloc::string::ToString;
use crate::alloc::sync::Arc;
use crate::parser::interpret;
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
use alloc::vec;
use core::sync::atomic::AtomicBool;

use spin::Mutex;
pub static COMMAND_PENDING: AtomicBool = AtomicBool::new(false);

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
