use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use chrono::{DateTime, Utc};
use crate::error::AppResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressEntry {
    pub created_at: DateTime<Utc>,
    pub address: String,
}

pub(crate) fn load_commands_from_json(path: &str) -> AppResult<Vec<String>> {
    let final_path = if Path::new(path).exists() {
        path
    } else if Path::new("commands.json").exists() {
        "commands.json"
    } else {
        path
    };

    let file = File::open(final_path)?;
    let reader = BufReader::new(file);
    let commands: Vec<String> = serde_json::from_reader(reader)?;
    Ok(commands)
}

pub(crate) fn load_address_book(path: &str) -> Vec<AddressEntry> {
    File::open(path)
        .ok()
        .and_then(|f| serde_json::from_reader(f).ok())
        .unwrap_or_default()
}

pub(crate) fn save_address_book(path: &str, entries: &Vec<AddressEntry>) -> AppResult<()> {
    let data = serde_json::to_string_pretty(entries)?;
    std::fs::write(path, data)?;
    Ok(())
}
