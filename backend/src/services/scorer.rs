use crate::models::LeaderboardEntry;
use crate::services::gamma::{fetch_markets_from_gamma, GammaMarket};

/// APY_Score = (rewardsDailyRate / liquidity) * volume_weight - spread_penalty
/// TDD: volume_weight from volume24hr, spread_penalty from spread (we use 0 if no spread in Gamma).
pub fn compute_leaderboard(markets: Vec<GammaMarket>, top_n: usize) -> Vec<LeaderboardEntry> {
    if markets.is_empty() {
        return Vec::new();
    }

    let condition_ids: Vec<String> = markets
        .iter()
        .filter_map(|m| m.condition_id.clone())
        .collect();
    let questions: Vec<String> = markets
        .iter()
        .map(|m| m.question.clone().unwrap_or_default())
        .collect();
    let liquidity: Vec<f64> = markets.iter().map(|m| m.liquidity_safe()).collect();
    let volume_24h: Vec<f64> = markets.iter().map(|m| m.volume_24h()).collect();
    let rewards_daily: Vec<f64> = markets.iter().map(|m| m.rewards_daily_rate_safe()).collect();

    let total_vol: f64 = volume_24h.iter().sum::<f64>().max(1.0);
    let volume_weight: Vec<f64> = volume_24h
        .iter()
        .map(|v| v / total_vol)
        .collect();

    let apy_score: Vec<f64> = rewards_daily
        .iter()
        .zip(liquidity.iter())
        .zip(volume_weight.iter())
        .map(|((rd, liq), vw)| (rd / liq) * vw)
        .collect();

    let mut out: Vec<(String, String, f64, f64)> = condition_ids
        .into_iter()
        .zip(questions)
        .zip(apy_score)
        .zip(liquidity)
        .map(|(((c, q), apy), liq)| (c, q, apy, liq))
        .collect();

    out.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    out.truncate(top_n);

    out.into_iter()
        .map(|(condition_id, question, apy, liquidity)| LeaderboardEntry {
            condition_id,
            question,
            apy,
            liquidity,
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
        (
            "0xmock_condition_1",
            "Will BTC reach 100k by 2026?",
            0.45,
            50000.0,
        ),
        (
            "0xmock_condition_2",
            "Trump wins 2024 election?",
            0.38,
            120000.0,
        ),
        (
            "0xmock_condition_3",
            "Fed rate cut in March?",
            0.32,
            80000.0,
        ),
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
}
