use crate::canvas::TextCanvas;
use crate::vga_buffer::{Color, ColorCode};

use crate::alloc::string::ToString;
use crate::programReturn::ProcessError;
use crate::programReturn::Success;
use core::sync::atomic::{AtomicBool, Ordering};

pub static W_PRESSED: AtomicBool = AtomicBool::new(false);
pub static S_PRESSED: AtomicBool = AtomicBool::new(false);
pub static UP_PRESSED: AtomicBool = AtomicBool::new(false);
pub static DOWN_PRESSED: AtomicBool = AtomicBool::new(false);

pub struct PongGame {
    canvas: TextCanvas,
    ball_x: isize,
    ball_y: isize,
    dx: isize,
    dy: isize,
    paddle1_y: isize, // Track the left paddle
    paddle2_y: isize,
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
        }
    }

    pub fn run(&mut self) -> Result<Success, ProcessError> {
        loop {
            self.update();
            self.wait();

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

        if W_PRESSED.load(Ordering::Relaxed) {
            self.paddle1_y = self.paddle1_y.saturating_sub(1);
        }
        if S_PRESSED.load(Ordering::Relaxed) {
            if self.paddle1_y < 24 - paddle_height {
                self.paddle1_y += 1;
            }
        }

        if UP_PRESSED.load(Ordering::Relaxed) {
            self.paddle2_y = self.paddle2_y.saturating_sub(1);
        }

        if DOWN_PRESSED.load(Ordering::Relaxed) {
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
