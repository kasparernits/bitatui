use std::process::Command;

pub(crate) fn run_bitcoin_cli(command: &str) -> Result<String, Box<dyn std::error::Error>> {
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
