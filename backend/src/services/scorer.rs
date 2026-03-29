use crate::models::LeaderboardEntry;
use crate::services::gamma::{fetch_markets_from_gamma, GammaMarket};
use polars::prelude::*;

/// APY_Score = (rewardsDailyRate / liquidity) * volume_weight
/// Uses Polars LazyFrame with vectorized expression engine as per TDD specification.
pub fn compute_leaderboard(markets: Vec<GammaMarket>, top_n: usize) -> Vec<LeaderboardEntry> {
    if markets.is_empty() {
        return Vec::new();
    }

    // Filter out markets without condition_id
    let valid_markets: Vec<_> = markets
        .into_iter()
        .filter(|m| m.condition_id.is_some())
        .collect();

    if valid_markets.is_empty() {
        return Vec::new();
    }

    let n = valid_markets.len();

    // Extract data into vectors for Polars Series
    let condition_ids: Vec<String> = valid_markets
        .iter()
        .map(|m| m.condition_id.clone().unwrap_or_default())
        .collect();

    let questions: Vec<String> = valid_markets
        .iter()
        .map(|m| m.question.clone().unwrap_or_default())
        .collect();

    let liquidity: Vec<f64> = valid_markets
        .iter()
        .map(|m| m.liquidity_safe())
        .collect();

    let volume_24h: Vec<f64> = valid_markets
        .iter()
        .map(|m| m.volume_24h())
        .collect();

    let rewards_daily: Vec<f64> = valid_markets
        .iter()
        .map(|m| m.rewards_daily_rate_safe())
        .collect();

    // Build DataFrame using Polars
    let df = match df! {
        "condition_id" => condition_ids,
        "question" => questions,
        "liquidity" => liquidity.clone(),
        "volume_24h" => volume_24h.clone(),
        "rewards_daily" => rewards_daily.clone(),
    } {
        Ok(df) => df,
        Err(e) => {
            tracing::warn!("Polars df! macro failed: {}, using fallback", e);
            return compute_leaderboard_simple(&valid_markets, top_n);
        }
    };

    // Convert to LazyFrame for expression-based computation
    let lf = df.lazy();

    // Pure Polars vectorized expression pipeline:
    // Step 1: total_volume = sum(volume_24h), clamp to 1.0 to avoid div-by-zero
    // Step 2: volume_weight = volume_24h / total_volume
    // Step 3: apy_score = (rewards_daily / liquidity) * volume_weight
    let total_vol: f64 = volume_24h.iter().sum::<f64>().max(1.0);

    let sorted_lf = lf
        .with_column(
            (col("volume_24h") / lit(total_vol)).alias("volume_weight")
        )
        .with_column(
            (when(col("liquidity").eq(lit(0.0)))
                .then(lit(0.0))
                .otherwise(col("rewards_daily") / col("liquidity"))
                * col("volume_weight"))
            .alias("apy_score"),
        )
        .sort(
            ["apy_score"],
            SortMultipleOptions::default().with_order_descending(true),
        )
        .limit(top_n as u32);

    // Collect and extract results
    match sorted_lf.collect() {
        Ok(sorted_df) => {
            let condition_id_col = sorted_df.column("condition_id").ok();
            let question_col = sorted_df.column("question").ok();
            let liquidity_col = sorted_df.column("liquidity").ok();
            let apy_col = sorted_df.column("apy_score").ok();

            let mut result = Vec::with_capacity(top_n);
            let row_count = sorted_df.height();

            for i in 0..top_n.min(row_count) {
                let condition_id = condition_id_col
                    .as_ref()
                    .and_then(|c| c.str().ok())
                    .and_then(|s| s.get(i).map(|v| v.to_string()))
                    .unwrap_or_default();

                let question = question_col
                    .as_ref()
                    .and_then(|c| c.str().ok())
                    .and_then(|s| s.get(i).map(|v| v.to_string()))
                    .unwrap_or_default();

                let liq = liquidity_col
                    .as_ref()
                    .and_then(|c| c.f64().ok())
                    .and_then(|s| s.get(i))
                    .unwrap_or(0.0);

                let apy = apy_col
                    .as_ref()
                    .and_then(|c| c.f64().ok())
                    .and_then(|s| s.get(i))
                    .unwrap_or(0.0);

                result.push(LeaderboardEntry {
                    condition_id,
                    question,
                    apy,
                    liquidity: liq,
                });
            }

            if result.is_empty() && n > 0 {
                tracing::warn!("Polars returned empty result, using fallback");
                return compute_leaderboard_simple(&valid_markets, top_n);
            }

            result
        }
        Err(e) => {
            tracing::warn!("Polars collect failed: {}, using fallback", e);
            compute_leaderboard_simple(&valid_markets, top_n)
        }
    }
}

