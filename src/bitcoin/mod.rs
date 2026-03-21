use std::process::Command;
use crate::config;
use crate::error::{AppError, AppResult};

pub struct BitcoinClient {
    rpc_user: Option<String>,
    rpc_password: Option<String>,
    rpc_wallet: Option<String>,
}

impl BitcoinClient {
    pub fn new() -> Self {
        Self {
            rpc_user: std::env::var("RPC_USER").ok(),
            rpc_password: std::env::var("RPC_PASSWORD").ok(),
            rpc_wallet: std::env::var("RPC_WALLET").ok(),
        }
    }

    pub fn run_cli(&self, command: &str) -> AppResult<String> {
        let mut parts = command.split_whitespace();
        let base_cmd = match parts.next() {
            Some(c) => c,
            None => return Ok("No command provided".to_string()),
        };
        let args: Vec<&str> = parts.collect();

        let mut cmd = Command::new("bitcoin-cli");
        cmd.arg(format!("-datadir={}", config::DATA_DIR));

        if let Some(user) = &self.rpc_user {
            cmd.arg(format!("-rpcuser={}", user));
        }
        if let Some(pass) = &self.rpc_password {
            cmd.arg(format!("-rpcpassword={}", pass));
        }
        if let Some(wallet) = &self.rpc_wallet {
            cmd.arg(format!("-rpcwallet={}", wallet));
        }

        cmd.arg(base_cmd).args(&args);

        let output = cmd.output().map_err(|e| AppError::BitcoinCli(e.to_string()))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Ok(format!("Error: {}", String::from_utf8_lossy(&output.stderr)))
        }
    }

    pub fn fetch_node_info(&self) -> AppResult<String> {
        let uptime_str = self.run_cli("uptime").unwrap_or_else(|_| "0".to_string());
        let uptime = uptime_str.trim().parse().unwrap_or(0);
        let blockcount = self.run_cli("getblockcount")?.trim().to_string();
        let bestblockhash = self.run_cli("getbestblockhash")?.trim().to_string();

        Ok(format!(
            "Uptime: {}\nBlock Count: {}\nBest Block Hash:\n{}",
            format_uptime(uptime),
            blockcount,
            bestblockhash
        ))
    }

    pub fn fetch_wallet_info(&self) -> AppResult<String> {
        let output = self.run_cli("getwalletinfo")?;
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
