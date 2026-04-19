#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(myOS::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![allow(unconditional_panic)]

use bootloader::{BootInfo, entry_point};
use core::panic::PanicInfo;
use myOS::{print, println};
use myOS::task::{Task, simple_executor::SimpleExecutor};
use myOS::task::keyboard;
use myOS::task::executor::Executor;
entry_point!(kernel_main);

extern crate alloc;

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use myOS::allocator;
    use myOS::fat16;
    use myOS::fat16::FS;
    use myOS::memory::{self, BootInfoFrameAllocator};
    use myOS::commands::shell_task;
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


    let mut executor = Executor::new();
    executor.spawn(Task::new(keyboard::print_keypresses()));
    executor.spawn(Task::new(shell_task()));
    executor.run();

    
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
