use std::fs::File;
use std::io::{self, BufReader};
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
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

use qrcode::QrCode;
use qrcode::render::unicode;

use bitcoin::{Address, Network};
use core::str::FromStr;

use arboard::Clipboard;

use serde::{Deserialize, Serialize};
use serde_json;

use chrono::{DateTime, Utc};

mod cli;
mod node;

use crate::cli::run_bitcoin_cli;
use crate::node::{fetch_node_info, fetch_wallet_info};

// ===== Address book types & constants =====
const ADDRESS_BOOK_PATH: &str = "addresses.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AddressEntry {
    created_at: DateTime<Utc>,
    address: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;

    let mut hide_amounts = false;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Load command list (left pane)
    let commands = load_commands_from_json("commands.json")?;

    // Main UI state
    let mut selected = 0usize;
    let mut last_input = Instant::now();
    let mut output = String::new();
    let mut output_lines: Vec<String> = Vec::new();
    let mut scroll_offset = 0usize;

    let _refresh_interval = Duration::from_secs(5);
    let mut _last_refresh = Instant::now();

    // Node/Wallet info
    let mut node_info = String::new();
    let mut wallet_info = String::new();

    // Overlay state
    let mut show_qr_overlay = false;
    let mut address = String::from("bc1qfpacvgpjms0eu6mszhwgjjs03yldesmmcgzad0");
    let mut addr_cursor: usize = address.len();

    // Address book state (persistent)
    let mut addr_book: Vec<AddressEntry> = load_address_book(ADDRESS_BOOK_PATH);
    let mut addr_selected: usize = if addr_book.is_empty() {
        0
    } else {
        addr_book.len() - 1
    };

    // Initial fetches
    output = run_bitcoin_cli(&commands[selected])?;
    output_lines = output.lines().map(|l| l.to_string()).collect();

    node_info = fetch_node_info().unwrap_or_else(|_| "Failed to fetch node info".to_string());
    wallet_info = fetch_wallet_info().unwrap_or_else(|_| "Failed to fetch wallet info".to_string());

    loop {
        terminal.draw(|f| {
            let size = f.size();

            // Root: main area + bottom help bar (height 4 to show 2 lines comfortably)
            let root = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(4)])
                .split(size);

            // ===== Main content (top) =====
            let main_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
                .split(root[0]);

            // Left: Node Info (7), Wallet Info (7), Commands (rest)
            let left_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(7), Constraint::Length(7), Constraint::Min(0)])
                .split(main_chunks[0]);

            // Node Info panel
            let node_info_paragraph = Paragraph::new(node_info.as_str())
                .block(
                    Block::default()
                        .title("Node Info")
                        .borders(Borders::ALL)
                        .border_style(Style::default()),
                )
                .wrap(Wrap { trim: true });
            f.render_widget(node_info_paragraph, left_chunks[0]);

            // Wallet Info panel
            let wallet_info_paragraph = Paragraph::new(mask_digits_if(&wallet_info, hide_amounts))
                .block(Block::default().title("Wallet Info").borders(Borders::ALL))
                .wrap(Wrap { trim: true });
            f.render_widget(wallet_info_paragraph, left_chunks[1]);

            // Commands list
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

            let list = List::new(items).block(Block::default().title("Commands").borders(Borders::ALL));
            f.render_widget(list, left_chunks[2]);

            // Right: Output panel
            let height = main_chunks[1].height as usize;
            let visible_height = if height > 2 { height - 2 } else { 0 };
            let visible_lines = if output_lines.len() > visible_height + scroll_offset {
                &output_lines[scroll_offset..scroll_offset + visible_height]
            } else if scroll_offset < output_lines.len() {
                &output_lines[scroll_offset..]
            } else {
                &[] as &[String]
            };

            let paragraph = Paragraph::new(visible_lines.join("\n"))
                .block(Block::default().title("Output").borders(Borders::ALL))
                .wrap(Wrap { trim: false });
            f.render_widget(paragraph, main_chunks[1]);

            // ===== Bottom Help bar =====
            let orange = Color::Rgb(255, 165, 0);
            let help_lines: Vec<Line> = if show_qr_overlay {
                vec![
                    Line::from(Span::styled(
                        "Overlay keys:",
                        Style::default().fg(orange).add_modifier(Modifier::BOLD),
                    )),
                    Line::from(
                        "Ctrl+N=new(save)  Ctrl+G=getnew  Ctrl+C=copy  ↑/↓=select saved  ←/→ Home End Backspace Delete=edit  Ctrl+X=close",
                    ),
                ]
            } else {
                vec![
                    Line::from(Span::styled(
                        "Main keys:",
                        Style::default().fg(orange).add_modifier(Modifier::BOLD),
                    )),
                   Line::from("↑/↓=select command  Enter=run  r=refresh  j/k=scroll output  h=hide/show amounts w=QR overlay  q=quit"),
                ]
            };

            let help = Paragraph::new(help_lines)
                .wrap(Wrap { trim: true })
                .block(
                    Block::default()
                        .title("Help")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(orange)),
                );
            f.render_widget(help, root[1]);

            // ===== Overlay on top (if active) =====
            if show_qr_overlay {
                let orange = Color::Rgb(255, 165, 0);
                let area = centered_rect(80, 75, size);
                f.render_widget(Clear, area);

                // Outer box
                let outer = Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(orange))
                    .title(" Address Book & QR (edit left • list right) ");
                f.render_widget(outer, area);

                // Split overlay horizontally: left (editor + QR), right (list)
                let cols = Layout::default()
                    .direction(Direction::Horizontal)
                    .margin(1)
                    .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                    .split(area);

                // Left column: input, QR
                let left = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(3), Constraint::Min(8)])
                    .split(cols[0]);

                // Validation
                let validity = check_address(&address);
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
                                Network::Bitcoin => "VALID (mainnet)",
                                Network::Testnet => "VALID (testnet)",
                                Network::Testnet4 => "VALID (testnet4)",
                                Network::Signet => "VALID (signet)",
                                Network::Regtest => "VALID (regtest)",
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

                // Input box
                let input_block = Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(orange))
                    .title(Span::styled(input_title.clone(), input_title_style));
                let input = Paragraph::new(address.clone()).block(input_block);
                f.render_widget(input, left[0]);

                // Cursor inside input
                let cursor_x = (left[0].x + 1).saturating_add(addr_cursor as u16);
                let cursor_y = left[0].y + 1;
                f.set_cursor(
                    cursor_x.min(left[0].x + left[0].width.saturating_sub(2)),
                    cursor_y,
                );

                // QR box
                let qr_block = Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(orange))
                    .title(Span::styled(qr_title.clone(), qr_title_style));
                let qr_text = if qr_dim { String::new() } else { generate_qr_unicode(&address) };
                let mut qr_par = Paragraph::new(qr_text).block(qr_block);
                if qr_dim {
                    qr_par = qr_par.style(Style::default().fg(Color::DarkGray));
                }
                f.render_widget(qr_par, left[1]);

                // Right column: address list
                let list_items: Vec<ListItem> = addr_book
                    .iter()
                    .enumerate()
                    .map(|(i, e)| {
                        let date_str = e.created_at.format("%Y-%m-%d %H:%M").to_string();
                        let shown = if e.address.len() > 22 {
                            format!(
                                "{}  {}…{}",
                                date_str,
                                &e.address[..12],
                                &e.address[e.address.len() - 8..]
                            )
                        } else {
                            format!("{}  {}", date_str, e.address)
                        };
                        let mut item = ListItem::new(shown);
                        if i == addr_selected {
                            item = item.style(
                                Style::default()
                                    .fg(Color::Yellow)
                                    .add_modifier(Modifier::BOLD),
                            );
                        }
                        item
                    })
                    .collect();

                let list_block = Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(orange))
                    .title(" Addresses (↑/↓ select) ");
                let list = List::new(list_items).block(list_block);
                f.render_widget(list, cols[1]);
            }
        })?;

        // ===== Input handling =====
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if last_input.elapsed() >= Duration::from_millis(120) {
                    if show_qr_overlay {
                        // Keys active while overlay is open
                        match (key.modifiers, key.code) {
                            // ---- Ctrl combos ----
                            (m, KeyCode::Char('n')) if m.contains(KeyModifiers::CONTROL) => {
                                match run_bitcoin_cli("getnewaddress") {
                                    Ok(s) => {
                                        let new_addr = s.trim().to_string();
                                        if matches!(
                                            check_address(&new_addr),
                                            AddrValidity::ValidAny(_)
                                        ) {
                                            let entry = AddressEntry {
                                                created_at: Utc::now(),
                                                address: new_addr.clone(),
                                            };
                                            addr_book.push(entry);
                                            let _ =
                                                save_address_book(ADDRESS_BOOK_PATH, &addr_book);
                                            addr_selected = addr_book.len() - 1;
                                            address = new_addr;
                                            addr_cursor = address.len();
                                        }
                                    }
                                    Err(_) => {}
                                }
                            }
                            (m, KeyCode::Char('g')) if m.contains(KeyModifiers::CONTROL) => {
                                if let Ok(s) = run_bitcoin_cli("getnewaddress") {
                                    address = s.trim().to_string();
                                    addr_cursor = address.len();
                                }
                            }
                            (m, KeyCode::Char('c')) if m.contains(KeyModifiers::CONTROL) => {
                                let _ = copy_to_clipboard(&address);
                            }
                            (m, KeyCode::Char('x')) if m.contains(KeyModifiers::CONTROL) => {
                                show_qr_overlay = false; // close overlay
                            }

                            // ---- Navigation in list ----
                            (_, KeyCode::Up) => {
                                if !addr_book.is_empty() && addr_selected > 0 {
                                    addr_selected -= 1;
                                    address = addr_book[addr_selected].address.clone();
                                    addr_cursor = address.len();
                                }
                            }
                            (_, KeyCode::Down) => {
                                if !addr_book.is_empty() && addr_selected + 1 < addr_book.len() {
                                    addr_selected += 1;
                                    address = addr_book[addr_selected].address.clone();
                                    addr_cursor = address.len();
                                }
                            }

                            // ---- Editing the input ----
                            (_, KeyCode::Left) => {
                                if addr_cursor > 0 {
                                    addr_cursor -= 1;
                                }
                            }
                            (_, KeyCode::Right) => {
                                if addr_cursor < address.len() {
                                    addr_cursor += 1;
                                }
                            }
                            (_, KeyCode::Home) => {
                                addr_cursor = 0;
                            }
                            (_, KeyCode::End) => {
                                addr_cursor = address.len();
                            }
                            (_, KeyCode::Backspace) => {
                                if addr_cursor > 0 && !address.is_empty() {
                                    address.remove(addr_cursor - 1);
                                    addr_cursor -= 1;
                                }
                            }
                            (_, KeyCode::Delete) => {
                                if addr_cursor < address.len() && !address.is_empty() {
                                    address.remove(addr_cursor);
                                }
                            }
                            (_, KeyCode::Char(c)) => {
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

                    // Main view keys (overlay closed)
                    match key.code {
                        KeyCode::Char('h') => {
                            hide_amounts = !hide_amounts;
                        }
                        KeyCode::Char('q') => break,
                        KeyCode::Char('w') => {
                            show_qr_overlay = true;
                            if !addr_book.is_empty() {
                                address = addr_book[addr_selected].address.clone();
                            }
                            addr_cursor = address.len();
                        }
                        KeyCode::Char('r') => {
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
                                _last_refresh = Instant::now();
                            }
                        }
                        KeyCode::Up => {
                            if selected > 0 {
                                selected -= 1;
                                output = run_bitcoin_cli(&commands[selected])?;
                                output_lines = output.lines().map(|l| l.to_string()).collect();
                                scroll_offset = 0;
                                _last_refresh = Instant::now();
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
                            _last_refresh = Instant::now();
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

// ===== Address validation =====

#[derive(Clone, Copy, Debug)]
enum AddrValidity {
    Empty,
    Invalid,
    ValidAny(Network),
}

fn check_address(addr: &str) -> AddrValidity {
    let s = addr.trim();
    if s.is_empty() {
        return AddrValidity::Empty;
    }
    match Address::from_str(s) {
        Ok(a) => {
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

// ===== Address book persistence =====
fn load_address_book(path: &str) -> Vec<AddressEntry> {
    match File::open(path) {
        Ok(f) => match serde_json::from_reader::<_, Vec<AddressEntry>>(f) {
            Ok(list) => list,
            Err(_) => Vec::new(),
        },
        Err(_) => Vec::new(),
    }
}

fn save_address_book(path: &str, entries: &Vec<AddressEntry>) -> Result<(), String> {
    let data = serde_json::to_string_pretty(entries).map_err(|e| e.to_string())?;
    std::fs::write(path, data).map_err(|e| e.to_string())
}

// ===== Amount masking (no regex, keeps punctuation/currency) =====
fn mask_digits_if(s: &str, hide: bool) -> String {
    if !hide {
        return s.to_string();
    }
    s.chars()
        .map(|c| if c.is_ascii_digit() { 'X' } else { c })
        .collect()
}
