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
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

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

    let refresh_interval = Duration::from_secs(5);
    let mut last_refresh = Instant::now();

    // For node info refresh
    let mut node_info = String::new();
    // For wallet info refresh
    let mut wallet_info = String::new();

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