/// Simple fallback computation without Polars (for error recovery)
fn compute_leaderboard_simple(markets: &[GammaMarket], top_n: usize) -> Vec<LeaderboardEntry> {
    let total_vol: f64 = markets.iter().map(|m| m.volume_24h()).sum::<f64>().max(1.0);

    let mut indexed: Vec<_> = markets
        .iter()
        .filter_map(|m| {
            let cid = m.condition_id.clone()?;
            let volume_weight = m.volume_24h() / total_vol;
            let apy = if m.liquidity_safe() > 0.0 {
                (m.rewards_daily_rate_safe() / m.liquidity_safe()) * volume_weight
            } else {
                0.0
            };
            Some((cid, m, apy))
        })
        .collect();

    indexed.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    indexed.truncate(top_n);

    indexed
        .into_iter()
        .map(|(condition_id, m, apy)| LeaderboardEntry {
            condition_id,
            question: m.question.clone().unwrap_or_default(),
            apy,
            liquidity: m.liquidity_safe(),
        })
        .collect()
}

/// Fetch from Gamma API and compute leaderboard; on network error return mock data.
pub async fn fetch_and_score(
    gamma_base: &str,
    top_n: usize,
) -> anyhow::Result<Vec<LeaderboardEntry>> {
    match fetch_markets_from_gamma(gamma_base).await {
        Ok(markets) => {
            let filtered: Vec<GammaMarket> = markets
                .into_iter()
                .filter(|m| m.condition_id.is_some() && m.liquidity_safe() >= 1.0)
                .collect();
            Ok(compute_leaderboard(filtered, top_n))
        }
        Err(e) => {
            tracing::warn!("Gamma fetch failed, using mock leaderboard: {}", e);
            Ok(mock_leaderboard(top_n))
        }
    }
}

fn mock_leaderboard(top_n: usize) -> Vec<LeaderboardEntry> {
    let mock = vec![
        ("0xmock_condition_1", "Will BTC reach 100k by 2026?", 0.45, 50000.0),
        ("0xmock_condition_2", "Trump wins 2024 election?", 0.38, 120000.0),
        ("0xmock_condition_3", "Fed rate cut in March?", 0.32, 80000.0),
    ];
    mock.into_iter()
        .take(top_n)
        .map(|(condition_id, question, apy, liquidity)| LeaderboardEntry {
            condition_id: condition_id.to_string(),
            question: question.to_string(),
            apy,
            liquidity,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_leaderboard_empty() {
        let r = compute_leaderboard(vec![], 20);
        assert!(r.is_empty());
    }

    #[test]
    fn test_mock_leaderboard() {
        let r = mock_leaderboard(2);
        assert_eq!(r.len(), 2);
        assert!(r[0].apy > 0.0);
    }

    #[test]
    fn test_compute_leaderboard_simple() {
        let markets = vec![
            GammaMarket {
                condition_id: Some("0x1".to_string()),
                question: Some("Test A?".to_string()),
                liquidity: Some(100000.0),
                volume: None,
                volume_24hr: Some(50000.0),
                clob_token_ids: None,
                outcomes: None,
                outcome_prices: None,
                rewards_daily_rate: Some(100.0),
                rewards_min_size: None,
                rewards_max_spread: None,
            },
            GammaMarket {
                condition_id: Some("0x2".to_string()),
                question: Some("Test B?".to_string()),
                liquidity: Some(50000.0),
                volume: None,
                volume_24hr: Some(30000.0),
                clob_token_ids: None,
                outcomes: None,
                outcome_prices: None,
                rewards_daily_rate: Some(50.0),
                rewards_min_size: None,
                rewards_max_spread: None,
            },
        ];
        let r = compute_leaderboard_simple(&markets, 10);
        assert!(!r.is_empty());
        assert_eq!(r[0].condition_id, "0x1");
    }
}
