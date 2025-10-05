use std::time::{Duration, Instant};

use arboard::Clipboard;
use bitcoin::{Address, Network};
use chrono::Utc;
use core::str::FromStr;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::cli::run_bitcoin_cli;
use crate::file::{ADDRESS_BOOK_PATH, AddressEntry, save_address_book};
use crate::{file, node};

const INPUT_THROTTLE: Duration = Duration::from_millis(120);
const DEFAULT_ADDRESS: &str = "bc1qfpacvgpjms0eu6mszhwgjjs03yldesmmcgzad0";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppAction {
    Continue,
    Quit,
}

pub struct App {
    pub commands: Vec<String>,
    pub selected: usize,
    pub hide_amounts: bool,
    pub last_input: Instant,
    pub output: String,
    pub output_lines: Vec<String>,
    pub scroll_offset: usize,
    pub node_info: String,
    pub wallet_info: String,
    pub show_qr_overlay: bool,
    pub address: String,
    pub addr_cursor: usize,
    pub addr_book: Vec<AddressEntry>,
    pub addr_selected: usize,
}

impl App {
    pub fn initialize() -> Result<Self, Box<dyn std::error::Error>> {
        let commands = file::load_commands_from_json("commands.json")?;
        if commands.is_empty() {
            return Err("commands.json did not contain any commands".into());
        }

        let mut app = Self {
            commands,
            selected: 0,
            hide_amounts: false,
            last_input: Instant::now(),
            output: String::new(),
            output_lines: Vec::new(),
            scroll_offset: 0,
            node_info: String::new(),
            wallet_info: String::new(),
            show_qr_overlay: false,
            address: DEFAULT_ADDRESS.to_string(),
            addr_cursor: DEFAULT_ADDRESS.len(),
            addr_book: file::load_address_book(ADDRESS_BOOK_PATH),
            addr_selected: 0,
        };

        if !app.addr_book.is_empty() {
            app.addr_selected = app.addr_book.len() - 1;
            app.address = app.addr_book[app.addr_selected].address.clone();
            app.addr_cursor = app.address.len();
        }

        app.run_selected_command()?;
        app.refresh_node_info();
        app.refresh_wallet_info();

        Ok(app)
    }

    pub fn visible_output_lines(&self, visible_height: usize) -> &[String] {
        if visible_height == 0 || self.output_lines.is_empty() {
            return &[];
        }

        let start = self.scroll_offset.min(self.output_lines.len());
        let end = (start + visible_height).min(self.output_lines.len());
        &self.output_lines[start..end]
    }

    pub fn wallet_info_text(&self) -> String {
        if self.hide_amounts {
            mask_digits(&self.wallet_info)
        } else {
            self.wallet_info.clone()
        }
    }

    pub fn handle_key_event(
        &mut self,
        key: KeyEvent,
    ) -> Result<AppAction, Box<dyn std::error::Error>> {
        if self.last_input.elapsed() < INPUT_THROTTLE {
            return Ok(AppAction::Continue);
        }

        let mut action = AppAction::Continue;
        if self.show_qr_overlay {
            self.handle_overlay_key(key)?;
        } else {
            action = self.handle_main_key(key)?;
        }

        self.last_input = Instant::now();
        Ok(action)
    }

