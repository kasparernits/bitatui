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
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

// QR + validation
use bitcoin::{Address, Network};
use core::str::FromStr;
use qrcode::QrCode;
use qrcode::render::unicode;

// Clipboard
use arboard::Clipboard;

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

    // Overlay state
    let mut show_qr_overlay = false;
    let mut address = String::from("bc1qfpacvgpjms0eu6mszhwgjjs03yldesmmcgzad0");
    let mut addr_cursor: usize = address.len();
    let mut overlay_status: Option<(String, Instant)> = None; // ephemeral status (copy ok/error, etc.)

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

            // Left panel split vertically into Node Info (7), Wallet Info (7), Commands (rest)
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
                    .title("Commands (↑/↓) • Enter=run • r=refresh • w=QR overlay • q=quit")
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
                let orange = Color::Rgb(255, 165, 0);
                let area = centered_rect(70, 70, size);
                f.render_widget(Clear, area);

                // Outer box with orange border
                let outer = Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(orange))
                    .title(" Address & QR (type to edit • g=new addr • c=copy • x/Esc close) ");
                f.render_widget(outer, area);

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

                // Build help/status lines
                let mut help_lines: Vec<Line> = vec![
                    Line::from(Span::styled(
                        "Edit the address to regenerate the QR",
                        Style::default().fg(orange).add_modifier(Modifier::BOLD),
                    )),
                    Line::from("←/→ move • Backspace/Delete • Home/End • g=new address • c=copy • x/Esc close"),
                ];
                if let Some((msg, ts)) = &overlay_status {
                    if ts.elapsed() < Duration::from_secs(2) {
                        help_lines.push(Line::from(Span::styled(
                            msg.as_str(),
                            Style::default().fg(Color::Cyan),
                        )));
                    }
                }
                let help = Paragraph::new(help_lines).wrap(Wrap { trim: true });
                f.render_widget(help, overlay_chunks[0]);

                // Validation
let validity = check_address(&address);

// NOTE: titles are owned Strings now
let (input_title, input_title_style, qr_title, qr_title_style, qr_dim): (String, Style, String, Style, bool) =
    match validity {
        AddrValidity::Empty => (
            " BTC Address ".to_string(),
            Style::default().fg(Color::Yellow),
            " Bitcoin QR Code — (enter an address) ".to_string(),
            Style::default().fg(Color::Yellow),
            true,
        ),
        AddrValidity::Invalid => (
            " BTC Address — INVALID ".to_string(),
            Style::default().fg(Color::Red),
            " Bitcoin QR Code — INVALID ".to_string(),
            Style::default().fg(Color::Red),
            true,
        ),
        AddrValidity::ValidAny(net) => {
            let label = match net {
                Network::Bitcoin  => "VALID (mainnet)",
                Network::Testnet  => "VALID (testnet)",
                Network::Testnet4 => "VALID (testnet4)",
                Network::Signet   => "VALID (signet)",
                Network::Regtest  => "VALID (regtest)",
            };
            (
                " BTC Address — VALID ".to_string(),
                Style::default().fg(Color::Green),
                format!(" Bitcoin QR Code — {label} "),
                Style::default().fg(Color::Green),
                false,
            )
        }
    };

// Input box (orange border, title colored by validity)
let input_block = Block::default()
    .borders(Borders::ALL)
    .border_style(Style::default().fg(orange))
    .title(Span::styled(input_title.clone(), input_title_style));
let input = Paragraph::new(address.clone()).block(input_block);
f.render_widget(input, overlay_chunks[1]);

// QR box (orange border + title colored by validity)
let qr_block = Block::default()
    .borders(Borders::ALL)
    .border_style(Style::default().fg(orange))
    .title(Span::styled(qr_title.clone(), qr_title_style));
let qr_text = if qr_dim { String::new() } else { generate_qr_unicode(&address) };
let mut qr_par = Paragraph::new(qr_text).block(qr_block);
if qr_dim {
    qr_par = qr_par.style(Style::default().fg(Color::DarkGray));
}
f.render_widget(qr_par, overlay_chunks[2]);

            }
        })?;

        // Input handling with debounce
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if last_input.elapsed() >= Duration::from_millis(150) {
                    if show_qr_overlay {
                        // Keys for overlay
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
                            KeyCode::Char('g') => match run_bitcoin_cli("getnewaddress") {
                                Ok(s) => {
                                    address = s.trim().to_string();
                                    addr_cursor = address.len();
                                    overlay_status =
                                        Some(("New address fetched".into(), Instant::now()));
                                }
                                Err(e) => {
                                    overlay_status =
                                        Some((format!("getnewaddress error: {e}"), Instant::now()));
                                }
                            },
                            KeyCode::Char('c') => match copy_to_clipboard(&address) {
                                Ok(()) => {
                                    overlay_status =
                                        Some(("Copied to clipboard".into(), Instant::now()))
                                }
                                Err(e) => {
                                    overlay_status =
                                        Some((format!("Copy failed: {e}"), Instant::now()))
                                }
                            },
                            KeyCode::Char(c) => {
                                if !c.is_control() && c != ' ' {
                                    address.insert(addr_cursor.min(address.len()), c);
                                    addr_cursor = (addr_cursor + 1).min(address.len());
                                }
                            }
                            _ => {}
                        }
                        last_input = Instant::now();
                        continue; // don't process main keys while modal is open
                    }

                    // Main view keys
                    match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char('w') => {
                            show_qr_overlay = true;
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
    let safe = if data.is_empty() { " " } else { data };
    let code = QrCode::new(safe).unwrap();
    code.render::<unicode::Dense1x2>().quiet_zone(false).build()
}

// ===== Address validation =====

#[derive(Clone, Copy, Debug)]
enum AddrValidity {
    Empty,
    Invalid,
    ValidAny(Network),
}

fn check_address(addr: &str) -> AddrValidity {
    use core::str::FromStr;
    let s = addr.trim();
    if s.is_empty() {
        return AddrValidity::Empty;
    }
    match Address::from_str(s) {
        Ok(a) => {
            // Try to promote to a checked address for each network.
            for net in [
                Network::Bitcoin,
                Network::Testnet,
                Network::Testnet4,
                Network::Signet,
                Network::Regtest,
            ] {
                if a.clone().require_network(net).is_ok() {
                    return AddrValidity::ValidAny(net);
                }
            }
            // Parsed but didn’t fit any known network (unlikely).
            AddrValidity::Invalid
        }
        Err(_) => AddrValidity::Invalid,
    }
}

// ===== Clipboard =====
fn copy_to_clipboard(text: &str) -> Result<(), String> {
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    clipboard
        .set_text(text.to_owned())
        .map_err(|e| e.to_string())
}
