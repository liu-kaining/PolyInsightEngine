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
    pub outcomes: Option<serde_json::Value>,
    #[serde(rename = "outcomePrices")]
    pub outcome_prices: Option<serde_json::Value>,
    #[serde(rename = "rewardsDailyRate")]
    pub rewards_daily_rate: Option<f64>,
    #[serde(rename = "rewardsMinSize")]
    #[allow(dead_code)]
    pub rewards_min_size: Option<f64>,
    #[serde(rename = "rewardsMaxSpread")]
    #[allow(dead_code)]
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

    pub fn yes_token_id(&self) -> Option<String> {
        let token_ids = parse_string_array(self.clob_token_ids.as_ref())?;
        let outcomes = parse_string_array(self.outcomes.as_ref())?;
        let yes_index = outcomes
            .iter()
            .position(|outcome| outcome.eq_ignore_ascii_case("yes"))?;
        token_ids.get(yes_index).cloned()
    }

    pub fn yes_outcome_price(&self) -> Option<f64> {
        let outcomes = parse_string_array(self.outcomes.as_ref())?;
        let prices = parse_string_array(self.outcome_prices.as_ref())?;
        let yes_index = outcomes
            .iter()
            .position(|outcome| outcome.eq_ignore_ascii_case("yes"))?;
        prices
            .get(yes_index)
            .and_then(|price| price.parse::<f64>().ok())
            .filter(|price| (0.0..=1.0).contains(price))
    }
}

fn parse_string_array(value: Option<&serde_json::Value>) -> Option<Vec<String>> {
    let value = value?;
    match value {
        serde_json::Value::Array(items) => Some(
            items
                .iter()
                .filter_map(|item| item.as_str().map(|s| s.to_string()))
                .collect(),
        ),
        serde_json::Value::String(raw) => {
            if raw.trim().is_empty() {
                return None;
            }
            match serde_json::from_str::<Vec<String>>(raw) {
                Ok(parsed) => Some(parsed),
                Err(_) => Some(
                    raw.split(',')
                        .map(|part| part.trim().trim_matches('"').to_string())
                        .filter(|part| !part.is_empty())
                        .collect(),
                ),
            }
        }
        _ => None,
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
            outcomes: None,
            outcome_prices: None,
            rewards_daily_rate: None,
            rewards_min_size: None,
            rewards_max_spread: None,
        };
        assert_eq!(m.volume_24h(), 100.0);
    }

    #[test]
    fn test_yes_token_and_price_extraction() {
        let m = GammaMarket {
            condition_id: Some("cid".to_string()),
            question: Some("Will BTC hit 100k?".to_string()),
            liquidity: Some(1_000.0),
            volume: None,
            volume_24hr: Some(100.0),
            clob_token_ids: Some(serde_json::Value::String(
                "[\"yes-token\",\"no-token\"]".to_string(),
            )),
            outcomes: Some(serde_json::Value::String("[\"Yes\",\"No\"]".to_string())),
            outcome_prices: Some(serde_json::Value::String("[\"0.61\",\"0.39\"]".to_string())),
            rewards_daily_rate: Some(0.0),
            rewards_min_size: None,
            rewards_max_spread: None,
        };

        assert_eq!(m.yes_token_id().as_deref(), Some("yes-token"));
        assert_eq!(m.yes_outcome_price(), Some(0.61));
    }
}