    fn handle_main_key(&mut self, key: KeyEvent) -> Result<AppAction, Box<dyn std::error::Error>> {
        match key.code {
            KeyCode::Char('h') => {
                self.hide_amounts = !self.hide_amounts;
            }
            KeyCode::Char('q') => {
                return Ok(AppAction::Quit);
            }
            KeyCode::Char('w') => {
                self.show_qr_overlay = true;
                if !self.addr_book.is_empty() {
                    self.address = self.addr_book[self.addr_selected].address.clone();
                    self.addr_cursor = self.address.len();
                }
            }
            KeyCode::Char('r') => {
                self.run_selected_command()?;
                self.refresh_node_info();
                self.refresh_wallet_info();
                self.scroll_offset = 0;
            }
            KeyCode::Down => {
                if self.selected + 1 < self.commands.len() {
                    self.selected += 1;
                    self.run_selected_command()?;
                    self.scroll_offset = 0;
                }
            }
            KeyCode::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                    self.run_selected_command()?;
                    self.scroll_offset = 0;
                }
            }
            KeyCode::PageDown | KeyCode::Char('j') => {
                if self.scroll_offset + 1 < self.output_lines.len() {
                    self.scroll_offset += 1;
                }
            }
            KeyCode::PageUp | KeyCode::Char('k') => {
                if self.scroll_offset > 0 {
                    self.scroll_offset -= 1;
                }
            }
            KeyCode::Enter => {
                self.run_selected_command()?;
                self.scroll_offset = 0;
            }
            _ => {}
        }

        Ok(AppAction::Continue)
    }

    fn handle_overlay_key(&mut self, key: KeyEvent) -> Result<(), Box<dyn std::error::Error>> {
        match (key.modifiers, key.code) {
            (m, KeyCode::Char('n')) if m.contains(KeyModifiers::CONTROL) => {
                if let Ok(s) = run_bitcoin_cli("getnewaddress") {
                    let new_addr = s.trim().to_string();
                    if matches!(check_address(&new_addr), AddrValidity::ValidAny(_)) {
                        let entry = AddressEntry {
                            created_at: Utc::now(),
                            address: new_addr.clone(),
                        };
                        self.addr_book.push(entry);
                        if let Err(err) = save_address_book(ADDRESS_BOOK_PATH, &self.addr_book) {
                            self.output_lines
                                .push(format!("Failed to save address book: {err}"));
                        }
                        self.addr_selected = self.addr_book.len() - 1;
                        self.address = new_addr;
                        self.addr_cursor = self.address.len();
                    }
                }
            }
            (m, KeyCode::Char('g')) if m.contains(KeyModifiers::CONTROL) => {
                if let Ok(s) = run_bitcoin_cli("getnewaddress") {
                    self.address = s.trim().to_string();
                    self.addr_cursor = self.address.len();
                }
            }
            (m, KeyCode::Char('c')) if m.contains(KeyModifiers::CONTROL) => {
                let _ = copy_to_clipboard(&self.address);
            }
            (m, KeyCode::Char('x')) if m.contains(KeyModifiers::CONTROL) => {
                self.show_qr_overlay = false;
            }
            (_, KeyCode::Up) => {
                if !self.addr_book.is_empty() && self.addr_selected > 0 {
                    self.addr_selected -= 1;
                    self.address = self.addr_book[self.addr_selected].address.clone();
                    self.addr_cursor = self.address.len();
                }
            }
            (_, KeyCode::Down) => {
                if !self.addr_book.is_empty() && self.addr_selected + 1 < self.addr_book.len() {
                    self.addr_selected += 1;
                    self.address = self.addr_book[self.addr_selected].address.clone();
                    self.addr_cursor = self.address.len();
                }
            }
            (_, KeyCode::Left) => {
                if self.addr_cursor > 0 {
                    self.addr_cursor -= 1;
                }
            }
            (_, KeyCode::Right) => {
                if self.addr_cursor < self.address.len() {
                    self.addr_cursor += 1;
                }
            }
            (_, KeyCode::Home) => {
                self.addr_cursor = 0;
            }
            (_, KeyCode::End) => {
                self.addr_cursor = self.address.len();
            }
            (_, KeyCode::Backspace) => {
                if self.addr_cursor > 0 && !self.address.is_empty() {
                    self.address.remove(self.addr_cursor - 1);
                    self.addr_cursor -= 1;
                }
            }
            (_, KeyCode::Delete) => {
                if self.addr_cursor < self.address.len() && !self.address.is_empty() {
                    self.address.remove(self.addr_cursor);
                }
            }
            (_, KeyCode::Char(c)) => {
                if !c.is_control() && c != ' ' {
                    let idx = self.addr_cursor.min(self.address.len());
                    self.address.insert(idx, c);
                    self.addr_cursor = self.addr_cursor.saturating_add(1).min(self.address.len());
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn run_selected_command(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.commands.is_empty() {
            self.output.clear();
            self.output_lines.clear();
            return Ok(());
        }

        let output = run_bitcoin_cli(&self.commands[self.selected])?;
        self.output = output.clone();
        self.output_lines = output.lines().map(|l| l.to_string()).collect();
        Ok(())
    }

    fn refresh_node_info(&mut self) {
        self.node_info =
            node::fetch_node_info().unwrap_or_else(|_| "Failed to fetch node info".to_string());
    }

    fn refresh_wallet_info(&mut self) {
        self.wallet_info =
            node::fetch_wallet_info().unwrap_or_else(|_| "Failed to fetch wallet info".to_string());
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddrValidity {
    Empty,
    Invalid,
    ValidAny(Network),
}

pub fn check_address(addr: &str) -> AddrValidity {
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

fn copy_to_clipboard(text: &str) -> Result<(), String> {
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    clipboard
        .set_text(text.to_owned())
        .map_err(|e| e.to_string())
}

fn mask_digits(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_digit() { 'X' } else { c })
        .collect()
}
