//! Ratatui rendering for the TUI client.
//!
//! Stateless draw functions for the chat, file tree, code editor and system
//! status views; the app supplies state, this module supplies pixels.
//! Responsive down to compact split panes without size errors.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

use super::app::{App, AppMode};

/// Compute accent color for the active TUI theme (/tuicolor).
/// Default: Brand Crimson Oxide `#B32032`.
pub fn theme_accent_color(theme: &str) -> Color {
    match theme.to_lowercase().as_str() {
        "slate" => Color::Rgb(138, 150, 142),
        "tokyo_night" | "cyber" => Color::Rgb(122, 162, 247),
        "catppuccin" => Color::Rgb(203, 166, 247),
        "nord" => Color::Rgb(136, 192, 208),
        "gruvbox" => Color::Rgb(250, 189, 47),
        "matrix" => Color::Rgb(56, 161, 105),
        "copper" | "amber" => Color::Rgb(217, 119, 6),
        "obsidian" => Color::Rgb(179, 32, 50),
        _ => Color::Rgb(179, 32, 50), // #B32032 Brand Crimson default
    }
}

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
        _ => Color::Rgb(138, 150, 142),
    }
}

/// Render the complete TUI frame with 100% fluid responsiveness.
pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    if area.width < 8 || area.height < 2 {
        return;
    }

    // Adaptive height allocation
    let show_top = area.height >= 5;
    let top_height: u16 = if show_top { 1 } else { 0 };
    let bottom_height: u16 = if area.height < 7 {
        1
    } else if area.height < 12 {
        2
    } else {
        3
    };

    // ---- Top Bar ----
    if show_top {
        let top_bar = render_top_bar(app, area.width);
        frame.render_widget(top_bar, Rect::new(area.x, area.y, area.width, 1));
    }

    // ---- Bottom Bar (input) ----
    let bottom_bar = render_bottom_bar(app, bottom_height);
    frame.render_widget(
        bottom_bar,
        Rect::new(
            area.x,
            area.y + area.height.saturating_sub(bottom_height),
            area.width,
            bottom_height,
        ),
    );

    // ---- Main content area ----
    let main_height = area.height.saturating_sub(top_height + bottom_height);
    let main_area = Rect::new(area.x, area.y + top_height, area.width, main_height);

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

/// Top bar: responsive DIAD Agent title + active provider·model pill + connection status.
fn render_top_bar(app: &App, width: u16) -> Paragraph<'static> {
    let accent = theme_accent_color(&app.config.theme);
    let status = if app.connected {
        "● CONNECTED"
    } else {
        "○ OFFLINE"
    };
    let status_color = if app.connected {
        Color::Rgb(56, 161, 105) // #38A169 Muted Zinc Green
    } else {
        Color::Rgb(217, 119, 6) // #D97706 Industrial Amber
    };

    let spans = if width < 42 {
        // Ultra-compact: ◈ DIAD | OFFLINE
        vec![
            Span::styled(
                " ◈ DIAD ",
                Style::default().fg(Color::Rgb(229, 234, 230)).bg(accent).bold(),
            ),
            Span::raw(" "),
            Span::styled(status, Style::default().fg(status_color).bold()),
        ]
    } else if width < 68 {
        // Compact: ◈ DIAD | PROVIDER | STATUS
        let prov = app.provider.to_uppercase();
        vec![
            Span::styled(
                " ◈ DIAD ",
                Style::default().fg(Color::Rgb(229, 234, 230)).bg(accent).bold(),
            ),
            Span::raw(" │ "),
            Span::styled(
                format!(" {prov} "),
                Style::default()
                    .fg(Color::Rgb(229, 234, 230))
                    .bg(Color::Rgb(30, 35, 33))
                    .bold(),
            ),
            Span::raw(" │ "),
            Span::styled(status, Style::default().fg(status_color).bold()),
        ]
    } else {
        // Full rich bar
        let provider_pill = if app.model.is_empty() {
            app.provider.to_uppercase()
        } else {
            format!("{} · {}", app.provider.to_uppercase(), app.model)
        };
        vec![
            Span::styled(
                " ◈ DIAD ",
                Style::default().fg(Color::Rgb(229, 234, 230)).bg(accent).bold(),
            ),
            Span::raw(" │ "),
            Span::styled(
                " [F5 GUI STUDIO] ",
                Style::default()
                    .fg(Color::Rgb(9, 10, 10))
                    .bg(Color::Rgb(138, 150, 142))
                    .bold(),
            ),
            Span::raw(" │ "),
            Span::styled(
                format!(" {provider_pill} "),
                Style::default()
                    .fg(Color::Rgb(229, 234, 230))
                    .bg(Color::Rgb(30, 35, 33))
                    .bold(),
            ),
            Span::raw(" │ "),
            Span::styled(
                format!(" {} ", app.mode),
                Style::default()
                    .fg(Color::Rgb(138, 150, 142))
                    .bg(Color::Rgb(22, 25, 24)),
            ),
            Span::raw(" │ "),
            Span::styled(status, Style::default().fg(status_color).bold()),
        ]
    };

    Paragraph::new(Line::from(spans)).style(Style::default().bg(Color::Rgb(15, 17, 16)))
}

