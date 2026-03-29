use serde::Deserialize;

/// Binance-compatible REST oracle: `/ticker/price?symbol=...`.
/// Used for BTC spot and other symbols; no trait layer — call methods directly.
pub struct RestOracleAdapter {
    pub base_url: String,
    pub client: reqwest::Client,
}

#[derive(Deserialize)]
struct BinanceTicker {
    #[serde(rename = "lastPrice")]
    last_price: String,
}

impl RestOracleAdapter {
    pub fn new_with_client(base_url: String, client: reqwest::Client) -> Self {
        Self { base_url, client }
    }

    /// Fetch BTC/USDT last price from a Binance-style public API.
    pub async fn fetch_btc_price(&self) -> Result<f64, anyhow::Error> {
        self.fetch_latest_price("BTCUSDT").await
    }

    /// Fetch last price for a symbol (e.g. `BTCUSDT`, `ETHUSDT`).
    pub async fn fetch_latest_price(&self, symbol: &str) -> Result<f64, anyhow::Error> {
        let sym = symbol.to_uppercase();
        let url = format!("{}/ticker/price", self.base_url.trim_end_matches('/'));
        let resp = self
            .client
            .get(&url)
            .query(&[("symbol", sym.as_str())])
            .send()
            .await?;
        if !resp.status().is_success() {
            anyhow::bail!("oracle HTTP error: {}", resp.status());
        }
        let ticker: BinanceTicker = resp.json().await?;
        Ok(ticker.last_price.parse()?)
    }
}
