#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(myOS::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![allow(unconditional_panic)]

use bootloader::{BootInfo, entry_point};
use core::panic::PanicInfo;
use myOS::{print, println};

entry_point!(kernel_main);

extern crate alloc;

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use myOS::allocator;
    use myOS::fat16;
    use myOS::fat16::FS;
    use myOS::memory::{self, BootInfoFrameAllocator};
    use x86_64::VirtAddr;

    myOS::init();

    // Setup Memory
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };

    // Setup Heap
    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");

    fat16::init();

    myOS::commands::init_cmds();
    let arr: [u8; 3] = [12, 12, 15];
    {
        let res = FS
            .lock()
            .as_ref()
            .unwrap()
            .write_new_file(*b"test    ", *b"txt", &arr);

        println!("{:?}", res);
    }

    print!("user: ");

    loop {
        x86_64::instructions::interrupts::disable();

        if myOS::commands::COMMAND_PENDING.swap(false, core::sync::atomic::Ordering::Acquire) {
            x86_64::instructions::interrupts::enable();

            let cmd_opt = {
                let mut shell = myOS::shell::SHELL.lock();
                let cmd = shell.getcmd().clone();
                shell.clear();
                cmd
            };

            if let Some(cmd_str) = cmd_opt {
                if !cmd_str[0].trim().is_empty() {
                    let response = myOS::commands::run_cmd(cmd_str.clone());

                    if let Err(ref error) = response {
                        println!("ERROR: {}", error.error_str())
                    }
                    if let Ok(error) = response {
                        if *error.is_print() {
                            println!("Program succeded with code: {}", error.success_str());
                        }
                    }
                }
            }
        } else {
            x86_64::instructions::interrupts::enable_and_hlt();
        }
    }
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    myOS::hlt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    myOS::hlt_loop();
}