/// Bottom bar: input field with cursor (or masked secret/confirmation prompt).
fn render_bottom_bar(app: &App, height: u16) -> Paragraph<'_> {
    if app.mode == AppMode::Connect {
        let prompt = format!(" Paste API key for {} (hidden): ", app.connect_target);
        let masked: String = app.connect_input.chars().map(|_| '•').collect();
        let line = Line::from(vec![
            Span::styled(prompt, Style::default().fg(Color::Rgb(217, 119, 6))),
            Span::styled(masked, Style::default().fg(Color::Rgb(229, 234, 230))),
            Span::styled(
                if app.connect_input.is_empty() { "▊" } else { "" },
                Style::default().fg(Color::Rgb(179, 32, 50)),
            ),
        ]);

        let mut p = Paragraph::new(line).style(Style::default().bg(Color::Rgb(15, 17, 16)));
        if height > 1 {
            p = p.block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_style(Style::default().fg(Color::Rgb(32, 38, 35))),
            );
        }
        return p;
    }

    // ---- SecurePrompt mode ----
    if app.mode == AppMode::SecurePrompt {
        if let Some(confirm) = &app.confirmation_prompt {
            let line = Line::from(vec![
                Span::styled(
                    format!(" {}\u{00a0} ", confirm.label),
                    Style::default().fg(Color::Rgb(217, 119, 6)),
                ),
                Span::styled(" [y/N] ", Style::default().fg(Color::Rgb(179, 32, 50)).bold()),
            ]);

            let mut p = Paragraph::new(line).style(Style::default().bg(Color::Rgb(15, 17, 16)));
            if height > 1 {
                p = p.block(
                    Block::default()
                        .borders(Borders::TOP)
                        .border_style(Style::default().fg(Color::Rgb(32, 38, 35))),
                );
            }
            return p;
        }

        if let Some(prompt) = &app.secret_prompt {
            let masked: String = prompt.buffer.chars().map(|_| '•').collect();
            let line = Line::from(vec![
                Span::styled(
                    format!(" {} ", prompt.label),
                    Style::default().fg(Color::Rgb(217, 119, 6)),
                ),
                Span::styled(masked, Style::default().fg(Color::Rgb(229, 234, 230))),
                Span::styled(
                    if prompt.buffer.is_empty() { "▊" } else { "" },
                    Style::default().fg(Color::Rgb(179, 32, 50)),
                ),
            ]);

            let mut p = Paragraph::new(line).style(Style::default().bg(Color::Rgb(15, 17, 16)));
            if height > 1 {
                p = p.block(
                    Block::default()
                        .borders(Borders::TOP)
                        .border_style(Style::default().fg(Color::Rgb(32, 38, 35))),
                );
            }
            return p;
        }
    }

    let text = if app.input_buffer.is_empty() {
        if app.offline_flow == Some(super::app::OfflineFlow::WifiSelect) {
            Line::from(vec![
                Span::styled(
                    "✦ Wi-Fi ❯ ",
                    Style::default().fg(Color::Rgb(217, 119, 6)).bold(),
                ),
                Span::styled(
                    "Type SSID name to connect, then press Enter",
                    Style::default().fg(Color::Rgb(138, 150, 142)),
                ),
            ])
        } else {
            Line::from(vec![
                Span::styled("✦ DIAD ❯ ", Style::default().fg(Color::Rgb(179, 32, 50)).bold()),
                Span::styled(
                    "Type intent or command... (e.g. 'connect wifi', /help)",
                    Style::default().fg(Color::Rgb(138, 150, 142)),
                ),
            ])
        }
    } else {
        let mut spans: Vec<Span> = Vec::new();
        spans.push(Span::styled(
            "✦ DIAD ❯ ",
            Style::default().fg(Color::Rgb(179, 32, 50)).bold(),
        ));
        let input = &app.input_buffer;

        if app.cursor_position > 0 && app.cursor_position <= input.len() {
            spans.push(Span::styled(
                &input[..app.cursor_position],
                Style::default().fg(Color::Rgb(229, 234, 230)),
            ));
        }

        let cursor_char = if app.cursor_position < input.len() {
            input.chars().nth(app.cursor_position).unwrap_or(' ')
        } else {
            ' '
        };
        let accent = theme_accent_color(&app.config.theme);
        spans.push(Span::styled(
            cursor_char.to_string(),
            Style::default().fg(Color::Black).bg(accent).bold(),
        ));

        if app.cursor_position < input.len() {
            spans.push(Span::styled(
                &input[app.cursor_position + 1..],
                Style::default().fg(Color::Rgb(229, 234, 230)),
            ));
        }

        Line::from(spans)
    };

    let mut p = Paragraph::new(text).style(Style::default().bg(Color::Rgb(15, 17, 16)));
    if height > 1 {
        p = p.block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(Color::Rgb(32, 38, 35))),
        );
    }
    p
}

