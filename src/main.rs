use std::io;
use std::process::Command;
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

use serde_json; // Add this line for JSON parsing

fn main() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let commands = vec![
        "getblockchaininfo",
        "getblockcount",
        "getnetworkinfo",
        "getmempoolinfo",
        "getpeerinfo",
        "getwalletinfo",
        "getbestblockhash",
        "getdifficulty",
        "getchaintips",
        "getrawmempool",
        "estimatesmartfee 6",
    ];

    let mut selected = 0;
    let mut last_input = Instant::now();
    let mut output = String::new();
    let mut output_lines: Vec<String> = Vec::new();
    let mut scroll_offset = 0;

    let refresh_interval = Duration::from_secs(5);
    let mut last_refresh = Instant::now();

    // For node info refresh
    let mut node_info = String::new();
    // For wallet info refresh
    let mut wallet_info = String::new();

    // Initial fetch
    output = run_bitcoin_cli(commands[selected])?;
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
                        .border_style(Style::default().fg(Color::Rgb(255, 165, 0))),
                ) // orange
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

            let list =
                List::new(items).block(Block::default().title("Commands").borders(Borders::ALL));
            f.render_widget(list, left_chunks[2]);

            // Right panel output window
            let height = chunks[1].height as usize;
            let visible_height = if height > 2 { height - 2 } else { 0 };
            let visible_lines = if output_lines.len() > visible_height + scroll_offset {
                &output_lines[scroll_offset..scroll_offset + visible_height]
            } else if scroll_offset < output_lines.len() {
                &output_lines[scroll_offset..]
            } else {
                &[]
            };

            let paragraph = Paragraph::new(visible_lines.join("\n"))
                .block(Block::default().title("Output").borders(Borders::ALL))
                .wrap(Wrap { trim: false });
            f.render_widget(paragraph, chunks[1]);
        })?;

        // Input handling with debounce
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if last_input.elapsed() >= Duration::from_millis(150) {
                    match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char('r') => {
                            // Refresh output, node info, and wallet info
                            output = run_bitcoin_cli(commands[selected])?;
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
                                output = run_bitcoin_cli(commands[selected])?;
                                output_lines = output.lines().map(|l| l.to_string()).collect();
                                scroll_offset = 0;
                                last_refresh = Instant::now();
                            }
                        }
                        KeyCode::Up => {
                            if selected > 0 {
                                selected -= 1;
                                output = run_bitcoin_cli(commands[selected])?;
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
                            output = run_bitcoin_cli(commands[selected])?;
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

fn run_bitcoin_cli(command: &str) -> Result<String, Box<dyn std::error::Error>> {
    let rpc_user = std::env::var("RPC_USER").unwrap_or_else(|_| "youruser".to_string());
    let rpc_password = std::env::var("RPC_PASSWORD").unwrap_or_else(|_| "yourpassword".to_string());

    let mut parts = command.split_whitespace();
    let base_cmd = parts.next().unwrap();
    let args: Vec<&str> = parts.collect();

    let output = Command::new("bitcoin-cli")
        .arg(format!("-rpcuser={}", rpc_user))
        .arg(format!("-rpcpassword={}", rpc_password))
        .arg(base_cmd)
        .args(&args)
        .output()?;

    let out = if output.status.success() {
        String::from_utf8_lossy(&output.stdout).to_string()
    } else {
        format!("Error: {}", String::from_utf8_lossy(&output.stderr))
    };

    Ok(out)
}

fn format_uptime(seconds: u64) -> String {
    let minutes = seconds / 60;
    let hours = minutes / 60;
    let days = hours / 24;

    if days > 0 {
        let rem_hours = hours % 24;
        format!("{} day(s) {} hour(s)", days, rem_hours)
    } else if hours > 0 {
        let rem_minutes = minutes % 60;
        format!("{} hour(s) {} minute(s)", hours, rem_minutes)
    } else {
        format!("{} minute(s)", minutes)
    }
}

fn fetch_node_info() -> Result<String, Box<dyn std::error::Error>> {
    // uptime is not a standard bitcoin-cli RPC call, so fallback if it fails
    let uptime_str = run_bitcoin_cli("uptime").unwrap_or_else(|_| "0".to_string());
    let uptime = uptime_str.trim().parse().unwrap_or(0);
    let blockcount = run_bitcoin_cli("getblockcount")?.trim().to_string();
    let bestblockhash = run_bitcoin_cli("getbestblockhash")?.trim().to_string();

    Ok(format!(
        "Uptime: {}\nBlock Count: {}\nBest Block Hash:\n{}",
        format_uptime(uptime),
        blockcount,
        bestblockhash
    ))
}

fn fetch_wallet_info() -> Result<String, Box<dyn std::error::Error>> {
    let output = run_bitcoin_cli("getwalletinfo")?;
    let json: serde_json::Value = serde_json::from_str(&output)?;

    let wallet_name = json["walletname"].as_str().unwrap_or("N/A");
    let balance = json["balance"].as_f64().unwrap_or(0.0);
    let tx_count = json["txcount"].as_u64().unwrap_or(0);
    let keypool_size = json["keypoolsize"].as_u64().unwrap_or(0);

    Ok(format!(
        "Wallet: {}\nBalance: {:.8} BTC\nTransactions: {}\nKeypool Size: {}",
        wallet_name, balance, tx_count, keypool_size
    ))
}
