use serde::{Deserialize, Serialize};

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
    pub size: f64, // Trade amount in USD
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

/// Fetch latest trades from the configured Polymarket subgraph.
///
/// - On HTTP failure, timeout, GraphQL errors, missing `data`, JSON errors: returns **`Err`**.
/// - Returns **`Ok(vec![])`** only when the subgraph responds successfully with zero rows (no recent trades).
/// - Does **not** synthesize trades, default prices, or placeholder wallets.
pub async fn fetch_latest_whale_trades(
    client: &reqwest::Client,
    subgraph_url: &str,
) -> anyhow::Result<Vec<WhaleTrade>> {
    if subgraph_url.trim().is_empty() {
        anyhow::bail!("POLYMARKET_SUBGRAPH_URL is empty; refusing to call subgraph");
    }

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

    let response = client
        .post(subgraph_url)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({ "query": query }))
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("subgraph request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("subgraph HTTP {}: {}", status, body);
    }

    let body_text = response
        .text()
        .await
        .map_err(|e| anyhow::anyhow!("subgraph body read failed: {}", e))?;

    let gql_response: GraphQlResponse = serde_json::from_str(&body_text)
        .map_err(|e| anyhow::anyhow!("subgraph JSON parse failed: {}", e))?;

    if let Some(errors) = gql_response.errors {
        let msg = errors
            .first()
            .map(|e| e.message.as_str())
            .unwrap_or("unknown GraphQL error");
        anyhow::bail!("GraphQL error: {}", msg);
    }

    let data = gql_response
        .data
        .ok_or_else(|| anyhow::anyhow!("subgraph returned no data field"))?;

    let raw_count = data.transactions.len();
    let mut trades = Vec::with_capacity(raw_count);
    let mut parse_failures = 0u32;

    for tx in data.transactions {
        match parse_transaction_node(tx) {
            Ok(t) => trades.push(t),
            Err(reason) => {
                parse_failures += 1;
                tracing::warn!("smart_money: skipped subgraph row: {}", reason);
            }
        }
    }

    if raw_count > 0 && trades.is_empty() {
        anyhow::bail!(
            "subgraph returned {} transaction rows but none parsed successfully; schema or field types may have changed",
            raw_count
        );
    }

    if parse_failures > 0 {
        tracing::warn!(
            "smart_money: parsed {} trades, {} rows failed validation",
            trades.len(),
            parse_failures
        );
    } else if !trades.is_empty() {
        tracing::info!("smart_money: fetched {} trades from subgraph", trades.len());
    }

    Ok(trades)
}

fn parse_transaction_node(tx: TransactionNode) -> Result<WhaleTrade, String> {
    let tx_hash = tx.id.trim();
    if tx_hash.is_empty() {
        return Err("missing transaction id".into());
    }

    let wallet_address = tx
        .user
        .map(|u| u.id)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| "missing user.id".to_string())?;

    let condition_id = tx
        .condition_id
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| "missing conditionId".to_string())?;

    let price_str = tx
        .price
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "missing or empty price".to_string())?;

    let price = price_str
        .parse::<f64>()
        .map_err(|e| format!("invalid price {:?}: {}", price_str, e))?;

    if !(0.0..=1.0).contains(&price) {
        return Err(format!("price out of [0,1]: {}", price));
    }

    let amount_str = tx
        .trade_amount
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "missing or empty tradeAmount".to_string())?;

    let size = amount_str
        .parse::<f64>()
        .map_err(|e| format!("invalid tradeAmount {:?}: {}", amount_str, e))?;

    if !size.is_finite() || size < 0.0 {
        return Err(format!("invalid trade size: {}", size));
    }

    let ts_str = tx
        .timestamp
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "missing or empty timestamp".to_string())?;

    let timestamp = ts_str
        .parse::<u64>()
        .map_err(|e| format!("invalid timestamp {:?}: {}", ts_str, e))?;

    // TODO: Derive YES vs NO from outcome token id / market mapping once subgraph exposes
    // unambiguous outcome or token fields. Today many Polymarket subgraph `Transaction` shapes
    // only expose a scalar `price` without outcome token id; using mid-price as a coarse proxy
    // for "which side dominated" is an MVP compromise and can mis-label multi-outcome markets.
    let side = if price >= 0.5 {
        "YES".to_string()
    } else {
        "NO".to_string()
    };

    Ok(WhaleTrade {
        tx_hash: tx_hash.to_string(),
        wallet_address,
        condition_id,
        side,
        price,
        size,
        timestamp,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_transaction_node_rejects_bad_price() {
        let tx = TransactionNode {
            id: "0xabc".to_string(),
            user: Some(UserNode {
                id: "0xw".to_string(),
            }),
            condition_id: Some("0xc".to_string()),
            trade_amount: Some("1000".to_string()),
            price: Some("not-a-float".to_string()),
            timestamp: Some("1700000000".to_string()),
        };
        assert!(parse_transaction_node(tx).is_err());
    }

    #[test]
    fn parse_transaction_node_accepts_valid_row() {
        let tx = TransactionNode {
            id: "0xabc".to_string(),
            user: Some(UserNode {
                id: "0xw".to_string(),
            }),
            condition_id: Some("0xc".to_string()),
            trade_amount: Some("25000.5".to_string()),
            price: Some("0.62".to_string()),
            timestamp: Some("1700000000".to_string()),
        };
        let t = parse_transaction_node(tx).expect("valid");
        assert_eq!(t.side, "YES");
        assert_eq!(t.price, 0.62);
        assert_eq!(t.size, 25000.5);
    }
}
