//! Ratatui rendering for the TUI client.
//!
//! Stateless draw functions for the chat, file tree, code editor and system
//! status views; the app supplies state, this module supplies pixels.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

use super::app::{App, AppMode};

/// Compute a color for the current provider badge.
fn provider_color(provider: &str) -> Color {
    match provider.to_lowercase().as_str() {
        "anthropic" => Color::Rgb(212, 167, 98),   // warm amber
        "gemini" => Color::Rgb(66, 133, 244),      // google blue
        "openai" => Color::Rgb(16, 163, 127),      // openai green
        "qwen" => Color::Rgb(97, 56, 212),         // purple
        "kimi" => Color::Rgb(251, 113, 133),       // rose
        "deepseek" => Color::Rgb(34, 211, 238),    // cyan
        "groq" => Color::Rgb(251, 146, 60),        // orange
        "mistral" => Color::Rgb(234, 179, 8),      // yellow
        "openrouter" => Color::Rgb(167, 139, 250), // violet
        "ollama" => Color::Rgb(148, 163, 184),     // slate
        _ => Color::Gray,
    }
}

/// Render the complete TUI frame.
pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // ---- Top Bar ----
    let top_bar = render_top_bar(app);
    frame.render_widget(top_bar, Rect::new(area.x, area.y, area.width, 1));

    // ---- Bottom Bar (input) ----
    let bottom_bar = render_bottom_bar(app);
    frame.render_widget(
        bottom_bar,
        Rect::new(
            area.x,
            area.y + area.height.saturating_sub(3),
            area.width,
            3,
        ),
    );

    // ---- Main content area ----
    let main_area = Rect::new(
        area.x,
        area.y + 1,
        area.width,
        area.height.saturating_sub(4),
    );

    match app.mode {
        AppMode::Chat => render_chat_view(frame, app, main_area),
        AppMode::FileTree => render_filetree_view(frame, app, main_area),
        AppMode::CodeView => render_code_view(frame, app, main_area),
        AppMode::SystemStatus => render_system_view(frame, app, main_area),
        AppMode::Connect => render_connect_view(frame, app, main_area),
        AppMode::SecurePrompt => render_secure_prompt_view(frame, app, main_area),
        AppMode::ProviderPicker => {
            render_chat_view(frame, app, main_area);
            render_provider_picker(frame, app, main_area);
        }
        AppMode::ModelPicker => {
            render_chat_view(frame, app, main_area);
            render_model_picker(frame, app, main_area);
        }
    }
}

/// Top bar: DIAD Agent title + active provider·model pill + connection status.
fn render_top_bar(app: &App) -> Paragraph<'static> {
    let status = if app.connected {
        "● CONNECTED"
    } else {
        "○ OFFLINE"
    };
    let status_color = if app.connected {
        Color::Green
    } else {
        Color::Yellow
    };

    let provider_pill = if app.model.is_empty() {
        app.provider.to_uppercase()
    } else {
        format!("{} · {}", app.provider.to_uppercase(), app.model)
    };

    let line = Line::from(vec![
        Span::styled(
            " DIAD Agent ",
            Style::default().fg(Color::Black).bg(Color::Cyan).bold(),
        ),
        Span::raw(" │ "),
        Span::styled(
            format!(" {provider_pill} "),
            Style::default()
                .fg(Color::Black)
                .bg(provider_color(&app.provider)),
        ),
        Span::raw(" │ "),
        Span::styled(
            format!(" {} ", app.mode),
            Style::default().fg(Color::White).bg(Color::DarkGray),
        ),
        Span::raw(" │ "),
        Span::styled(status, Style::default().fg(status_color)),
    ]);

    Paragraph::new(line).style(Style::default().bg(Color::Rgb(30, 30, 30)))
}

