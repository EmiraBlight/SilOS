#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(myOS::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![allow(unconditional_panic)]

use bootloader::{BootInfo, entry_point};
use core::panic::PanicInfo;
use myOS::{hashmap::Hashable, println};

struct key {
    k: u128,
}

impl Hashable for key {
    fn hash(&self) -> usize {
        self.k as usize
    }
}

impl PartialEq for key {
    fn eq(&self, other: &key) -> bool {
        self.k == other.k
    }
}

entry_point!(kernel_main);

extern crate alloc;

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use myOS::allocator;
    use myOS::memory::{self, BootInfoFrameAllocator};
    use x86_64::VirtAddr;

    myOS::init();

    // Setup Memory
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };

    // Setup Heap
    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");

    myOS::commands::init_cmds();

    use myOS::hashmap::HashMap;

    let mut a: HashMap<key, u128> = HashMap::new();

    a.put(key { k: 12 }, 122);
    {
        let res = a.get(key { k: 12 });
        match res {
           Some(res) => println!("Result was: {}",*res),
           None => println!("No result to print"),
       }
    
    }


    a.remove(key { k: 12 }, 122);

    {
        let res = a.get(key { k: 12 });


       match res {
           Some(res) => println!("Result was: {}",*res),
           None => println!("No result to print"),
       }

        println!("Test: {:?}", res);
    }

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
                if !cmd_str.trim().is_empty() {
                    myOS::commands::run_cmd(cmd_str);
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
