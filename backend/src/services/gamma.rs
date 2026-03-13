use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GammaMarket {
    #[serde(alias = "condition_id")]
    pub condition_id: Option<String>,
    pub question: Option<String>,
    pub liquidity: Option<f64>,
    pub volume: Option<f64>,
    #[serde(rename = "volume24hr")]
    pub volume_24hr: Option<f64>,
    pub clob_token_ids: Option<serde_json::Value>,
    #[serde(rename = "rewardsDailyRate")]
    pub rewards_daily_rate: Option<f64>,
    #[serde(rename = "rewardsMinSize")]
    pub rewards_min_size: Option<f64>,
    #[serde(rename = "rewardsMaxSpread")]
    pub rewards_max_spread: Option<f64>,
}

impl GammaMarket {
    pub fn volume_24h(&self) -> f64 {
        self.volume_24hr.or(self.volume).unwrap_or(0.0)
    }

    pub fn liquidity_safe(&self) -> f64 {
        self.liquidity.unwrap_or(1.0).max(1.0)
    }

    pub fn rewards_daily_rate_safe(&self) -> f64 {
        self.rewards_daily_rate.unwrap_or(0.0)
    }
}

pub async fn fetch_markets_from_gamma(base_url: &str) -> anyhow::Result<Vec<GammaMarket>> {
    let url = format!("{}/markets", base_url);
    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .query(&[("closed", "false"), ("active", "true")])
        .send()
        .await?;
    let markets: Vec<GammaMarket> = resp.json().await?;
    Ok(markets)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_volume_24h_fallback() {
        let m = GammaMarket {
            condition_id: None,
            question: None,
            liquidity: None,
            volume: Some(100.0),
            volume_24hr: None,
            clob_token_ids: None,
            rewards_daily_rate: None,
            rewards_min_size: None,
            rewards_max_spread: None,
        };
        assert_eq!(m.volume_24h(), 100.0);
    }
}
