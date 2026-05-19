use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap,
};

use crate::app::{AppState, SizeFilter, WizardMode};
use crate::components::{boolean_badge, compact_model_name, fit_badge, footer_help, spinner_frame};
use crate::theme::Theme;

pub fn render(frame: &mut Frame<'_>, app: &AppState) {
    let area = frame.area();
    frame.render_widget(Block::default().style(Theme::background()), area);

    let root_block = Block::default()
        .title(Span::styled(
            " Gaia - LLM serving manager ",
            Theme::header(),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);
    let inner = root_block.inner(area);
    frame.render_widget(root_block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(9),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(inner);

    render_header(frame, chunks[0], app);
    render_top_panels(frame, chunks[1], app);
    render_models(frame, chunks[2], app);
    render_footer(frame, chunks[3], app);
    render_modal(frame, app);
}

fn render_header(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let backend = app.current_backend();
    let model_count = app.models.len();
    let status = format!(
        "{} {}",
        spinner_frame(app.spinner_tick),
        app.status_message.as_str()
    );
    let line = Line::from(vec![
        Span::styled("Backend: ", Theme::block_title()),
        Span::styled(backend.label, Theme::emphasis()),
        Span::raw("   "),
        Span::styled("Visible models: ", Theme::block_title()),
        Span::styled(model_count.to_string(), Theme::emphasis()),
        Span::raw("   "),
        Span::styled(status, Theme::body_text()),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" Session ", Theme::block_title()));
    frame.render_widget(Paragraph::new(line).block(block), area);
}

fn render_top_panels(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(area);

    render_machine_panel(frame, columns[0], app);
    render_backend_panel(frame, columns[1], app);
}

fn render_machine_panel(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let mut lines = Vec::new();
    let gpu_line = if let Some(gpu) = &app.machine.gpu {
        format!("GPU: {} ({:.1} GB)", gpu.name, gpu.vram_gb)
    } else {
        "GPU: none detected".to_owned()
    };
    lines.push(Line::from(vec![
        Span::styled(gpu_line, Theme::emphasis()),
        Span::raw(" "),
        boolean_badge("gpu", app.machine.gpu.is_some()),
    ]));

    lines.push(Line::from(format!(
        "RAM: {:.1} GB",
        app.machine.ram_total_gb
    )));
    lines.push(Line::from(format!("CPU: {} cores", app.machine.cpu_cores)));
    lines.push(Line::from(vec![
        Span::raw("Docker: "),
        Span::styled(
            if app.machine.docker.daemon_accessible {
                "OK"
            } else {
                "missing/locked"
            },
            if app.machine.docker.daemon_accessible {
                Theme::success()
            } else {
                Theme::danger()
            },
        ),
    ]));
    lines.push(Line::from(vec![
        Span::raw("HF_TOKEN: "),
        Span::styled(
            if app.machine.hf_token_present {
                "present"
            } else {
                "missing"
            },
            if app.machine.hf_token_present {
                Theme::success()
            } else {
                Theme::warning()
            },
        ),
    ]));
    if let Some(gpu) = &app.machine.gpu {
        lines.push(Line::from(format!(
            "NVIDIA driver: {}",
            gpu.driver_version.as_deref().unwrap_or("unknown")
        )));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" Machine ", Theme::block_title()));
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .style(Theme::body_text())
            .block(block)
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_backend_panel(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let total_backends = app.backend_choices.len();
    if total_backends == 0 {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(" Backend ", Theme::block_title()));
        frame.render_widget(Paragraph::new("No backend configured.").block(block), area);
        return;
    }

    // The panel is small; each backend currently consumes ~2 lines.
    // Keep a viewport centered around the selected backend so it never disappears.
    let content_height = area.height.saturating_sub(2) as usize;
    let rows_per_backend = 2usize;
    let max_visible_backends = (content_height / rows_per_backend).max(1);
    let selected_index = app.backend_index.min(total_backends.saturating_sub(1));

    let mut start_index = 0usize;
    if total_backends > max_visible_backends {
        let half = max_visible_backends / 2;
        start_index = selected_index.saturating_sub(half);
        let max_start = total_backends - max_visible_backends;
        if start_index > max_start {
            start_index = max_start;
        }
    }
    let end_index = (start_index + max_visible_backends).min(total_backends);

    let mut lines = Vec::new();
    for (index, backend) in app
        .backend_choices
        .iter()
        .enumerate()
        .skip(start_index)
        .take(end_index - start_index)
    {
        let availability = &app.backend_availability[index];
        let selected = index == app.backend_index;
        let marker = if selected { "> " } else { "  " };
        let mut spans = vec![Span::styled(
            marker,
            if selected {
                Theme::emphasis()
            } else {
                Theme::muted()
            },
        )];
        spans.push(Span::styled(
            format!("{:<8}", backend.label),
            if selected {
                Theme::emphasis()
            } else {
                Theme::body_text()
            },
        ));

        if app.is_backend_preferred(backend.id) {
            spans.push(Span::styled(
                " recommended ",
                Style::default().fg(Color::Black).bg(Color::LightGreen),
            ));
        }

        let availability_style = if availability.available {
            Theme::success()
        } else {
            Theme::warning()
        };
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            if availability.available {
                "available"
            } else {
                "limited"
            },
            availability_style,
        ));

        lines.push(Line::from(spans));
        lines.push(Line::from(Span::styled(
            format!("    {}", availability.reason),
            Theme::muted(),
        )));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" Backend ", Theme::block_title()));
    frame.render_widget(Paragraph::new(Text::from(lines)).block(block), area);
}

