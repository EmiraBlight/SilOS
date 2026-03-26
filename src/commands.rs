use crate::pong::PongGame;
use crate::println;
use crate::shell::SHELL;
use crate::vga_buffer::WRITER;
use alloc::collections::BTreeMap;
use alloc::string::String;
use core::sync::atomic::AtomicBool;
use lazy_static::lazy_static;
use spin::Mutex;
use alloc::format;
use crate::alloc::string::ToString;
use crate::programReturn::Success;
use crate::programReturn::ProcessError;

pub static COMMAND_PENDING: AtomicBool = AtomicBool::new(false);



type CommandFn = fn() -> Result<Success,ProcessError>;

fn clear() -> Result<Success,ProcessError>{
    WRITER.lock().clear();
    Ok(Success{success_code:"worked".to_string(),print_code:false})
}

fn pong() ->Result<Success,ProcessError>{
    crate::interrupts::LAUNCH_PONG.swap(true, core::sync::atomic::Ordering::Relaxed);
    let mut game = PongGame::new();
    let result = game.run();
    crate::interrupts::LAUNCH_PONG.swap(false, core::sync::atomic::Ordering::Relaxed);
    result
}

fn history() -> Result<Success,ProcessError> {
    let s = SHELL.lock();
    s.history();
    Ok(Success{success_code:"worked".to_string(),print_code:false})
}

pub fn init_cmds() {
    let mut c = COMMANDS.lock();
    c.insert(String::from("pong"), pong);
    c.insert(String::from("clear"), clear);
    c.insert(String::from("history"), history);
}

pub fn run_cmd(cmd: String)->Result<Success,ProcessError> {
    match COMMANDS.lock().get(&cmd) {
        None =>{
            let f =  format!("'{}' command not found",cmd);
            Err(ProcessError{error_code: String::from(f)})
        },  
                                            
        Some(f) => f(),
    }
}

lazy_static! {
    pub static ref COMMANDS: Mutex<BTreeMap<String, CommandFn>> = Mutex::new(BTreeMap::new());
}
