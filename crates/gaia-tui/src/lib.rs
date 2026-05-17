pub mod app;
pub mod components;
pub mod events;
pub mod theme;
pub mod ui;

use std::io;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::DisableMouseCapture;
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::app::{AppAction, AppState, SelectorInput, SelectorResult};
use crate::events::{AppEvent, EventHandler};

#[derive(Debug, Clone)]
pub enum SelectorOutcome {
    Cancelled,
    Launch(SelectorResult),
}

pub fn run_selector(input: SelectorInput) -> Result<SelectorOutcome> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let outcome = run_app(&mut terminal, input);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    outcome
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    input: SelectorInput,
) -> Result<SelectorOutcome> {
    let mut app = AppState::new(input);
    let mut event_handler = EventHandler::new(Duration::from_millis(120));

    loop {
        terminal.draw(|frame| ui::render(frame, &app))?;

        match event_handler.next()? {
            AppEvent::Tick => app.on_tick(),
            AppEvent::Resize(_, _) => {}
            AppEvent::Key(key_event) => match app.handle_key(key_event) {
                AppAction::None => {}
                AppAction::Cancelled => return Ok(SelectorOutcome::Cancelled),
                AppAction::Confirmed(result) => return Ok(SelectorOutcome::Launch(result)),
            },
        }
    }
}
