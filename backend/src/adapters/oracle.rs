use async_trait::async_trait;
use crate::models::OracleTick;
use serde::Deserialize;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

#[async_trait]
pub trait OracleAdapter: Send + Sync {
    fn source_name(&self) -> &'static str;

    async fn fetch_latest_price(&self, symbol: &str) -> Result<f64, anyhow::Error>;

    async fn subscribe_stream(
        &self,
        symbols: Vec<String>,
        tx: mpsc::Sender<OracleTick>,
    ) -> Result<(), anyhow::Error>;
}

/// Mock oracle: returns fixed probability 0.5 for any symbol.
pub struct MockOracleAdapter;

#[async_trait]
impl OracleAdapter for MockOracleAdapter {
    fn source_name(&self) -> &'static str {
        "Mock"
    }

    async fn fetch_latest_price(&self, _symbol: &str) -> Result<f64, anyhow::Error> {
        Ok(0.5)
    }

    async fn subscribe_stream(
        &self,
        _symbols: Vec<String>,
        _tx: mpsc::Sender<OracleTick>,
    ) -> Result<(), anyhow::Error> {
        Ok(())
    }
}

/// REST oracle: fetches price from a JSON endpoint (e.g. Binance-style ticker).
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
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: reqwest::Client::new(),
        }
    }

    /// Fetch BTC price from Binance-style /ticker/price?symbol=BTCUSDT.
    pub async fn fetch_btc_price(&self) -> Result<f64, anyhow::Error> {
        let url = format!("{}/ticker/price", self.base_url.trim_end_matches('/'));
        let resp = self
            .client
            .get(&url)
            .query(&[("symbol", "BTCUSDT")])
            .send()
            .await?;
        let ticker: BinanceTicker = resp.json().await?;
        let price: f64 = ticker.last_price.parse()?;
        Ok(price)
    }
}

#[async_trait]
impl OracleAdapter for RestOracleAdapter {
    fn source_name(&self) -> &'static str {
        "Binance"
    }

    async fn fetch_latest_price(&self, symbol: &str) -> Result<f64, anyhow::Error> {
        let sym = symbol.to_uppercase();
        let url = format!("{}/ticker/price", self.base_url.trim_end_matches('/'));
        let resp = self.client.get(&url).query(&[("symbol", sym)]).send().await?;
        let ticker: BinanceTicker = resp.json().await?;
        Ok(ticker.last_price.parse()?)
    }

    async fn subscribe_stream(
        &self,
        symbols: Vec<String>,
        _tx: mpsc::Sender<OracleTick>,
    ) -> Result<(), anyhow::Error> {
        if symbols.is_empty() {
            return Ok(());
        }
        let _ = self.fetch_latest_price(symbols.first().map(|s| s.as_str()).unwrap_or("BTCUSDT")).await?;
        Ok(())
    }
}

fn unix_ts_u64() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