/// Chat view: message list with wrapping.
fn render_chat_view(frame: &mut Frame, app: &App, area: Rect) {
    let visible = area.height.saturating_sub(1) as usize;
    let skip = app.messages.len().saturating_sub(visible);
    let accent = theme_accent_color(&app.config.theme);
    let messages: Vec<ListItem> = app
        .messages
        .iter()
        .skip(skip)
        .map(|msg| {
            let styled = if let Some(content) = msg.strip_prefix("[You] ") {
                Line::from(vec![
                    Span::styled(
                        "◈ USER ",
                        Style::default().fg(Color::White).bg(accent).bold(),
                    ),
                    Span::raw(" "),
                    Span::styled(
                        content.to_string(),
                        Style::default().fg(Color::Rgb(229, 234, 230)).bold(),
                    ),
                ])
            } else if msg.starts_with("[You]") {
                Line::from(Span::styled(
                    msg.clone(),
                    Style::default().fg(accent).bold(),
                ))
            } else if let Some(content) = msg.strip_prefix("[Assistant] ") {
                Line::from(vec![
                    Span::styled(
                        "❖ DIAD ",
                        Style::default()
                            .fg(Color::Rgb(229, 234, 230))
                            .bg(Color::Rgb(30, 35, 33))
                            .bold(),
                    ),
                    Span::raw(" "),
                    Span::styled(
                        content.to_string(),
                        Style::default().fg(Color::Rgb(229, 234, 230)),
                    ),
                ])
            } else if let Some(content) = msg.strip_prefix("[Tool Result] ") {
                Line::from(vec![
                    Span::styled(
                        "✓ SUCCESS ",
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Rgb(56, 161, 105))
                            .bold(),
                    ),
                    Span::raw(" "),
                    Span::styled(
                        content.to_string(),
                        Style::default().fg(Color::Rgb(167, 243, 208)),
                    ),
                ])
            } else if let Some(content) = msg.strip_prefix("[Error] ") {
                Line::from(vec![
                    Span::styled(
                        "✗ ERROR ",
                        Style::default()
                            .fg(Color::White)
                            .bg(Color::Rgb(179, 32, 50))
                            .bold(),
                    ),
                    Span::raw(" "),
                    Span::styled(
                        content.to_string(),
                        Style::default().fg(Color::Rgb(254, 202, 202)),
                    ),
                ])
            } else if msg.starts_with("[tool]") {
                Line::from(Span::styled(
                    msg.clone(),
                    Style::default().fg(Color::Rgb(138, 150, 142)).italic(),
                ))
            } else if msg.starts_with("[local]") || msg.starts_with("[system]") {
                Line::from(Span::styled(
                    msg.clone(),
                    Style::default().fg(Color::Rgb(138, 150, 142)).italic(),
                ))
            } else {
                Line::from(Span::styled(
                    msg.clone(),
                    Style::default().fg(Color::Rgb(229, 234, 230)),
                ))
            };
            ListItem::new(styled)
        })
        .collect();

    let list = List::new(messages)
        .block(Block::default().borders(Borders::NONE))
        .style(Style::default().bg(Color::Rgb(15, 17, 16)));

    frame.render_widget(list, area);
}

