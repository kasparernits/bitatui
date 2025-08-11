use crate::cli::run_bitcoin_cli;

pub(crate) fn fetch_node_info() -> Result<String, Box<dyn std::error::Error>> {
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

pub(crate) fn fetch_wallet_info() -> Result<String, Box<dyn std::error::Error>> {
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