/// Bottom bar: input field with cursor (or masked secret/confirmation prompt).
fn render_bottom_bar(app: &App) -> Paragraph<'_> {
    if app.mode == AppMode::Connect {
        let prompt = format!(" Paste API key for {} (hidden): ", app.connect_target);
        let masked: String = app.connect_input.chars().map(|_| '•').collect();
        let line = Line::from(vec![
            Span::styled(prompt, Style::default().fg(Color::Yellow)),
            Span::styled(masked, Style::default().fg(Color::White)),
            Span::styled(
                if app.connect_input.is_empty() {
                    "▊"
                } else {
                    ""
                },
                Style::default().fg(Color::Cyan),
            ),
        ]);

        let block = Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::Yellow));

        return Paragraph::new(line)
            .block(block)
            .style(Style::default().fg(Color::White));
    }

    // ---- SecurePrompt mode (masked secret input) ----
    if app.mode == AppMode::SecurePrompt {
        // Confirmation prompt: show the label and wait for y/n
        if let Some(confirm) = &app.confirmation_prompt {
            let line = Line::from(vec![
                Span::styled(
                    format!(" {}\u{00a0} ", confirm.label),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(" [y/N] ", Style::default().fg(Color::Cyan)),
            ]);

            let block = Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(Color::Yellow));

            return Paragraph::new(line)
                .block(block)
                .style(Style::default().fg(Color::White));
        }

        // Masked secret input
        if let Some(prompt) = &app.secret_prompt {
            let masked: String = prompt.buffer.chars().map(|_| '•').collect();
            let line = Line::from(vec![
                Span::styled(
                    format!(" {} ", prompt.label),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(masked, Style::default().fg(Color::White)),
                Span::styled(
                    if prompt.buffer.is_empty() { "▊" } else { "" },
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    " (Enter to confirm, Esc to cancel)",
                    Style::default().fg(Color::DarkGray),
                ),
            ]);

            let block = Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(Color::Yellow));

            return Paragraph::new(line)
                .block(block)
                .style(Style::default().fg(Color::White));
        }

        // No active prompt — fall back to chat input
    }

    let text = if app.input_buffer.is_empty() {
        if app.offline_flow == Some(super::app::OfflineFlow::WifiSelect) {
            Line::from(vec![Span::styled(
                " Type the Wi-Fi SSID to connect to, then press Enter (Esc to cancel):",
                Style::default().fg(Color::Yellow).italic(),
            )])
        } else {
            Line::from(vec![Span::styled(
                " Type your intent here... (Ctrl+D to quit)",
                Style::default().fg(Color::DarkGray).italic(),
            )])
        }
    } else {
        // Build the input line with cursor indicator
        let mut spans: Vec<Span> = Vec::new();
        let input = &app.input_buffer;

        // Text before cursor
        if app.cursor_position > 0 {
            spans.push(Span::raw(&input[..app.cursor_position]));
        }

        // Cursor indicator
        let cursor_char = if app.cursor_position < input.len() {
            input.chars().nth(app.cursor_position).unwrap_or(' ')
        } else {
            ' '
        };
        spans.push(Span::styled(
            cursor_char.to_string(),
            Style::default().fg(Color::Black).bg(Color::Cyan),
        ));

        // Text after cursor
        if app.cursor_position < input.len() {
            spans.push(Span::raw(&input[app.cursor_position + 1..]));
        }

        Line::from(spans)
    };

    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(Color::DarkGray));

    Paragraph::new(text)
        .block(block)
        .style(Style::default().fg(Color::White))
}