/// File tree view (placeholder).
fn render_filetree_view(frame: &mut Frame, _app: &App, area: Rect) {
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    let tree_block = Block::default()
        .title(" File Tree ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(32, 38, 35)));

    let tree_items = vec![
        ListItem::new("📁 ~/"),
        ListItem::new("  📁 Documents/"),
        ListItem::new("  📁 Downloads/"),
        ListItem::new("  📁 .config/"),
        ListItem::new("  📁 .diad/"),
        ListItem::new("  📄 .bashrc"),
    ];

    let tree_list = List::new(tree_items)
        .block(tree_block)
        .style(Style::default().bg(Color::Rgb(15, 17, 16)).fg(Color::Rgb(229, 234, 230)));
    frame.render_widget(tree_list, layout[0]);

    let preview_block = Block::default()
        .title(" Preview ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(32, 38, 35)));

    let preview_text = Paragraph::new("Select a file to preview...\n\nUse the input bar to search or navigate.\nType a path to jump to a directory.")
        .block(preview_block)
        .style(Style::default().bg(Color::Rgb(15, 17, 16)).fg(Color::Rgb(138, 150, 142)))
        .wrap(Wrap { trim: true });

    frame.render_widget(preview_text, layout[1]);
}

/// Code view (placeholder).
fn render_code_view(frame: &mut Frame, _app: &App, area: Rect) {
    let code_block = Block::default()
        .title(" Code View ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(32, 38, 35)));

    let code_text = Paragraph::new(
        "// No file open.\n// Use the input bar to open a file.\n// Type 'open <path>' or describe what you want to edit.\n\n// The agent will show code here with syntax highlighting\n// when you open or create a file.",
    )
    .block(code_block)
    .style(Style::default().bg(Color::Rgb(15, 17, 16)).fg(Color::Rgb(229, 234, 230)))
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
        ("Kernel", env!("CARGO_PKG_VERSION")),
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
            .border_style(Style::default().fg(Color::Rgb(179, 32, 50)));

        let line = Line::from(vec![
            Span::styled(
                format!(" {label:>12}  "),
                Style::default().fg(Color::Rgb(138, 150, 142)),
            ),
            Span::styled(*value, Style::default().fg(Color::Rgb(229, 234, 230))),
        ]);

        let para = Paragraph::new(line)
            .block(block)
            .style(Style::default().bg(Color::Rgb(15, 17, 16)));
        frame.render_widget(para, layout[i]);
    }

    let help_block = Block::default()
        .title(" Help ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(32, 38, 35)));

    let help_text = Text::from(vec![
        Line::from(vec![
            Span::styled("F1", Style::default().fg(Color::Rgb(179, 32, 50)).bold()),
            Span::raw(" Chat    "),
            Span::styled("F2", Style::default().fg(Color::Rgb(179, 32, 50)).bold()),
            Span::raw(" File Tree    "),
            Span::styled("F3", Style::default().fg(Color::Rgb(179, 32, 50)).bold()),
            Span::raw(" Code View    "),
            Span::styled("F4", Style::default().fg(Color::Rgb(179, 32, 50)).bold()),
            Span::raw(" System"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Ctrl+C/D", Style::default().fg(Color::Rgb(179, 32, 50)).bold()),
            Span::raw(" Quit    "),
            Span::styled("Ctrl+L", Style::default().fg(Color::Rgb(179, 32, 50)).bold()),
            Span::raw(" Clear chat    "),
            Span::styled("Esc", Style::default().fg(Color::Rgb(179, 32, 50)).bold()),
            Span::raw(" Clear input"),
        ]),
    ]);

    let help_para = Paragraph::new(help_text)
        .block(help_block)
        .style(Style::default().bg(Color::Rgb(15, 17, 16)).fg(Color::Rgb(229, 234, 230)));
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

    let title = Paragraph::new(Line::from(vec![Span::styled(
        format!(" Connect to {} ", app.connect_target),
        Style::default().fg(Color::Rgb(217, 119, 6)).bold(),
    )]))
    .centered()
    .style(Style::default().bg(Color::Rgb(15, 17, 16)));
    frame.render_widget(title, layout[0]);

    let info_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(32, 38, 35)))
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
                Style::default().fg(Color::Rgb(179, 32, 50)).bold(),
            ),
            Span::raw(" slot."),
        ]),
        Line::from(vec![
            Span::raw("  It is "),
            Span::styled("never", Style::default().fg(Color::Rgb(179, 32, 50)).bold()),
            Span::raw(" written to disk in plaintext or logged."),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("  Press "),
            Span::styled("Enter", Style::default().fg(Color::Rgb(56, 161, 105)).bold()),
            Span::raw(" to save and validate, "),
            Span::styled("Esc", Style::default().fg(Color::Rgb(138, 150, 142)).bold()),
            Span::raw(" to cancel."),
        ]),
    ])
    .block(info_block)
    .style(Style::default().bg(Color::Rgb(15, 17, 16)).fg(Color::Rgb(229, 234, 230)))
    .wrap(Wrap { trim: true });
    frame.render_widget(info, layout[1]);

    let visible = layout[2].height.saturating_sub(2) as usize;
    let skip = app.messages.len().saturating_sub(visible);
    let msg_items: Vec<ListItem> = app
        .messages
        .iter()
        .skip(skip)
        .map(|msg| {
            let styled = if msg.starts_with("[You]") {
                Line::from(Span::styled(msg.clone(), Style::default().fg(Color::Rgb(179, 32, 50))))
            } else if msg.starts_with("[Error]") {
                Line::from(Span::styled(msg.clone(), Style::default().fg(Color::Rgb(179, 32, 50))))
            } else {
                Line::from(Span::styled(
                    msg.clone(),
                    Style::default().fg(Color::Rgb(138, 150, 142)),
                ))
            };
            ListItem::new(styled)
        })
        .collect();
    let list = List::new(msg_items)
        .block(Block::default().borders(Borders::NONE))
        .style(Style::default().bg(Color::Rgb(15, 17, 16)));
    frame.render_widget(list, layout[2]);
}

