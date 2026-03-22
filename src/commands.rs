use crate::pong::PongGame;
use crate::println;
use crate::shell::SHELL;
use crate::vga_buffer::WRITER;
use alloc::collections::BTreeMap;
use alloc::string::String;
use core::sync::atomic::AtomicBool;
use lazy_static::lazy_static;
use spin::Mutex;

pub static COMMAND_PENDING: AtomicBool = AtomicBool::new(false);

type CommandFn = fn() -> ();

fn clear() {
    WRITER.lock().clear()
}

fn pong() {
    crate::interrupts::LAUNCH_PONG.swap(true, core::sync::atomic::Ordering::Relaxed);
    let mut game = PongGame::new();
    game.run();
    crate::interrupts::LAUNCH_PONG.swap(false, core::sync::atomic::Ordering::Relaxed);
}

fn history() {
    let s = SHELL.lock();
    s.history();
}

pub fn init_cmds() {
    let mut c = COMMANDS.lock();
    c.insert(String::from("pong"), pong);
    c.insert(String::from("clear"), clear);
    c.insert(String::from("history"), history);
}

pub fn run_cmd(cmd: String) {
    match COMMANDS.lock().get(&cmd) {
        None => println!("{} is not a command", cmd),
        Some(f) => f(),
    }
}

lazy_static! {
    pub static ref COMMANDS: Mutex<BTreeMap<String, CommandFn>> = Mutex::new(BTreeMap::new());
}