/// Chat view: message list (newest at bottom).
fn render_chat_view(frame: &mut Frame, app: &App, area: Rect) {
    let visible = area.height.saturating_sub(2) as usize;
    let skip = app.messages.len().saturating_sub(visible);
    let messages: Vec<ListItem> = app
        .messages
        .iter()
        .skip(skip)
        .map(|msg| {
            let styled = if msg.starts_with("[You]") {
                Line::from(Span::styled(msg.clone(), Style::default().fg(Color::Cyan)))
            } else if msg.starts_with("[Error]") {
                Line::from(Span::styled(msg.clone(), Style::default().fg(Color::Red)))
            } else if msg.starts_with("[Tool Result]") {
                Line::from(Span::styled(msg.clone(), Style::default().fg(Color::Green)))
            } else if msg.starts_with("[tool]") {
                // Dim system line for tool executions
                Line::from(Span::styled(
                    msg.clone(),
                    Style::default().fg(Color::Blue).italic(),
                ))
            } else if msg.starts_with("[local]") {
                // Dim system line for offline intent execution
                Line::from(Span::styled(
                    msg.clone(),
                    Style::default().fg(Color::DarkGray).italic(),
                ))
            } else if msg.starts_with("[Assistant]") || msg.starts_with('[') {
                Line::from(Span::styled(msg.clone(), Style::default().fg(Color::White)))
            } else {
                Line::from(Span::raw(msg.clone()))
            };
            ListItem::new(styled)
        })
        .collect();

    let list = List::new(messages)
        .block(Block::default().borders(Borders::NONE))
        .style(Style::default());

    frame.render_widget(list, area);
}

