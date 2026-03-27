use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// GraphQL endpoint for Polymarket Subgraph (Polygon)
/// Note: The actual subgraph URL may vary; this is a placeholder for the MVP.
const POLYMARKET_SUBGRAPH_URL: &str = "https://api.thegraph.com/subgraphs/name/polymarket/polymarket";

/// Threshold for "whale" trades (in USD)
pub const WHALE_TRADE_THRESHOLD: f64 = 10_000.0;

/// A single trade record from Polymarket Subgraph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhaleTrade {
    pub tx_hash: String,
    pub wallet_address: String,
    pub condition_id: String,
    pub side: String, // "YES" or "NO"
    pub price: f64,
    pub size: f64,    // Trade amount in USD
    pub timestamp: u64,
}

/// GraphQL response wrapper for transactions query
#[derive(Debug, Deserialize)]
struct GraphQlResponse {
    data: Option<TransactionsData>,
    errors: Option<Vec<GraphQlError>>,
}

#[derive(Debug, Deserialize)]
struct TransactionsData {
    transactions: Vec<TransactionNode>,
}

#[derive(Debug, Deserialize)]
struct TransactionNode {
    id: String,
    #[serde(default)]
    user: Option<UserNode>,
    #[serde(rename = "conditionId", default)]
    condition_id: Option<String>,
    #[serde(rename = "tradeAmount", default)]
    trade_amount: Option<String>,
    #[serde(default)]
    price: Option<String>,
    #[serde(default)]
    timestamp: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UserNode {
    id: String,
}

#[derive(Debug, Deserialize)]
struct GraphQlError {
    message: String,
}

/// Fetch the latest whale trades from Polymarket Subgraph via GraphQL.
/// Falls back to mock data if the subgraph is unavailable or returns errors.
pub async fn fetch_latest_whale_trades() -> anyhow::Result<Vec<WhaleTrade>> {
    // Try to fetch from the real subgraph first
    match fetch_from_subgraph().await {
        Ok(trades) if !trades.is_empty() => {
            tracing::info!("smart_money: fetched {} trades from subgraph", trades.len());
            return Ok(trades);
        }
        Ok(_) => {
            tracing::warn!("smart_money: subgraph returned empty, using mock data");
        }
        Err(e) => {
            tracing::warn!("smart_money: subgraph fetch failed ({}), using mock data", e);
        }
    }

    // Fallback to mock data for MVP demonstration
    Ok(generate_mock_whale_trades(5))
}

/// Actual GraphQL query to Polymarket subgraph
async fn fetch_from_subgraph() -> anyhow::Result<Vec<WhaleTrade>> {
    let query = r#"
    {
        transactions(first: 20, orderBy: timestamp, orderDirection: desc) {
            id
            user {
                id
            }
            conditionId
            tradeAmount
            price
            timestamp
        }
    }
    "#;

    let client = reqwest::Client::new();
    let response = client
        .post(POLYMARKET_SUBGRAPH_URL)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({ "query": query }))
        .send()
        .await?;

    if !response.status().is_success() {
        anyhow::bail!("GraphQL request failed with status: {}", response.status());
    }

    let gql_response: GraphQlResponse = response.json().await?;

    // Check for GraphQL errors
    if let Some(errors) = gql_response.errors {
        let msg = errors.first().map(|e| e.message.as_str()).unwrap_or("Unknown GraphQL error");
        anyhow::bail!("GraphQL error: {}", msg);
    }

    let data = gql_response.data.ok_or_else(|| anyhow::anyhow!("No data in response"))?;

    // Parse transactions into WhaleTrade
    let trades: Vec<WhaleTrade> = data
        .transactions
        .into_iter()
        .filter_map(|tx| {
            let tx_hash = tx.id;
            let wallet_address = tx.user?.id.unwrap_or_else(|| "0xunknown".to_string());
            let condition_id = tx.condition_id.unwrap_or_else(|| "0xunknown_condition".to_string());

            // Parse numeric fields, defaulting to reasonable values
            let trade_amount: f64 = tx.trade_amount
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);

            let price: f64 = tx.price
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.5);

            let timestamp: u64 = tx.timestamp
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(now_ts);

            // Determine side based on price relative to 0.5
            let side = if price >= 0.5 { "YES".to_string() } else { "NO".to_string() };

            Some(WhaleTrade {
                tx_hash,
                wallet_address,
                condition_id,
                side,
                price,
                size: trade_amount,
                timestamp,
            })
        })
        .collect();

    Ok(trades)
}

/// Generate mock whale trades for MVP demonstration when subgraph is unavailable
fn generate_mock_whale_trades(count: usize) -> Vec<WhaleTrade> {
    use rand::{Rng, SeedableRng};
    use rand::rngs::StdRng;

    let now = now_ts();
    let mut rng = StdRng::seed_from_u64(now);

    let mock_conditions = [
        "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
        "0xfedcba0987654321fedcba0987654321fedcba0987654321fedcba0987654321",
        "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890",
    ];

    let mock_wallets = [
        "0xwhale1a2b3c4d5e6f789012345678901234567890ab",
        "0xwhale2b3c4d5e6f789012345678901234567890abcd",
        "0xwhale3c4d5e6f789012345678901234567890abcdef",
        "0xwhale4d5e6f789012345678901234567890abcdef01",
    ];

    (0..count)
        .map(|i| {
            let condition_idx = rng.gen_range(0..mock_conditions.len());
            let wallet_idx = rng.gen_range(0..mock_wallets.len());
            let side = if rng.gen_bool(0.5) { "YES" } else { "NO" };
            let price: f64 = rng.gen_range(0.3..0.8);
            // Generate sizes above whale threshold ($10k - $500k)
            let size: f64 = rng.gen_range(10_000.0..500_000.0);
            let timestamp = now.saturating_sub(rng.gen_range(0..3600)); // Within last hour

            WhaleTrade {
                tx_hash: format!("0xtx{:016x}{:016x}", now, i),
                wallet_address: mock_wallets[wallet_idx].to_string(),
                condition_id: mock_conditions[condition_idx].to_string(),
                side: side.to_string(),
                price,
                size,
                timestamp,
            }
        })
        .collect()
}

/// Get current Unix timestamp in seconds
fn now_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_mock_trades() {
        let trades = generate_mock_whale_trades(3);
        assert_eq!(trades.len(), 3);
        for trade in trades {
            assert!(trade.size >= WHALE_TRADE_THRESHOLD);
            assert!(trade.side == "YES" || trade.side == "NO");
        }
    }
}