/// SecurePrompt view.
fn render_secure_prompt_view(frame: &mut Frame, app: &App, area: Rect) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(0)])
        .split(area);

    let title_text = if let Some(confirm) = &app.confirmation_prompt {
        format!(" Confirm: {}", confirm.label)
    } else if let Some(prompt) = &app.secret_prompt {
        format!(" Secure Prompt: {}", prompt.label)
    } else {
        " Secure Prompt".to_string()
    };
    let title = Paragraph::new(Line::from(vec![Span::styled(
        title_text,
        Style::default().fg(Color::Rgb(217, 119, 6)).bold(),
    )]))
    .style(Style::default().bg(Color::Rgb(15, 17, 16)));
    frame.render_widget(title, layout[0]);

    let visible = layout[1].height.saturating_sub(2) as usize;
    let skip = app.messages.len().saturating_sub(visible);
    let msg_items: Vec<ListItem> = app
        .messages
        .iter()
        .skip(skip)
        .map(|msg| {
            let styled = if msg.starts_with("[You]") {
                Line::from(Span::styled(msg.clone(), Style::default().fg(Color::Rgb(179, 32, 50))))
            } else if msg.starts_with("[Error]") {
                Line::from(Span::styled(msg.clone(), Style::default().fg(Color::Rgb(179, 32, 50))))
            } else if msg.starts_with("[local]") {
                Line::from(Span::styled(
                    msg.clone(),
                    Style::default().fg(Color::Rgb(138, 150, 142)).italic(),
                ))
            } else if msg.starts_with("[tool]") {
                Line::from(Span::styled(
                    msg.clone(),
                    Style::default().fg(Color::Rgb(179, 32, 50)).italic(),
                ))
            } else {
                Line::from(Span::styled(
                    msg.clone(),
                    Style::default().fg(Color::Rgb(138, 150, 142)),
                ))
            };
            ListItem::new(styled)
        })
        .collect();
    let list = List::new(msg_items)
        .block(Block::default().borders(Borders::NONE))
        .style(Style::default().bg(Color::Rgb(15, 17, 16)));
    frame.render_widget(list, layout[1]);
}

