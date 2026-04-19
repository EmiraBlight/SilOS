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
use crate::task::executor::yield_now;
pub static COMMAND_PENDING: AtomicBool = AtomicBool::new(false);
use core::str;

use crate::fat16::FS;
use core::cmp::min;


pub async fn shell_task() {
    loop {
        // Check for command signal
        if crate::commands::COMMAND_PENDING.swap(false, core::sync::atomic::Ordering::Acquire) {
            let cmd_opt = {
                let mut shell = crate::shell::SHELL.lock();
                let cmd = shell.getcmd().clone();
                shell.clear();
                cmd
            };

            if let Some(cmd_str) = cmd_opt {
                if !cmd_str[0].trim().is_empty() {
                    // Process logic
                    let response = crate::commands::run_cmd(cmd_str.clone());
                    // Handle errors/success...
                }
            }
        }
        // Yield control back to the executor, allowing
        // the keyboard/print task to run
        yield_now();
    }
}



fn run_file(args: Vec<String>) -> Result<Success, ProcessError> {
    if args.len() < 3 {
        return Err(ProcessError {
            error_code: "Usage: run <NAME> <EXT> [args...]".to_string(),
        });
    }

    let mut filename = [b' '; 8];
    let name_bytes = args[1].to_ascii_uppercase().into_bytes();
    let name_len = min(8, name_bytes.len());
    filename[..name_len].copy_from_slice(&name_bytes[..name_len]);

    let mut ext = [b' '; 3];
    let ext_bytes = args[2].to_ascii_uppercase().into_bytes();
    let ext_len = min(3, ext_bytes.len());
    ext[..ext_len].copy_from_slice(&ext_bytes[..ext_len]);

    let lisp_code = {
        let fs_lock = FS.lock();
        if let Some(fs) = fs_lock.as_ref() {
            match fs.find_file(&filename, &ext) {
                Some(entry) => {
                    let file_data = fs.read_file(&entry);
                    match core::str::from_utf8(&file_data) {
                        Ok(text) => text.trim_matches('\0').to_string(),
                        Err(_) => {
                            return Err(ProcessError {
                                error_code: "Script contains invalid text formatting".to_string(),
                            });
                        }
                    }
                }
                None => {
                    return Err(ProcessError {
                        error_code: format!("Script {}.{} not found", args[1], args[2]),
                    });
                }
            }
        } else {
            return Err(ProcessError {
                error_code: "File system not mounted!".to_string(),
            });
        }
    };

    let mut exec_expr = vec![String::from("exec"), lisp_code];

    if args.len() > 3 {
        for arg in &args[3..] {
            exec_expr.push(arg.clone());
        }
    }

    interpret(exec_expr)
}

fn cat_file(args: Vec<String>) -> Result<Success, ProcessError> {
    if args.len() < 3 {
        return Err(ProcessError {
            error_code: "Usage: cat <NAME> <EXT>".to_string(),
        });
    }

    let mut filename = [b' '; 8];
    let name_bytes = args[1].to_ascii_uppercase().into_bytes();
    let name_len = min(8, name_bytes.len());
    filename[..name_len].copy_from_slice(&name_bytes[..name_len]);

    let mut ext = [b' '; 3];
    let ext_bytes = args[2].to_ascii_uppercase().into_bytes();
    let ext_len = min(3, ext_bytes.len());
    ext[..ext_len].copy_from_slice(&ext_bytes[..ext_len]);

    let fs_lock = FS.lock();
    if let Some(fs) = fs_lock.as_ref() {
        match fs.find_file(&filename, &ext) {
            Some(entry) => {
                let file_data = fs.read_file(&entry);

                match core::str::from_utf8(&file_data) {
                    Ok(text) => {
                        println!("--- {} ---", args[1]);
                        println!("{}", text);
                        println!("------------");

                        Ok(Success {
                            success_code: "File read successfully".to_string(),
                            print_code: false,
                        })
                    }
                    Err(_) => Err(ProcessError {
                        error_code: "File contains invalid UTF-8 (might be a binary file)"
                            .to_string(),
                    }),
                }
            }
            None => Err(ProcessError {
                error_code: format!("File {}.{} not found", args[1], args[2]),
            }),
        }
    } else {
        Err(ProcessError {
            error_code: "File system not mounted!".to_string(),
        })
    }
}

fn make_file(args: Vec<String>) -> Result<Success, ProcessError> {
    if args.len() < 4 {
        return Err(ProcessError {
            error_code: "Usage: mkfile <NAME> <EXT> <data to write>".to_string(),
        });
    }

    let mut filename = [b' '; 8];
    let name_bytes = args[1].to_ascii_uppercase().into_bytes();
    let name_len = min(8, name_bytes.len());
    filename[..name_len].copy_from_slice(&name_bytes[..name_len]);

    let mut ext = [b' '; 3];
    let ext_bytes = args[2].to_ascii_uppercase().into_bytes();
    let ext_len = min(3, ext_bytes.len());
    ext[..ext_len].copy_from_slice(&ext_bytes[..ext_len]);

    let data_string = args[3..].join(" ");
    let data_bytes = data_string.as_bytes();

    let fs_lock = FS.lock();
    if let Some(fs) = fs_lock.as_ref() {
        match fs.write_new_file(filename, ext, data_bytes) {
            Ok(_) => Ok(Success {
                success_code: format!(
                    "Created {}.{} ({} bytes)",
                    args[1],
                    args[2],
                    data_bytes.len()
                ),
                print_code: true,
            }),
            Err(e) => Err(ProcessError {
                error_code: format!("FS Error: {}", e),
            }),
        }
    } else {
        Err(ProcessError {
            error_code: "File system not mounted! Did you run fs::init()?".to_string(),
        })
    }
}

fn format_disk(args: Vec<String>) -> Result<Success, ProcessError> {
    match crate::fat16::Fat16FileSystem::format_drive() {
        Ok(_) => {
            return Ok(Success {
                success_code: "format worked!".to_string(),
                print_code: true,
            });
        }
        Err(e) => {
            return Err(ProcessError {
                error_code: "failed to format!".to_string(),
            });
        }
    }
}

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

fn edit(args: Vec<String>) -> Result<Success, ProcessError> {
    if args.len() != 4 {
        return Err(ProcessError {
            error_code: "Usage: edit <name> <ext> <data>".to_string(),
        });
    }

    let mut filename = [b' '; 8];
    let name_bytes = args[1].to_ascii_uppercase().into_bytes();
    let name_len = min(8, name_bytes.len());
    filename[..name_len].copy_from_slice(&name_bytes[..name_len]);

    let mut ext = [b' '; 3];
    let ext_bytes = args[2].to_ascii_uppercase().into_bytes();
    let ext_len = min(3, ext_bytes.len());
    ext[..ext_len].copy_from_slice(&ext_bytes[..ext_len]);

    let fs = FS.lock();

    if let Some(a) = fs.as_ref() {
        match a.overwrite_file(filename, ext, args[3].as_bytes()) {
            Ok(_) => Ok(Success {
                success_code: "Worked".to_string(),
                print_code: false,
            }),
            Err(e) => Err(ProcessError {
                error_code: e.to_string(),
            }),
        }
    } else {
        Err(ProcessError {
            error_code: "File system not mounted!".to_string(),
        })
    }
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
    c.insert(String::from("formatd"), Arc::new(format_disk));
    c.insert(String::from("mkdir"), Arc::new(make_file));
    c.insert(String::from("cat"), Arc::new(cat_file));
    c.insert(String::from("run"), Arc::new(run_file));
    c.insert(String::from("edit"), Arc::new(edit));
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
