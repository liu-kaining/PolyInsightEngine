use std::collections::{HashMap, HashSet};
use std::time::Duration;

use anyhow::{Context, Result};

use crate::services::gamma::GammaMarket;

const MAX_BATCH_SIZE: usize = 500;
const MAX_RETRIES: usize = 3;

pub struct ClobClient {
    base_url: String,
    client: reqwest::Client,
}

#[derive(serde::Serialize)]
struct MidpointRequest<'a> {
    token_id: &'a str,
}

#[derive(serde::Deserialize)]
struct LastTradePriceEntry {
    token_id: String,
    price: String,
}

impl ClobClient {
    pub fn new_with_client(base_url: String, client: reqwest::Client) -> Self {
        Self { base_url, client }
    }

    pub async fn fetch_midpoints(&self, token_ids: &[String]) -> Result<HashMap<String, f64>> {
        if token_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let mut prices = HashMap::new();
        for chunk in token_ids.chunks(MAX_BATCH_SIZE) {
            let batch = self.fetch_midpoints_batch(chunk).await?;
            prices.extend(batch);
        }
        Ok(prices)
    }

    pub async fn fetch_last_trade_prices(&self, token_ids: &[String]) -> Result<HashMap<String, f64>> {
        if token_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let mut prices = HashMap::new();
        for chunk in token_ids.chunks(MAX_BATCH_SIZE) {
            let batch = self.fetch_last_trade_prices_batch(chunk).await?;
            prices.extend(batch);
        }
        Ok(prices)
    }

    pub async fn resolve_yes_prices_for_markets(
        &self,
        markets: &[GammaMarket],
    ) -> Result<HashMap<String, f64>> {
        let mut condition_prices = HashMap::new();
        let mut token_to_condition = HashMap::new();
        let mut pending_tokens = Vec::new();
        let mut seen_tokens = HashSet::new();

        for market in markets {
            let Some(condition_id) = market.condition_id.as_ref() else {
                continue;
            };

            if let Some(price) = market.yes_outcome_price() {
                condition_prices.insert(condition_id.clone(), price);
                continue;
            }

            let Some(token_id) = market.yes_token_id() else {
                continue;
            };

            token_to_condition.insert(token_id.clone(), condition_id.clone());
            if seen_tokens.insert(token_id.clone()) {
                pending_tokens.push(token_id);
            }
        }

        if pending_tokens.is_empty() {
            return Ok(condition_prices);
        }

        let mut token_prices = self.fetch_midpoints(&pending_tokens).await?;
        let unresolved_tokens: Vec<String> = pending_tokens
            .iter()
            .filter(|token_id| !token_prices.contains_key(*token_id))
            .cloned()
            .collect();

        if !unresolved_tokens.is_empty() {
            let fallback_prices = self.fetch_last_trade_prices(&unresolved_tokens).await?;
            token_prices.extend(fallback_prices);
        }

        for (token_id, condition_id) in token_to_condition {
            if let Some(price) = token_prices.get(&token_id).copied() {
                condition_prices.insert(condition_id, price);
            }
        }

        Ok(condition_prices)
    }

    async fn fetch_midpoints_batch(&self, token_ids: &[String]) -> Result<HashMap<String, f64>> {
        let url = format!("{}/midpoints", self.base_url.trim_end_matches('/'));
        let body: Vec<MidpointRequest<'_>> = token_ids
            .iter()
            .map(|token_id| MidpointRequest {
                token_id: token_id.as_str(),
            })
            .collect();

        let response: HashMap<String, String> = self
            .send_with_retry(|| {
                self.client
                    .post(&url)
                    .json(&body)
            })
            .await?
            .error_for_status()
            .context("CLOB midpoint request failed")?
            .json()
            .await
            .context("failed to deserialize CLOB midpoint response")?;

        Ok(parse_price_map(response))
    }

    async fn fetch_last_trade_prices_batch(&self, token_ids: &[String]) -> Result<HashMap<String, f64>> {
        let url = format!("{}/last-trades-prices", self.base_url.trim_end_matches('/'));
        let body: Vec<MidpointRequest<'_>> = token_ids
            .iter()
            .map(|token_id| MidpointRequest {
                token_id: token_id.as_str(),
            })
            .collect();

        let response: Vec<LastTradePriceEntry> = self
            .send_with_retry(|| {
                self.client
                    .post(&url)
                    .json(&body)
            })
            .await?
            .error_for_status()
            .context("CLOB last-trade request failed")?
            .json()
            .await
            .context("failed to deserialize CLOB last-trade response")?;

        let mut prices = HashMap::new();
        for entry in response {
            if let Ok(price) = entry.price.parse::<f64>() {
                if (0.0..=1.0).contains(&price) {
                    prices.insert(entry.token_id, price);
                }
            }
        }
        Ok(prices)
    }

    async fn send_with_retry<F>(&self, make_request: F) -> Result<reqwest::Response>
    where
        F: Fn() -> reqwest::RequestBuilder,
    {
        let mut backoff = Duration::from_millis(250);
        let mut last_error = None;

        for attempt in 1..=MAX_RETRIES {
            match make_request().send().await {
                Ok(response) if response.status().is_success() => return Ok(response),
                Ok(response) => {
                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();
                    last_error = Some(anyhow::anyhow!(
                        "HTTP {} on attempt {}: {}",
                        status,
                        attempt,
                        body
                    ));
                }
                Err(error) => {
                    last_error = Some(error.into());
                }
            }

            if attempt < MAX_RETRIES {
                tokio::time::sleep(backoff).await;
                backoff *= 2;
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("CLOB request failed without error details")))
    }
}

fn parse_price_map(raw: HashMap<String, String>) -> HashMap<String, f64> {
    let mut prices = HashMap::new();
    for (token_id, price_str) in raw {
        if let Ok(price) = price_str.parse::<f64>() {
            if (0.0..=1.0).contains(&price) {
                prices.insert(token_id, price);
            }
        }
    }
    prices
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_price_map_filters_invalid_values() {
        let raw = HashMap::from([
            ("yes".to_string(), "0.62".to_string()),
            ("bad".to_string(), "NaN".to_string()),
            ("too_high".to_string(), "1.5".to_string()),
        ]);

        let parsed = parse_price_map(raw);
        assert_eq!(parsed.get("yes"), Some(&0.62));
        assert!(!parsed.contains_key("bad"));
        assert!(!parsed.contains_key("too_high"));
    }
}
