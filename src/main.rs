#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(myOS::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![allow(unconditional_panic)]

use bootloader::{BootInfo, entry_point};
use core::panic::PanicInfo;
use myOS::memory;
use myOS::memory::translate_addr;
use myOS::println;
use myOS::vga_buffer::WRITER;
use x86_64::{VirtAddr, structures::paging::Translate};

entry_point!(kernel_main);

extern crate alloc;

use alloc::{boxed::Box, rc::Rc, vec, vec::Vec};

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use myOS::allocator;
    use myOS::memory::{self, BootInfoFrameAllocator};
    use myOS::pong::PongGame;
    use x86_64::VirtAddr;

    myOS::init();

    // Setup Memory
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };

    // Setup Heap
    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");

    loop {
        if myOS::interrupts::LAUNCH_PONG.load(core::sync::atomic::Ordering::Relaxed) {
            let mut game = PongGame::new();
            game.run();
            println!("Game ended. Returned to shell.");
            myOS::interrupts::LAUNCH_PONG.swap(false, core::sync::atomic::Ordering::Relaxed);
            x86_64::instructions::hlt();
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
