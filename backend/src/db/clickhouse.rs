use clickhouse::Client;
use std::sync::Arc;

pub type ClickHousePool = Arc<Client>;

#[derive(clickhouse::Row, serde::Serialize, serde::Deserialize)]
pub struct AiSignalRow {
    pub signal_id: uuid::Uuid,
    pub condition_id: String,
    pub target_side: String,
    pub confidence: f64,
    pub reasoning: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(clickhouse::Row, serde::Serialize, serde::Deserialize)]
pub struct MarketSnapshotRow {
    pub condition_id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub yes_price: f64,
    pub no_price: f64,
    pub liquidity: f64,
    pub volume_24h: f64,
}

pub fn connect(url: &str) -> anyhow::Result<ClickHousePool> {
    let client = Client::default().with_url(url);
    Ok(Arc::new(client))
}

pub async fn ping(client: &Client) -> anyhow::Result<()> {
    #[derive(clickhouse::Row, serde::Deserialize)]
    struct OneRow {
        #[allow(dead_code)]
        one: u8,
    }
    let _: OneRow = client
        .query("SELECT 1 AS one")
        .fetch_one::<OneRow>()
        .await?;
    Ok(())
}

pub async fn init_schema(client: &Client) -> anyhow::Result<()> {
    client
        .query(
            r#"
        CREATE TABLE IF NOT EXISTS market_snapshots (
            condition_id String,
            timestamp DateTime64(3, 'UTC'),
            yes_price Float64,
            no_price Float64,
            liquidity Float64,
            volume_24h Float64
        ) ENGINE = MergeTree()
        ORDER BY (condition_id, timestamp)
        "#,
        )
        .execute()
        .await?;

    client
        .query(
            r#"
        CREATE TABLE IF NOT EXISTS smart_money_trades (
            tx_hash String,
            wallet_address String,
            condition_id String,
            side String,
            price Float64,
            size Float64,
            timestamp DateTime64(3, 'UTC')
        ) ENGINE = MergeTree()
        ORDER BY (wallet_address, timestamp)
        "#,
        )
        .execute()
        .await?;

    client
        .query(
            r#"
        CREATE TABLE IF NOT EXISTS ai_signals_log (
            signal_id UUID,
            condition_id String,
            target_side String,
            confidence Float64,
            reasoning String,
            timestamp DateTime64(3, 'UTC')
        ) ENGINE = MergeTree()
        ORDER BY (timestamp, condition_id)
        "#,
        )
        .execute()
        .await?;

    Ok(())
}

pub async fn insert_ai_signal(
    client: &Client,
    signal_id: uuid::Uuid,
    condition_id: &str,
    target_side: &str,
    confidence: f64,
    reasoning: &str,
) -> anyhow::Result<()> {
    let now = chrono::Utc::now();
    let row = AiSignalRow {
        signal_id,
        condition_id: condition_id.to_string(),
        target_side: target_side.to_string(),
        confidence,
        reasoning: reasoning.to_string(),
        timestamp: now,
    };
    let mut insert = client.insert("ai_signals_log")?;
    insert.write(&row).await?;
    insert.end().await?;
    Ok(())
}

/// Insert a single market snapshot into ClickHouse.
#[allow(dead_code)]
pub async fn insert_market_snapshot(
    client: &Client,
    condition_id: &str,
    yes_price: f64,
    no_price: f64,
    liquidity: f64,
    volume_24h: f64,
) -> anyhow::Result<()> {
    let now = chrono::Utc::now();
    let row = MarketSnapshotRow {
        condition_id: condition_id.to_string(),
        timestamp: now,
        yes_price,
        no_price,
        liquidity,
        volume_24h,
    };
    let mut insert = client.insert("market_snapshots")?;
    insert.write(&row).await?;
    insert.end().await?;
    Ok(())
}