/// Provider picker popup (F6 / `/model`).
fn render_provider_picker(frame: &mut Frame, app: &App, area: Rect) {
    let popup = centered_rect(64, 70, area);

    if app.provider_entries.is_empty() {
        let block = Block::default()
            .title(" Providers ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(179, 32, 50)));
        frame.render_widget(
            Paragraph::new("No providers configured.")
                .block(block)
                .style(Style::default().bg(Color::Rgb(22, 25, 24)).fg(Color::Rgb(229, 234, 230))),
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
                Color::Rgb(56, 161, 105)
            } else {
                Color::Rgb(217, 119, 6)
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
        .border_style(Style::default().fg(Color::Rgb(179, 32, 50)));

    let list = List::new(items)
        .block(block)
        .style(Style::default().bg(Color::Rgb(22, 25, 24)).fg(Color::Rgb(229, 234, 230)))
        .highlight_style(Style::default().bg(Color::Rgb(179, 32, 50)).fg(Color::White).bold());

    let mut state = ratatui::widgets::ListState::default();
    state.select(Some(app.picker_index.min(app.provider_entries.len() - 1)));
    frame.render_stateful_widget(list, popup, &mut state);
}

/// Model picker popup.
fn render_model_picker(frame: &mut Frame, app: &App, area: Rect) {
    let popup = centered_rect(56, 50, area);

    if app.model_list.is_empty() {
        let block = Block::default()
            .title(format!(" {} — models ", app.provider))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(179, 32, 50)));
        frame.render_widget(
            Paragraph::new("No models listed for this provider.")
                .block(block)
                .style(Style::default().bg(Color::Rgb(22, 25, 24)).fg(Color::Rgb(229, 234, 230))),
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
                Span::styled(m.clone(), Style::default().fg(Color::Rgb(229, 234, 230))),
            ]))
        })
        .collect();

    let block = Block::default()
        .title(format!(" {} — pick a model · Enter apply ", app.provider))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(179, 32, 50)));

    let list = List::new(items)
        .block(block)
        .style(Style::default().bg(Color::Rgb(22, 25, 24)).fg(Color::Rgb(229, 234, 230)))
        .highlight_style(Style::default().bg(Color::Rgb(179, 32, 50)).fg(Color::White).bold());

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