fn render_models(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(62), Constraint::Percentage(38)])
        .split(area);

    render_model_list(frame, columns[0], app);
    render_model_details(frame, columns[1], app);
}

fn render_model_list(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let title = format!(
        " Models | backend={} | category={} | size={} | search={}{} ",
        app.current_backend().label,
        app.category_label(),
        app.size_label(),
        if app.search_query.is_empty() {
            "none"
        } else {
            app.search_query.as_str()
        },
        if app.search_query.is_empty() {
            ""
        } else {
            " | press x to clear"
        },
    );

    let items = if app.models.is_empty() {
        vec![ListItem::new(Line::from(vec![Span::styled(
            "No model matches the current filters.",
            Theme::warning(),
        )]))]
    } else {
        app.models
            .iter()
            .map(|model| {
                let name = compact_model_name(&model.id, 34);
                let mut spans = vec![
                    Span::raw(format!("{:<36}", name)),
                    Span::styled(format!("{:>5.1}B ", model.params_b), Theme::body_text()),
                    fit_badge(model.fit),
                ];
                if model.recommended {
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(
                        " recommended ",
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::LightGreen)
                            .add_modifier(Modifier::BOLD),
                    ));
                }
                if model.gated {
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(
                        " gated ",
                        Style::default().fg(Color::Black).bg(Color::Yellow),
                    ));
                }

                ListItem::new(Line::from(spans))
            })
            .collect::<Vec<_>>()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(title, Theme::block_title()));
    let list = List::new(items)
        .block(block)
        .highlight_style(Theme::selected_row())
        .highlight_symbol(">> ");

    let mut state = ListState::default();
    if !app.models.is_empty() {
        state.select(Some(app.selected_model_index));
    }
    frame.render_stateful_widget(list, area, &mut state);
}

fn render_model_details(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let lines = if let Some(model) = app.selected_model() {
        vec![
            Line::from(vec![
                Span::styled("Model: ", Theme::block_title()),
                Span::styled(model.display_name.as_str(), Theme::emphasis()),
            ]),
            Line::from(format!("ID: {}", model.id)),
            Line::from(format!("Family: {}", model.family)),
            Line::from(format!("Params: {:.1}B", model.params_b)),
            Line::from(format!("Categories: {}", model.categories.join(", "))),
            Line::from(""),
            Line::from(vec![Span::styled(
                format!("Fit: {}", model.fit.as_badge()),
                Theme::emphasis(),
            )]),
            Line::from(format!("Why: {}", model.explanation)),
            Line::from(format!(
                "VRAM GB (FP16/INT8/INT4): {:.1}/{:.1}/{:.1}",
                model.min_vram_gb_fp16, model.min_vram_gb_int8, model.min_vram_gb_int4
            )),
            Line::from(""),
            Line::from(format!("Use: {}", model.recommended_use)),
            Line::from(format!(
                "Supports: vLLM={} | TGI={} | SGLang={}",
                if model.supports_vllm { "yes" } else { "no" },
                if model.supports_tgi { "yes" } else { "no" },
                if model.supports_sglang { "yes" } else { "no" }
            )),
            Line::from(format!(
                "          llama.cpp={} | Ollama={}",
                if model.supports_llamacpp { "yes" } else { "no" },
                if model.supports_ollama { "yes" } else { "no" }
            )),
            Line::from("HF link: see footer in details focus"),
        ]
    } else {
        vec![Line::from("Select a model to view details.")]
    };

    let title = if app.details_focus {
        " Model details [focus] | Space to return "
    } else {
        " Model details | Space to focus "
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(if app.details_focus {
            Theme::emphasis()
        } else {
            Style::default()
        })
        .title(Span::styled(title, Theme::block_title()));
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .block(block)
            .scroll((app.model_details_scroll, 0))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let help_text = match app.mode {
        WizardMode::Browse => {
            if app.details_focus {
                app.selected_model_hf_url()
                    .unwrap_or_else(|| "No model selected.".to_owned())
            } else {
                footer_help().to_owned()
            }
        }
        WizardMode::Search => "Search: type text | Enter apply | Esc cancel".to_owned(),
        WizardMode::CategoryPicker => {
            "Category filter: Up/Down select | Enter apply | Esc cancel".to_owned()
        }
        WizardMode::SizePicker => {
            "Size filter: Up/Down select | Enter apply | Esc cancel".to_owned()
        }
        WizardMode::PortInput => "Port input: digits | Enter continue | Esc back".to_owned(),
        WizardMode::ApiKeyInput => {
            "API key: Enter continue | Ctrl+g local-key | Esc back".to_owned()
        }
        WizardMode::Confirm => "Confirm: Enter launch | Esc back | q quit".to_owned(),
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" Shortcuts ", Theme::block_title()));
    frame.render_widget(Paragraph::new(help_text).block(block), area);
}