/// File tree view (placeholder).
fn render_filetree_view(frame: &mut Frame, _app: &App, area: Rect) {
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    // Left: file tree placeholder
    let tree_block = Block::default()
        .title(" File Tree ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let tree_items = vec![
        ListItem::new("📁 ~/"),
        ListItem::new("  📁 Documents/"),
        ListItem::new("  📁 Downloads/"),
        ListItem::new("  📁 .config/"),
        ListItem::new("  📁 .diad/"),
        ListItem::new("  📄 .bashrc"),
    ];

    let tree_list = List::new(tree_items).block(tree_block);
    frame.render_widget(tree_list, layout[0]);

    // Right: file preview placeholder
    let preview_block = Block::default()
        .title(" Preview ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let preview_text = Paragraph::new("Select a file to preview...\n\nUse the input bar to search or navigate.\nType a path to jump to a directory.")
        .block(preview_block)
        .wrap(Wrap { trim: true });

    frame.render_widget(preview_text, layout[1]);
}

/// Code view (placeholder).
fn render_code_view(frame: &mut Frame, _app: &App, area: Rect) {
    let code_block = Block::default()
        .title(" Code View ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let code_text = Paragraph::new(
        "// No file open.\n// Use the input bar to open a file.\n// Type 'open <path>' or describe what you want to edit.\n\n// The agent will show code here with syntax highlighting\n// when you open or create a file.",
    )
    .block(code_block)
    .style(Style::default().fg(Color::White))
    .wrap(Wrap { trim: false });

    frame.render_widget(code_text, area);
}

/// System status view.
fn render_system_view(frame: &mut Frame, app: &App, area: Rect) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(area);

    let info_lines = [
        ("OS", "DIAD (Arch Linux)"),
        ("Kernel", env!("CARGO_PKG_VERSION")), // placeholder
        ("Provider", &app.provider),
        (
            "Connection",
            if app.connected {
                "Connected"
            } else {
                "Offline"
            },
        ),
        ("Session Dir", "~/.diad/sessions/"),
    ];

    for (i, (label, value)) in info_lines.iter().enumerate() {
        let block = Block::default()
            .borders(Borders::LEFT)
            .border_style(Style::default().fg(Color::DarkGray));

        let line = Line::from(vec![
            Span::styled(
                format!(" {label:>12}  "),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(*value, Style::default().fg(Color::White)),
        ]);

        let para = Paragraph::new(line).block(block);
        frame.render_widget(para, layout[i]);
    }

    // Help panel
    let help_block = Block::default()
        .title(" Help ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let help_text = Text::from(vec![
        Line::from(vec![
            Span::styled("F1", Style::default().fg(Color::Cyan).bold()),
            Span::raw(" Chat    "),
            Span::styled("F2", Style::default().fg(Color::Cyan).bold()),
            Span::raw(" File Tree    "),
            Span::styled("F3", Style::default().fg(Color::Cyan).bold()),
            Span::raw(" Code View    "),
            Span::styled("F4", Style::default().fg(Color::Cyan).bold()),
            Span::raw(" System"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Ctrl+C/D", Style::default().fg(Color::Cyan).bold()),
            Span::raw(" Quit    "),
            Span::styled("Ctrl+L", Style::default().fg(Color::Cyan).bold()),
            Span::raw(" Clear chat    "),
            Span::styled("Esc", Style::default().fg(Color::Cyan).bold()),
            Span::raw(" Clear input"),
        ]),
    ]);

    let help_para = Paragraph::new(help_text).block(help_block);
    frame.render_widget(help_para, layout[5]);
}

/// Connect view: API key entry screen.
fn render_connect_view(frame: &mut Frame, app: &App, area: Rect) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(area);

    let title_block = Block::default().borders(Borders::NONE);
    let title = Paragraph::new(Line::from(vec![Span::styled(
        format!(" Connect to {} ", app.connect_target),
        Style::default().fg(Color::Yellow).bold(),
    )]))
    .block(title_block)
    .centered();
    frame.render_widget(title, layout[0]);

    let info_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Instructions ");
    let info = Paragraph::new(vec![
        Line::from(""),
        Line::from(vec![Span::raw(
            "  Your API key is stored securely in the system keyring (libsecret),",
        )]),
        Line::from(vec![
            Span::raw("  in the "),
            Span::styled(
                format!("'{}'", app.connect_target),
                Style::default().fg(Color::Cyan).bold(),
            ),
            Span::raw(" slot."),
        ]),
        Line::from(vec![
            Span::raw("  It is "),
            Span::styled("never", Style::default().fg(Color::Red).bold()),
            Span::raw(" written to disk in plaintext or logged."),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("  Press "),
            Span::styled("Enter", Style::default().fg(Color::Cyan).bold()),
            Span::raw(" to save and validate, "),
            Span::styled("Esc", Style::default().fg(Color::Cyan).bold()),
            Span::raw(" to cancel."),
        ]),
    ])
    .block(info_block)
    .wrap(Wrap { trim: true });
    frame.render_widget(info, layout[1]);

    // Show the rest of the chat messages below
    let visible = layout[2].height.saturating_sub(2) as usize;
    let skip = app.messages.len().saturating_sub(visible);
    let msg_items: Vec<ListItem> = app
        .messages
        .iter()
        .skip(skip)
        .map(|msg| {
            let styled = if msg.starts_with("[You]") {
                Line::from(Span::styled(msg.clone(), Style::default().fg(Color::Cyan)))
            } else if msg.starts_with("[Error]") {
                Line::from(Span::styled(msg.clone(), Style::default().fg(Color::Red)))
            } else {
                Line::from(Span::styled(
                    msg.clone(),
                    Style::default().fg(Color::DarkGray),
                ))
            };
            ListItem::new(styled)
        })
        .collect();
    let list = List::new(msg_items).block(Block::default().borders(Borders::NONE));
    frame.render_widget(list, layout[2]);
}