/// Batch insert multiple market snapshots.
pub async fn insert_market_snapshots_batch(
    client: &Client,
    snapshots: Vec<MarketSnapshotRow>,
) -> anyhow::Result<()> {
    if snapshots.is_empty() {
        return Ok(());
    }
    let mut insert = client.insert("market_snapshots")?;
    for row in snapshots {
        insert.write(&row).await?;
    }
    insert.end().await?;
    Ok(())
}

pub async fn fetch_market_history(
    client: &Client,
    condition_id: &str,
    _range_hours: u32,
) -> anyhow::Result<Vec<MarketSnapshotRow>> {
    let sql = "SELECT condition_id, timestamp, yes_price, no_price, liquidity, volume_24h FROM market_snapshots WHERE condition_id = ? AND timestamp >= now() - toIntervalHour(24) ORDER BY timestamp ASC";
    let rows = client
        .query(sql)
        .bind(condition_id)
        .fetch_all::<MarketSnapshotRow>()
        .await?;
    Ok(rows)
}

pub async fn fetch_recent_ai_signals(
    client: &Client,
    limit: u32,
) -> anyhow::Result<Vec<AiSignalRow>> {
    let sql = format!(
        "SELECT signal_id, condition_id, target_side, confidence, reasoning, timestamp FROM ai_signals_log ORDER BY timestamp DESC LIMIT {}",
        limit
    );
    let rows = client
        .query(&sql)
        .fetch_all::<AiSignalRow>()
        .await?;
    Ok(rows)
}

/// Row structure for smart_money_trades table
#[derive(clickhouse::Row, serde::Serialize, serde::Deserialize)]
pub struct SmartMoneyTradeRow {
    pub tx_hash: String,
    pub wallet_address: String,
    pub condition_id: String,
    pub side: String,
    pub price: f64,
    pub size: f64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Insert a single smart money trade into ClickHouse.
#[allow(dead_code)]
pub async fn insert_smart_money_trade(
    client: &Client,
    tx_hash: &str,
    wallet_address: &str,
    condition_id: &str,
    side: &str,
    price: f64,
    size: f64,
    timestamp: u64,
) -> anyhow::Result<()> {
    // Safely convert Unix timestamp to DateTime, fallback to current time if invalid
    let ts = chrono::DateTime::from_timestamp(timestamp as i64, 0)
        .filter(|t| t.timestamp() > 0 && t.timestamp() < 4102444800) // Sanity check: between 1970 and 2100
        .unwrap_or_else(chrono::Utc::now);

    let row = SmartMoneyTradeRow {
        tx_hash: tx_hash.to_string(),
        wallet_address: wallet_address.to_string(),
        condition_id: condition_id.to_string(),
        side: side.to_string(),
        price,
        size,
        timestamp: ts,
    };

    let mut insert = client.insert("smart_money_trades")?;
    insert.write(&row).await?;
    insert.end().await?;
    Ok(())
}

/// Batch insert multiple smart money trades.
pub async fn insert_smart_money_trades_batch(
    client: &Client,
    trades: Vec<SmartMoneyTradeRow>,
) -> anyhow::Result<()> {
    if trades.is_empty() {
        return Ok(());
    }
    let mut insert = client.insert("smart_money_trades")?;
    for row in trades {
        insert.write(&row).await?;
    }
    insert.end().await?;
    Ok(())
}

/// Fetch recent smart money trades for a specific market condition.
pub async fn fetch_smart_money_trades(
    client: &Client,
    condition_id: &str,
    limit: u32,
) -> anyhow::Result<Vec<SmartMoneyTradeRow>> {
    let sql = format!(
        "SELECT tx_hash, wallet_address, condition_id, side, price, size, timestamp \
         FROM smart_money_trades \
         WHERE condition_id = ? \
         ORDER BY timestamp DESC \
         LIMIT {}",
        limit
    );
    let rows = client
        .query(&sql)
        .bind(condition_id)
        .fetch_all::<SmartMoneyTradeRow>()
        .await?;
    Ok(rows)
}
