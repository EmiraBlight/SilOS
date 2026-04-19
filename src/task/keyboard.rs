use conquer_once::spin::OnceCell;
use crossbeam_queue::ArrayQueue;
use core::{pin::Pin, task::{Poll, Context}};
use futures_util::stream::Stream;

use futures_util::task::AtomicWaker;

static WAKER: AtomicWaker = AtomicWaker::new();

static SCANCODE_QUEUE: OnceCell<ArrayQueue<u8>> = OnceCell::uninit();


static INPUT_QUEUE: OnceCell<ArrayQueue<u8>> = OnceCell::uninit();

pub fn add_processed_char(c: u8) {
    if let Ok(queue) = INPUT_QUEUE.try_get() {
        let _ = queue.push(c); // Ignore overflow for now or handle appropriately
    }
}

use crate::println;
pub(crate) fn add_scancode(scancode: u8) {
    if let Ok(queue) = SCANCODE_QUEUE.try_get() {
        if let Err(_) = queue.push(scancode) {
            println!("WARNING: scancode queue full; dropping keyboard input");
        }
        else{
            WAKER.wake();
        }
    } else {
        println!("WARNING: scancode queue uninitialized");
    }

}

pub struct ScancodeStream {
    _private: (),
}

impl ScancodeStream {
    pub fn new() -> Self {
        SCANCODE_QUEUE.try_init_once(|| ArrayQueue::new(100))
            .expect("ScancodeStream::new should only be called once");
        ScancodeStream { _private: () }
    }
}


impl Stream for ScancodeStream {
    type Item = u8;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<u8>> {
        let queue = SCANCODE_QUEUE
            .try_get()
            .expect("scancode queue not initialized");

        if let Some(scancode) = queue.pop() {
            return Poll::Ready(Some(scancode));
        }

        WAKER.register(cx.waker());

        match queue.pop() {
            Some(scancode) => {
                Poll::Ready(Some(scancode))
            }
            None => {

                Poll::Pending
            }
        }
    }
}

use futures_util::stream::StreamExt;
use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};
use crate::print;
use crate::task::executor::yield_now;

pub async fn print_keypresses() {
    let mut scancodes = ScancodeStream::new();
    let mut keyboard = Keyboard::new(ScancodeSet1::new(), layouts::Us104Key, HandleControl::Ignore);

    while let Some(scancode) = scancodes.next().await {
        // Handle Pong/System keys (Atomic updates are safe in tasks)
        // You can keep your logic for W/S/O/L here, or re-parse key events

        if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
            if let Some(key) = keyboard.process_keyevent(key_event) {
                match key {
                    DecodedKey::Unicode(character) => {
                        if character == '\u{8}' { // Backspace
                            // This is safe to lock now because we are in a task
                            crate::vga_buffer::_backspace();
                            crate::shell::SHELL.lock().backspace();
                        } else if character == '\n' {
                            print!("\n");
                            crate::commands::COMMAND_PENDING.store(true, core::sync::atomic::Ordering::Release);
                        } else {
                            // Standard character handling
                            print!("{}", character);
                            crate::shell::SHELL.lock().add(character as u8);
                        }
                    }
                    DecodedKey::RawKey(_) => {
                        // Handle your Pong controls (W/S/O/L) here based on key_event
                    }
                }
            }
        }
    }
}