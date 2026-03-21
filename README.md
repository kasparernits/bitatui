# bitatui 🍲

**bitatui** is a high-performance terminal UI for interacting with and monitoring your local Bitcoin node, built using Rust and [Ratatui](https://github.com/ratatui-org/ratatui).

It is designed to be fast, responsive (non-blocking), and integrated directly with your Bitcoin data directory.

## ✨ Features

-   **Non-Blocking UI**: Uses background threads and message passing (`mpsc`) to ensure the interface never freezes while fetching node data, prices, or running commands.
-   **Bitcoin Node Integration**: Defaults to `/var/bitcoin` for your configuration and data. Automatically uses `bitcoin.conf`.
-   **Node & Wallet Monitoring**: Real-time display of uptime, block count, best block hash, and wallet balance.
-   **BTC Price Tracker**: Live price updates for BTC in **USD** and **CHF** via the CoinGecko API.
-   **Interactive Command Runner**: Quickly execute common `bitcoin-cli` commands from a customizable list (`commands.json`).
-   **Address Book & QR Codes**: Store frequently used addresses and generate/edit addresses with instant QR code rendering directly in your terminal.
-   **Privacy Mode**: Toggle "Hide Amounts" (`h`) to mask sensitive wallet balances.

## 🚀 Getting Started

### Prerequisites

-   A running `bitcoind` instance.
-   `bitcoin-cli` installed and in your `PATH`.
-   Read/Write permissions to `/var/bitcoin` (or your configured `datadir`).

### Configuration

Set your RPC credentials as environment variables before launching:

```bash
export RPC_USER="your_username"
export RPC_PASSWORD="your_password"
# Optional: Specify a specific wallet
export RPC_WALLET="Mahugon"
```

### Running

```bash
cargo run
```

## 🎮 Controls

### Main Screen
-   `↑/↓`: Navigate the command list.
-   `Enter`: Run the selected `bitcoin-cli` command.
-   `j/k` or `PgUp/PgDn`: Scroll the command output.
-   `r`: Refresh node info, wallet info, and BTC prices.
-   `h`: Toggle privacy mode (mask amounts).
-   `w`: Open the **Address Book & QR Overlay**.
-   `q`: Quit the application.

### Address Book Overlay
-   `Ctrl + N`: Generate and save a new address.
-   `Ctrl + G`: Get a new address (preview only).
-   `Ctrl + C`: Copy the current address to clipboard.
-   `Ctrl + X`: Close the overlay.
-   `↑/↓`: Select a saved address from the book.
-   `←/→/Home/End/Backspace/Delete`: Edit the current address field manually.

## 🛠 Architecture

The project is structured for maintainability and scalability:
-   `src/bitcoin`: Encapsulates all `bitcoin-cli` and node interaction logic.
-   `src/api`: Handles external API requests (e.g., CoinGecko).
-   `src/app`: The central state machine and background thread orchestrator.
-   `src/ui`: Pure rendering logic using Ratatui widgets.
-   `src/error`: Centralized, idiomatic error handling using `thiserror`.

## 📜 License

This project is licensed under the MIT License - see the LICENSE file for details.

---
*Created for the Bitcoin community with 🧡 and Rust.*
