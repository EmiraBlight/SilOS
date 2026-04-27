use alloc::collections::VecDeque;
use pc_keyboard::KeyEvent;
use spin::Mutex; // Or whatever key type your decoder outputs

lazy_static::lazy_static! {
    // The global bucket of recent key presses
    pub static ref KEY_EVENT_QUEUE: Mutex<VecDeque<KeyEvent>> = Mutex::new(VecDeque::new());
}

// A simple helper function for programs to pull the next key
pub fn pop_key() -> Option<KeyEvent> {
    KEY_EVENT_QUEUE.lock().pop_front()
}
