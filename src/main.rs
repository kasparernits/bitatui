use std::io;
use std::time::{Duration, Instant};

use serde_json;
use std::fs::File;
use std::io::BufReader;

use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

// NEW: for QR
use qrcode::QrCode;
use qrcode::render::unicode;

mod cli;
mod node;

use crate::cli::run_bitcoin_cli;
use crate::node::{fetch_node_info, fetch_wallet_info};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let commands = load_commands_from_json("commands.json")?;

    let mut selected = 0;
    let mut last_input = Instant::now();
    let mut output = String::new();
    let mut output_lines: Vec<String> = Vec::new();
    let mut scroll_offset = 0;

    let _refresh_interval = Duration::from_secs(5);
    let mut last_refresh = Instant::now();

    // For node info refresh
    let mut node_info = String::new();
    // For wallet info refresh
    let mut wallet_info = String::new();

    // NEW: overlay state
    let mut show_qr_overlay = false;
    let mut address = String::from("bc1qfpacvgpjms0eu6mszhwgjjs03yldesmmcgzad0");
    let mut addr_cursor: usize = address.len();

    // Initial fetch
    output = run_bitcoin_cli(&commands[selected])?;
    output_lines = output.lines().map(|l| l.to_string()).collect();

    node_info = fetch_node_info().unwrap_or_else(|_| "Failed to fetch node info".to_string());
    wallet_info = fetch_wallet_info().unwrap_or_else(|_| "Failed to fetch wallet info".to_string());

    loop {
        terminal.draw(|f| {
            let size = f.size();
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
                .split(size);

            // Left panel split vertically into Node Info (7 lines), Wallet Info (7 lines), Commands (rest)
            let left_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(7),
                    Constraint::Length(7),
                    Constraint::Min(0),
                ])
                .split(chunks[0]);

            // Node Info Panel
            let node_info_paragraph = Paragraph::new(node_info.as_str())
                .block(
                    Block::default()
                        .title("Node Info")
                        .borders(Borders::ALL)
                        .border_style(Style::default()),
                )
                .wrap(Wrap { trim: true });
            f.render_widget(node_info_paragraph, left_chunks[0]);

            // Wallet Info Panel
            let wallet_info_paragraph = Paragraph::new(wallet_info.as_str())
                .block(Block::default().title("Wallet Info").borders(Borders::ALL))
                .wrap(Wrap { trim: true });
            f.render_widget(wallet_info_paragraph, left_chunks[1]);

            // Commands List
            let items: Vec<ListItem> = commands
                .iter()
                .enumerate()
                .map(|(i, cmd)| {
                    let mut item = ListItem::new(cmd.to_string());
                    if i == selected {
                        item = item.style(
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        );
                    }
                    item
                })
                .collect();

            let list = List::new(items).block(
                Block::default()
                    .title("Commands (↑ / ↓) • Enter=run • r=refresh • w=QR overlay • q=quit")
                    .borders(Borders::ALL),
            );
            f.render_widget(list, left_chunks[2]);

            // Right panel output window
            let height = chunks[1].height as usize;
            let visible_height = if height > 2 { height - 2 } else { 0 };
            let visible_lines = if output_lines.len() > visible_height + scroll_offset {
                &output_lines[scroll_offset..scroll_offset + visible_height]
            } else if scroll_offset < output_lines.len() {
                &output_lines[scroll_offset..]
            } else {
                &[] as &[String]
            };

            let paragraph = Paragraph::new(visible_lines.join("\n"))
                .block(
                    Block::default()
                        .title("Output (j / k to scroll) ")
                        .borders(Borders::ALL),
                )
                .wrap(Wrap { trim: false });
            f.render_widget(paragraph, chunks[1]);

            // ===== Overlay (modal) for Address + QR =====
            if show_qr_overlay {
                let area = centered_rect(70, 70, size); // 70% x 70% of screen, centered
                // Clear anything behind the overlay area
                f.render_widget(Clear, area);

                // Split overlay vertically: help/title, input, qr
                let overlay_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(1)
                    .constraints([
                        Constraint::Length(3),
                        Constraint::Length(3),
                        Constraint::Min(8),
                    ])
                    .split(area);

                // Outer box
                let outer = Block::default()
                    .borders(Borders::ALL)
                    .title(" Address & QR (type to edit • x/Esc to close) ");
                f.render_widget(outer, area);

                // Help line
                let help = Paragraph::new("Edit the address to regenerate the QR • ←/→ move • Backspace/Delete • Home/End")
                    .wrap(Wrap { trim: true });
                f.render_widget(help, overlay_chunks[0]);

                // Input box
                let input = Paragraph::new(address.clone())
                    .block(Block::default().borders(Borders::ALL).title(" BTC Address "));
                f.render_widget(input, overlay_chunks[1]);

                // Place cursor inside input (account for borders)
                let cursor_x = (overlay_chunks[1].x + 1).saturating_add(addr_cursor as u16);
                let cursor_y = overlay_chunks[1].y + 1;
                f.set_cursor(
                    cursor_x.min(overlay_chunks[1].x + overlay_chunks[1].width.saturating_sub(2)),
                    cursor_y,
                );

                // QR box
                let qr_text = generate_qr_unicode(&address);
                let qr = Paragraph::new(qr_text)
                    .block(Block::default().borders(Borders::ALL).title(" Bitcoin QR Code "));
                f.render_widget(qr, overlay_chunks[2]);
            }
        })?;

        // Input handling with debounce
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if last_input.elapsed() >= Duration::from_millis(150) {
                    if show_qr_overlay {
                        // Keys active while overlay is open
                        match key.code {
                            KeyCode::Esc | KeyCode::Char('x') => {
                                show_qr_overlay = false;
                            }
                            KeyCode::Left => {
                                if addr_cursor > 0 {
                                    addr_cursor -= 1;
                                }
                            }
                            KeyCode::Right => {
                                if addr_cursor < address.len() {
                                    addr_cursor += 1;
                                }
                            }
                            KeyCode::Home => {
                                addr_cursor = 0;
                            }
                            KeyCode::End => {
                                addr_cursor = address.len();
                            }
                            KeyCode::Backspace => {
                                if addr_cursor > 0 && !address.is_empty() {
                                    address.remove(addr_cursor - 1);
                                    addr_cursor -= 1;
                                }
                            }
                            KeyCode::Delete => {
                                if addr_cursor < address.len() && !address.is_empty() {
                                    address.remove(addr_cursor);
                                }
                            }
                            KeyCode::Char(c) => {
                                if !c.is_control() && c != ' ' {
                                    address.insert(addr_cursor.min(address.len()), c);
                                    addr_cursor = (addr_cursor + 1).min(address.len());
                                }
                            }
                            _ => {}
                        }
                        last_input = Instant::now();
                        continue; // Don't process main view keys when modal is open
                    }

                    // Main view keys
                    match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char('w') => {
                            show_qr_overlay = true;
                            // Keep cursor index in bounds
                            if addr_cursor > address.len() {
                                addr_cursor = address.len();
                            }
                        }
                        KeyCode::Char('r') => {
                            // Refresh output, node info, and wallet info
                            output = run_bitcoin_cli(&commands[selected])?;
                            output_lines = output.lines().map(|l| l.to_string()).collect();
                            if let Ok(info) = fetch_node_info() {
                                node_info = info;
                            }
                            if let Ok(w_info) = fetch_wallet_info() {
                                wallet_info = w_info;
                            }
                            scroll_offset = 0;
                        }
                        KeyCode::Down => {
                            if selected < commands.len() - 1 {
                                selected += 1;
                                output = run_bitcoin_cli(&commands[selected])?;
                                output_lines = output.lines().map(|l| l.to_string()).collect();
                                scroll_offset = 0;
                                last_refresh = Instant::now();
                            }
                        }
                        KeyCode::Up => {
                            if selected > 0 {
                                selected -= 1;
                                output = run_bitcoin_cli(&commands[selected])?;
                                output_lines = output.lines().map(|l| l.to_string()).collect();
                                scroll_offset = 0;
                                last_refresh = Instant::now();
                            }
                        }
                        KeyCode::PageDown | KeyCode::Char('j') => {
                            if scroll_offset + 1 < output_lines.len() {
                                scroll_offset += 1;
                            }
                        }
                        KeyCode::PageUp | KeyCode::Char('k') => {
                            if scroll_offset > 0 {
                                scroll_offset -= 1;
                            }
                        }
                        KeyCode::Enter => {
                            output = run_bitcoin_cli(&commands[selected])?;
                            output_lines = output.lines().map(|l| l.to_string()).collect();
                            scroll_offset = 0;
                            last_refresh = Instant::now();
                        }
                        _ => {}
                    }
                    last_input = Instant::now();
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

fn load_commands_from_json(path: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let commands: Vec<String> = serde_json::from_reader(reader)?;
    Ok(commands)
}

// ===== Helpers for overlay & QR =====

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn generate_qr_unicode(data: &str) -> String {
    // Render a tiny valid QR even for empty input to avoid panic
    let safe = if data.is_empty() { " " } else { data };
    let code = QrCode::new(safe).unwrap();
    code.render::<unicode::Dense1x2>().quiet_zone(false).build()
}
