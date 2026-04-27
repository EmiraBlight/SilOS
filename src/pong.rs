use crate::alloc::string::ToString;
use crate::canvas::TextCanvas;
use crate::programReturn::{ProcessError, Success};
use crate::vga_buffer::{Color, ColorCode};

// You'll need to import your input queue and the pc_keyboard types
use crate::input::pop_key;
use crate::task::executor::yield_now;
use pc_keyboard::{KeyCode, KeyState};

pub struct PongGame {
    canvas: TextCanvas,
    ball_x: isize,
    ball_y: isize,
    dx: isize,
    dy: isize,
    paddle1_y: isize,
    paddle2_y: isize,
    // Store key states internally instead of using global atomics
    w_pressed: bool,
    s_pressed: bool,
    up_pressed: bool,
    down_pressed: bool,
}

impl PongGame {
    pub fn new() -> Self {
        Self {
            canvas: TextCanvas::new(),
            ball_x: 40,
            ball_y: 12,
            dx: 1,
            dy: 1,
            paddle1_y: 10,
            paddle2_y: 10,
            w_pressed: false,
            s_pressed: false,
            up_pressed: false,
            down_pressed: false,
        }
    }

    pub async fn run(&mut self) -> Result<Success, ProcessError> {
        loop {
            while let Some(key_event) = pop_key() {
                match (key_event.code, key_event.state) {
                    (KeyCode::W, KeyState::Down) => self.w_pressed = true,
                    (KeyCode::W, KeyState::Up) => self.w_pressed = false,

                    (KeyCode::S, KeyState::Down) => self.s_pressed = true,
                    (KeyCode::S, KeyState::Up) => self.s_pressed = false,

                    // Assuming O and L for player 2 based on your previous setup
                    (KeyCode::O, KeyState::Down) => self.up_pressed = true,
                    (KeyCode::O, KeyState::Up) => self.up_pressed = false,

                    (KeyCode::L, KeyState::Down) => self.down_pressed = true,
                    (KeyCode::L, KeyState::Up) => self.down_pressed = false,

                    // Give the user a way to exit gracefully!
                    (KeyCode::Escape, KeyState::Down) => {
                        self.canvas.clear();
                        return Ok(Success {
                            success_code: "Game Exited".to_string(),
                            print_code: true,
                        });
                    }
                    _ => {}
                }
            }

            self.update();

            yield_now().await;

            self.wait();

            // 5. Check win conditions
            if self.ball_x < 1 {
                self.canvas.clear();
                return Ok(Success {
                    success_code: "player 2 wins!".to_string(),
                    print_code: true,
                });
            }

            if self.ball_x > 79 {
                self.canvas.clear();
                return Ok(Success {
                    success_code: "player 1 wins!".to_string(),
                    print_code: true,
                });
            }
        }
    }

    fn wait(&self) {
        for _ in 0..1_000_000 {
            unsafe {
                core::arch::asm!("nop");
            }
        }
    }

    pub fn update(&mut self) {
        let paddle_height = 5;

        // Use internal state variables instead of Atomics
        if self.w_pressed {
            self.paddle1_y = self.paddle1_y.saturating_sub(1);
        }
        if self.s_pressed {
            if self.paddle1_y < 24 - paddle_height {
                self.paddle1_y += 1;
            }
        }

        if self.up_pressed {
            self.paddle2_y = self.paddle2_y.saturating_sub(1);
        }

        if self.down_pressed {
            if self.paddle2_y < 24 - paddle_height {
                self.paddle2_y += 1;
            }
        }

        self.canvas.clear();

        let paddle_x = 2;
        for offset in 0..paddle_height {
            let current_y = self.paddle1_y + offset;
            if current_y >= 0 && current_y < 25 {
                self.canvas.set_char(
                    paddle_x,
                    current_y as usize,
                    b'|',
                    ColorCode::new(Color::Cyan, Color::Black),
                );
            }
        }

        for offset in 0..paddle_height {
            let current_y = self.paddle2_y + offset;
            if current_y >= 0 && current_y < 25 {
                self.canvas.set_char(
                    76,
                    current_y as usize,
                    b'|',
                    ColorCode::new(Color::Cyan, Color::Black),
                );
            }
        }

        self.canvas.set_char(
            self.ball_x as usize,
            self.ball_y as usize,
            b'O',
            ColorCode::new(Color::White, Color::Black),
        );

        self.ball_x += self.dx;
        self.ball_y += self.dy;

        if self.ball_y <= 0 || self.ball_y >= 24 {
            self.dy = -self.dy;
        }

        if (0..5).contains(&self.ball_x)
            && (self.paddle1_y - 4..self.paddle1_y + 4).contains(&self.ball_y)
            && self.dx < 0
        {
            self.dx = -self.dx
        }

        if (76..80).contains(&self.ball_x)
            && (self.paddle2_y - 4..self.paddle2_y + 4).contains(&self.ball_y)
            && self.dx > 0
        {
            self.dx = -self.dx
        }
    }
}
