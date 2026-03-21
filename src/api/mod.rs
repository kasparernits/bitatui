use serde::Deserialize;
use crate::error::AppResult;

#[derive(Debug, Deserialize)]
struct CoinGeckoResponse {
    bitcoin: BitcoinPrices,
}

#[derive(Debug, Deserialize)]
struct BitcoinPrices {
    usd: f64,
    chf: f64,
}

pub struct PriceClient;

impl PriceClient {
    pub fn new() -> Self {
        Self
    }

    pub fn fetch_btc_prices(&self) -> AppResult<String> {
        let url = "https://api.coingecko.com/api/v3/simple/price?ids=bitcoin&vs_currencies=usd,chf";
        
        let resp: CoinGeckoResponse = ureq::get(url)
            .header("User-Agent", "bitatui/0.1.0")
            .call()?
            .body_mut()
            .read_json()?;

        Ok(format!(
            "BTC Price:\nUSD: ${:.2}\nCHF: {:.2}",
            resp.bitcoin.usd, resp.bitcoin.chf
        ))
    }
}
