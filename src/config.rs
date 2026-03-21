pub const DATA_DIR: &str = "/var/bitcoin";
pub const ADDRESS_BOOK_FILE: &str = "addresses.json";
pub const COMMANDS_FILE: &str = "commands.json";

pub fn get_address_book_path() -> String {
    format!("{}/{}", DATA_DIR, ADDRESS_BOOK_FILE)
}

pub fn get_commands_path() -> String {
    format!("{}/{}", DATA_DIR, COMMANDS_FILE)
}
