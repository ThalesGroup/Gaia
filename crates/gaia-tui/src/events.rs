use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, KeyEventKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppEvent {
    Tick,
    Key(KeyEvent),
    Resize(u16, u16),
}

#[derive(Debug)]
pub struct EventHandler {
    tick_rate: Duration,
    last_tick: Instant,
}

impl EventHandler {
    pub fn new(tick_rate: Duration) -> Self {
        Self {
            tick_rate,
            last_tick: Instant::now(),
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> io::Result<AppEvent> {
        loop {
            let timeout = self
                .tick_rate
                .checked_sub(self.last_tick.elapsed())
                .unwrap_or(Duration::ZERO);

            if event::poll(timeout)? {
                match event::read()? {
                    CrosstermEvent::Key(key_event) => {
                        if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
                            return Ok(AppEvent::Key(key_event));
                        }
                    }
                    CrosstermEvent::Resize(width, height) => {
                        return Ok(AppEvent::Resize(width, height));
                    }
                    _ => {}
                }
            }

            if self.last_tick.elapsed() >= self.tick_rate {
                self.last_tick = Instant::now();
                return Ok(AppEvent::Tick);
            }

            // Keep waiting until a meaningful event arrives or tick elapses.
        }
    }
}
