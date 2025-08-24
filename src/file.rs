use serde::{Deserialize, Serialize};
use serde_json;
use std::fs::File;
use std::io::{self, BufReader};

use chrono::{DateTime, Utc};

// ===== Address book types & constants =====
pub const ADDRESS_BOOK_PATH: &str = "addresses.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressEntry {
    pub created_at: DateTime<Utc>,
    pub address: String,
}

pub(crate) fn load_commands_from_json(
    path: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let commands: Vec<String> = serde_json::from_reader(reader)?;
    Ok(commands)
}

pub(crate) fn load_address_book(path: &str) -> Vec<AddressEntry> {
    match File::open(path) {
        Ok(f) => match serde_json::from_reader::<_, Vec<AddressEntry>>(f) {
            Ok(list) => list,
            Err(_) => Vec::new(),
        },
        Err(_) => Vec::new(),
    }
}

pub(crate) fn save_address_book(path: &str, entries: &Vec<AddressEntry>) -> Result<(), String> {
    let data = serde_json::to_string_pretty(entries).map_err(|e| e.to_string())?;
    std::fs::write(path, data).map_err(|e| e.to_string())
}
