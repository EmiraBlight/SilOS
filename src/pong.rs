use crate::canvas::TextCanvas;
use crate::vga_buffer::{Color, ColorCode};

pub struct PongGame {
    canvas: TextCanvas,
    ball_x: isize,
    ball_y: isize,
    dx: isize,
    dy: isize,
}

impl PongGame {
    pub fn new() -> Self {
        Self {
            canvas: TextCanvas::new(),
            ball_x: 40,
            ball_y: 12,
            dx: 1,
            dy: 1,
        }
    }

    pub fn run(&mut self) {
        let mut running = true;
        while running {
            self.update();
            self.wait();

            // Example Exit Condition: Ball goes too far past a paddle
            if self.ball_x < 1 || self.ball_x > 79 {
                running = false;
            }

            // Future: check a global flag for the Escape key here
        }

        // Before returning, clear the screen to leave it "blank"
        self.canvas.clear();
    }
    fn wait(&self) {
        // Crude delay until you implement a proper sleep based on PIT/APIC timers
        for _ in 0..1_000_000 {
            unsafe {
                core::arch::asm!("nop");
            }
        }
    }

    pub fn update(&mut self) {
        // Clear previous frame
        self.canvas.clear();

        // Draw Ball
        self.canvas.set_char(
            self.ball_x as usize,
            self.ball_y as usize,
            b'O',
            ColorCode::new(Color::White, Color::Black),
        );

        // Movement Logic
        self.ball_x += self.dx;
        self.ball_y += self.dy;

        // Bounce Logic
        if self.ball_y <= 0 || self.ball_y >= 24 {
            self.dy = -self.dy;
        }
        if self.ball_x <= 0 || self.ball_x >= 79 {
            self.dx = -self.dx;
        }
    }
}
