use pc_keyboard::{DecodedKey, KeyCode};

pub async fn run_editor(filename: &str) {
    let mut editor = Editor::new("");

    let mut keyboard = pc_keyboard::Keyboard::new(
        pc_keyboard::ScancodeSet1::new(),
        pc_keyboard::layouts::Us104Key,
        pc_keyboard::HandleControl::Ignore,
    );


    loop {
        let mut needs_redraw = false;

        while let Some(key_event) = crate::input::pop_key() {
            if let Some(key) = keyboard.process_keyevent(key_event) {
                match key {
                    DecodedKey::Unicode(character) => {
                        if character == '\u{1b}' {
                            crate::vga_buffer::clear_screen();
                            return;
                        } else if character == '\u{8}' {
                            editor.backspace();
                        } else {
                            editor.insert_char(character);
                        }
                        needs_redraw = true;
                    }
                    DecodedKey::RawKey(raw_key) => {

                        match raw_key {
                            KeyCode::ArrowLeft => editor.move_cursor_left(),
                            KeyCode::ArrowRight => editor.move_cursor_right(),
                            KeyCode::ArrowUp => editor.move_cursor_up(),
                            KeyCode::ArrowDown => editor.move_cursor_down(),
                            _ => {}
                        }
                        needs_redraw = true;
                    }
                }
            }
        }

        if needs_redraw {
        }

        crate::yield_now().await;
    }
}