/// SecurePrompt view: shows the prompt label and the chat history below.
/// The actual masked input is rendered in the bottom bar.
fn render_secure_prompt_view(frame: &mut Frame, app: &App, area: Rect) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(0)])
        .split(area);

    let title_block = Block::default().borders(Borders::NONE);
    let title_text = if let Some(confirm) = &app.confirmation_prompt {
        format!(" Confirm: {}", confirm.label)
    } else if let Some(prompt) = &app.secret_prompt {
        format!(" Secure Prompt: {}", prompt.label)
    } else {
        " Secure Prompt".to_string()
    };
    let title = Paragraph::new(Line::from(vec![Span::styled(
        title_text,
        Style::default().fg(Color::Yellow).bold(),
    )]))
    .block(title_block);
    frame.render_widget(title, layout[0]);

    // Show the chat history below the prompt title
    let visible = layout[1].height.saturating_sub(2) as usize;
    let skip = app.messages.len().saturating_sub(visible);
    let msg_items: Vec<ListItem> = app
        .messages
        .iter()
        .skip(skip)
        .map(|msg| {
            let styled = if msg.starts_with("[You]") {
                Line::from(Span::styled(msg.clone(), Style::default().fg(Color::Cyan)))
            } else if msg.starts_with("[Error]") {
                Line::from(Span::styled(msg.clone(), Style::default().fg(Color::Red)))
            } else if msg.starts_with("[local]") {
                Line::from(Span::styled(
                    msg.clone(),
                    Style::default().fg(Color::DarkGray).italic(),
                ))
            } else if msg.starts_with("[tool]") {
                Line::from(Span::styled(
                    msg.clone(),
                    Style::default().fg(Color::Blue).italic(),
                ))
            } else {
                Line::from(Span::styled(
                    msg.clone(),
                    Style::default().fg(Color::DarkGray),
                ))
            };
            ListItem::new(styled)
        })
        .collect();
    let list = List::new(msg_items).block(Block::default().borders(Borders::NONE));
    frame.render_widget(list, layout[1]);
}

/// Provider picker popup (F6 / `/model`): all configured providers with
/// connection status. Selecting one is resolved by the event loop.
fn render_provider_picker(frame: &mut Frame, app: &App, area: Rect) {
    let popup = centered_rect(64, 70, area);

    if app.provider_entries.is_empty() {
        let block = Block::default()
            .title(" Providers ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        frame.render_widget(
            Paragraph::new("No providers configured.").block(block),
            popup,
        );
        return;
    }

    let items: Vec<ListItem> = app
        .provider_entries
        .iter()
        .map(|e| {
            let status = if e.needs_key {
                if e.has_key {
                    "● configured"
                } else {
                    "○ no key"
                }
            } else {
                "✓ no key needed"
            };
            let status_color = if e.has_key {
                Color::Green
            } else {
                Color::Yellow
            };
            let active = if e.name == app.provider { "►" } else { " " };
            ListItem::new(Line::from(vec![
                Span::raw(format!(" {active} ")),
                Span::styled(
                    format!("{:<12}", e.name),
                    Style::default().fg(provider_color(&e.name)).bold(),
                ),
                Span::raw("  "),
                Span::styled(status, Style::default().fg(status_color)),
            ]))
        })
        .collect();

    let block = Block::default()
        .title(" Providers — ↑/↓ select · Enter pick · Esc close ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().bg(Color::Cyan).fg(Color::Black));

    let mut state = ratatui::widgets::ListState::default();
    state.select(Some(app.picker_index.min(app.provider_entries.len() - 1)));
    frame.render_stateful_widget(list, popup, &mut state);
}

/// Model picker popup: shown after choosing a provider. Arrow keys select
/// the active model; Enter applies it.
fn render_model_picker(frame: &mut Frame, app: &App, area: Rect) {
    let popup = centered_rect(56, 50, area);

    if app.model_list.is_empty() {
        let block = Block::default()
            .title(format!(" {} — models ", app.provider))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        frame.render_widget(
            Paragraph::new("No models listed for this provider.").block(block),
            popup,
        );
        return;
    }

    let items: Vec<ListItem> = app
        .model_list
        .iter()
        .map(|m| {
            let active = if *m == app.model { "►" } else { " " };
            ListItem::new(Line::from(vec![
                Span::raw(format!(" {active} ")),
                Span::styled(m.clone(), Style::default().fg(Color::White)),
            ]))
        })
        .collect();

    let block = Block::default()
        .title(format!(" {} — pick a model · Enter apply ", app.provider))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().bg(Color::Cyan).fg(Color::Black));

    let mut state = ratatui::widgets::ListState::default();
    state.select(Some(app.model_index.min(app.model_list.len() - 1)));
    frame.render_stateful_widget(list, popup, &mut state);
}

/// A centered popup rectangle covering `percent_x`/`percent_y` of `area`.
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

    horizontal[1]
}