fn render_modal(frame: &mut Frame<'_>, app: &AppState) {
    match app.mode {
        WizardMode::Browse => {}
        WizardMode::Search => {
            render_input_popup(
                frame,
                "Search model",
                "Type a model id, family, or category.",
                app.search_input.as_str(),
            );
        }
        WizardMode::PortInput => {
            render_input_popup(
                frame,
                "API port",
                "OpenAI-compatible API will listen on this port.",
                app.port_input.as_str(),
            );
        }
        WizardMode::ApiKeyInput => {
            render_input_popup(
                frame,
                "API key",
                "Leave empty to use local-key (Ctrl+g).",
                app.api_key_input.as_str(),
            );
        }
        WizardMode::CategoryPicker => {
            render_selection_popup(
                frame,
                "Category filter",
                &app.category_options,
                app.category_picker_index,
            );
        }
        WizardMode::SizePicker => {
            let options = SizeFilter::all()
                .iter()
                .map(|value| value.label().to_owned())
                .collect::<Vec<_>>();
            render_selection_popup(frame, "Size filter", &options, app.size_picker_index);
        }
        WizardMode::Confirm => {
            let model = app.selected_model();
            let model_id = model.map(|item| item.id.as_str()).unwrap_or("-");
            let availability = app.current_backend_availability();
            let content = Text::from(vec![
                Line::from("Ready to launch with this configuration?"),
                Line::from(""),
                Line::from(format!("Backend: {}", app.current_backend().label)),
                Line::from(format!("Model: {model_id}")),
                Line::from(format!("API port: {}", app.port_input)),
                Line::from(format!(
                    "API key: {}",
                    if app.api_key_input.is_empty() {
                        "local-key"
                    } else {
                        app.api_key_input.as_str()
                    }
                )),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Backend status: ", Theme::block_title()),
                    Span::styled(
                        if availability.available {
                            "available"
                        } else {
                            "limited"
                        },
                        if availability.available {
                            Theme::success()
                        } else {
                            Theme::warning()
                        },
                    ),
                ]),
                Line::from(format!("Reason: {}", availability.reason)),
                Line::from(""),
                Line::from("Enter = launch | Esc = back"),
            ]);
            let paragraph = Paragraph::new(content).wrap(Wrap { trim: true }).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(Span::styled(" Confirm launch ", Theme::block_title())),
            );
            let area = centered_rect(68, 60, frame.area());
            frame.render_widget(Clear, area);
            frame.render_widget(paragraph, area);
        }
    }
}

fn render_input_popup(frame: &mut Frame<'_>, title: &str, subtitle: &str, input: &str) {
    let area = centered_rect(60, 34, frame.area());
    let content = Text::from(vec![
        Line::from(subtitle.to_owned()),
        Line::from(""),
        Line::from(vec![
            Span::styled("> ", Theme::emphasis()),
            Span::styled(format!("{input}_"), Theme::emphasis()),
        ]),
    ]);

    let block = Block::default()
        .title(Span::styled(format!(" {title} "), Theme::block_title()))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new(content).block(block), area);
}

fn render_selection_popup(
    frame: &mut Frame<'_>,
    title: &str,
    options: &[String],
    selected_index: usize,
) {
    let area = centered_rect(50, 55, frame.area());
    let items = options
        .iter()
        .map(|item| ListItem::new(Line::from(item.clone())))
        .collect::<Vec<_>>();

    let list = List::new(items)
        .block(
            Block::default()
                .title(Span::styled(format!(" {title} "), Theme::block_title()))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        )
        .highlight_style(Theme::selected_row())
        .highlight_symbol(">> ");

    let mut state = ListState::default();
    if !options.is_empty() {
        state.select(Some(selected_index.min(options.len().saturating_sub(1))));
    }
    frame.render_widget(Clear, area);
    frame.render_stateful_widget(list, area, &mut state);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1]);

    horizontal[1].inner(Margin {
        vertical: 0,
        horizontal: 0,
    })
}
