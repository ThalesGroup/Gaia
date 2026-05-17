use ratatui::style::{Color, Modifier, Style};

#[derive(Debug, Clone, Copy)]
pub struct Theme;

impl Theme {
    pub fn background() -> Style {
        Style::default().bg(Color::Black)
    }

    pub fn header() -> Style {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    }

    pub fn block_title() -> Style {
        Style::default()
            .fg(Color::LightCyan)
            .add_modifier(Modifier::BOLD)
    }

    pub fn body_text() -> Style {
        Style::default().fg(Color::Gray)
    }

    pub fn emphasis() -> Style {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    }

    pub fn selected_row() -> Style {
        Style::default()
            .fg(Color::Black)
            .bg(Color::LightCyan)
            .add_modifier(Modifier::BOLD)
    }

    pub fn warning() -> Style {
        Style::default().fg(Color::Yellow)
    }

    pub fn danger() -> Style {
        Style::default().fg(Color::LightRed)
    }

    pub fn success() -> Style {
        Style::default().fg(Color::LightGreen)
    }

    pub fn muted() -> Style {
        Style::default().fg(Color::DarkGray)
    }
}
