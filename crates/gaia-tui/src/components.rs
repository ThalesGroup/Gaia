use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

use gaia_core::recommendation::FitStatus;

pub fn footer_help() -> &'static str {
    "Enter: select | /: search | x: clear search | f: filters | b: backend | c: category | s: size | r: refresh catalog | Space: details focus | q: quit"
}

pub fn spinner_frame(tick: usize) -> &'static str {
    const FRAMES: [&str; 4] = ["|", "/", "-", "\\"];
    FRAMES[tick % FRAMES.len()]
}

pub fn fit_badge(fit: FitStatus) -> Span<'static> {
    let (label, color) = match fit {
        FitStatus::Easy => (" easy ", Color::LightGreen),
        FitStatus::Fits => (" fits ", Color::Cyan),
        FitStatus::Tight => (" tight ", Color::Yellow),
        FitStatus::RequiresQuantization => (" quant ", Color::Magenta),
        FitStatus::RequiresMultiGpu => (" multi-gpu ", Color::LightRed),
        FitStatus::NotRecommended => (" risky ", Color::Red),
        FitStatus::Unknown => (" unknown ", Color::Gray),
    };

    Span::styled(
        label,
        Style::default()
            .fg(Color::Black)
            .bg(color)
            .add_modifier(Modifier::BOLD),
    )
}

pub fn boolean_badge(label: &'static str, value: bool) -> Span<'static> {
    if value {
        Span::styled(
            format!(" {label}:ok "),
            Style::default().fg(Color::Black).bg(Color::LightGreen),
        )
    } else {
        Span::styled(
            format!(" {label}:missing "),
            Style::default().fg(Color::White).bg(Color::DarkGray),
        )
    }
}

pub fn compact_model_name(id: &str, max_chars: usize) -> String {
    if id.chars().count() <= max_chars {
        return id.to_owned();
    }

    let mut output = String::new();
    for (index, ch) in id.chars().enumerate() {
        if index + 3 >= max_chars {
            output.push_str("...");
            break;
        }
        output.push(ch);
    }
    output
}
