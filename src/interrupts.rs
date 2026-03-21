use crate::gdt;
use crate::hlt_loop;
use crate::print;
use crate::println;
use crate::shell::SHELL;
use crate::vga_buffer::WRITER;
use lazy_static::lazy_static;
use pic8259::ChainedPics;
use spin;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

use core::sync::atomic::{AtomicBool, Ordering};

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;
pub static LAUNCH_PONG: AtomicBool = AtomicBool::new(false);

pub static PICS: spin::Mutex<ChainedPics> =
    spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard,
}

impl InterruptIndex {
    fn as_u8(self) -> u8 {
        self as u8
    }

    fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    use pc_keyboard::{DecodedKey, HandleControl, Keyboard, ScancodeSet1, layouts};
    use x86_64::instructions::port::Port;

    lazy_static! {
        static ref KEYBOARD: spin::Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> =
            spin::Mutex::new(Keyboard::new(
                ScancodeSet1::new(),
                layouts::Us104Key,
                HandleControl::Ignore
            ));
    }

    let mut keyboard = KEYBOARD.lock();
    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };

    if scancode == 0x0E {
        crate::vga_buffer::_backspace();
        *&SHELL.lock().backspace();
    } else if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
        if let Some(key) = keyboard.process_keyevent(key_event) {
            match key {
                DecodedKey::Unicode(character) => {
                    if character != '\t'
                        && character != '\n'
                        && !crate::interrupts::LAUNCH_PONG
                            .load(core::sync::atomic::Ordering::Relaxed)
                    {
                        print!("{}", character);
                        *&SHELL.lock().add(character as u8);
                    } else if character == '\n' {
                        print!("\n");

                        match &*&SHELL.lock().getcmd() {
                            None => (),
                            Some(s) => match s.as_str() {
                                "" => (),
                                "clear" => WRITER.lock().clear(),
                                "pong" => {
                                    LAUNCH_PONG.store(true, Ordering::Relaxed);
                                }
                                a => println!("{} is not a command!", a),
                            },
                        }

                        *&SHELL.lock().clear();
                    }
                }

                DecodedKey::RawKey(_) => {}
            }
        }

        if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
            use pc_keyboard::KeyCode;

            let is_pressed = key_event.state == pc_keyboard::KeyState::Down;

            match key_event.code {
                KeyCode::W => crate::pong::W_PRESSED.store(is_pressed, Ordering::Relaxed),
                KeyCode::S => crate::pong::S_PRESSED.store(is_pressed, Ordering::Relaxed),
                _ => {}
            }
        }
    }

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;

    println!("EXCEPTION: PAGE FAULT");
    println!("Accessed Address: {:?}", Cr2::read());
    println!("Error Code: {:?}", error_code);
    println!("{:#?}", stack_frame);
    hlt_loop();
}

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        unsafe {
            idt.double_fault.set_handler_fn(double_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX); // new
        }
        idt[InterruptIndex::Timer.as_usize()]
                    .set_handler_fn(timer_interrupt_handler);
        idt[InterruptIndex::Keyboard.as_usize()]
                    .set_handler_fn(keyboard_interrupt_handler);
         idt.page_fault.set_handler_fn(page_fault_handler);
        idt
    };
}

pub fn init_idt() {
    IDT.load();
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}
