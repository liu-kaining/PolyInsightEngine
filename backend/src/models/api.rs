use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    pub condition_id: String,
    pub question: String,
    pub apy: f64,
    pub liquidity: f64,
}

#[derive(Debug, Serialize)]
pub struct MarketHistoryPoint {
    pub timestamp: String,
    pub yes_price: f64,
    pub no_price: f64,
    pub liquidity: f64,
    pub volume_24h: f64,
}

#[derive(Debug, Serialize)]
pub struct SignalSummary {
    pub signal_id: String,
    pub condition_id: String,
    pub target_side: String,
    pub confidence: f64,
    pub reasoning: String,
    pub timestamp: String,
}
