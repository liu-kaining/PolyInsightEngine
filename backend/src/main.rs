mod adapters;
mod api;
mod config;
mod db;
mod models;
mod services;

use std::net::SocketAddr;
use std::sync::Arc;

use config::Config;
use db::{clickhouse, redis};
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub redis: redis::RedisPool,
    pub clickhouse: clickhouse::ClickHousePool,
    pub llm_adapter: Option<std::sync::Arc<adapters::LlmAdapter>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env();

    let redis_pool = redis::connect(&config.redis_url).await?;
    redis::ping(&redis_pool).await?;
    tracing::info!("Redis connected");

    let clickhouse_client = clickhouse::connect(&config.clickhouse_url)?;
    if clickhouse::ping(&clickhouse_client).await.is_ok() {
        tracing::info!("ClickHouse connected");
        if let Err(e) = clickhouse::init_schema(&clickhouse_client).await {
            tracing::error!("Failed to initialize ClickHouse schema, database might be incomplete: {}", e);
        }
    } else {
        tracing::warn!("ClickHouse ping failed (optional at startup)");
    }

    let llm_adapter = match (config.llm_base_url.clone(), config.llm_api_key.clone()) {
        (Some(base_url), Some(api_key)) if !api_key.is_empty() => {
            Some(std::sync::Arc::new(adapters::LlmAdapter::new(
                base_url,
                api_key,
                config.llm_model.clone(),
            )))
        }
        _ => None,
    };

    let state = AppState {
        config: config.clone(),
        redis: redis_pool.clone(),
        clickhouse: clickhouse_client,
        llm_adapter,
    };

    tokio::spawn(refresh_leaderboard_loop(
        redis_pool.clone(),
        config.gamma_api_base.clone(),
    ));

    tokio::spawn(ingest_market_snapshots_loop(
        clickhouse_client.clone(),
        config.gamma_api_base.clone(),
    ));

    tokio::spawn(auto_signal_generator_loop(Arc::clone(&state)));

    tokio::spawn(smart_money_tracker_loop(clickhouse_client.clone()));

    tokio::spawn(oracle_arbitrage_loop(Arc::clone(&state)));

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = api::routes::routes()
        .layer(cors)
        .with_state(Arc::new(state));

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    tracing::info!("listening on {}", addr);
    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;
    Ok(())
}

async fn refresh_leaderboard_loop(redis: db::redis::RedisPool, gamma_base: String) {
    use crate::db;
    use crate::services::scorer;
    const KEY: &str = "insight:leaderboard:apy";
    const TOP_N: usize = 20;
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        match scorer::fetch_and_score(&gamma_base, TOP_N).await {
            Ok(list) => {
                if let Err(e) = db::redis::set_json(&redis, KEY, &list, Some(90)).await {
                    tracing::warn!("leaderboard refresh redis set error: {}", e);
                }
            }
            Err(e) => tracing::warn!("leaderboard refresh fetch error: {}", e),
        }
    }
}

/// Background task: ingest market snapshots into ClickHouse every 5 minutes.
/// Generates mock yes/no prices around 0.5 since Gamma API doesn't expose precise prices.
async fn ingest_market_snapshots_loop(clickhouse: db::clickhouse::ClickHousePool, gamma_base: String) {
    use crate::db::clickhouse::{insert_market_snapshots_batch, MarketSnapshotRow};
    use crate::services::gamma::fetch_markets_from_gamma;
    use chrono::Utc;
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(300)).await;
        match fetch_markets_from_gamma(&gamma_base).await {
            Ok(markets) => {
                let now = Utc::now();
                let snapshots: Vec<MarketSnapshotRow> = markets
                    .into_iter()
                    .filter_map(|m| {
                        let condition_id = m.condition_id?;
                        // Generate mock prices around 0.5 for frontend chart visualization
                        // In production, these would come from orderbook data
                        let yes_price = 0.45 + (m.liquidity_safe() % 100.0) / 200.0; // Range: 0.45 - 0.95
                        let no_price = 1.0 - yes_price;
                        Some(MarketSnapshotRow {
                            condition_id,
                            timestamp: now,
                            yes_price,
                            no_price,
                            liquidity: m.liquidity_safe(),
                            volume_24h: m.volume_24h(),
                        })
                    })
                    .collect();

                if !snapshots.is_empty() {
                    match insert_market_snapshots_batch(&clickhouse, snapshots).await {
                        Ok(_) => tracing::info!("market snapshots ingested"),
                        Err(e) => tracing::warn!("market snapshots ingest error: {}", e),
                    }
                }
            }
            Err(e) => tracing::warn!("market snapshots fetch error: {}", e),
        }
    }
}

/// Background task: automatically generate AI alpha signals for top APY markets.
/// Runs every 2 minutes, picks the #1 market from the leaderboard and generates a signal.
async fn auto_signal_generator_loop(state: Arc<AppState>) {
    use crate::models::LeaderboardEntry;
    use crate::services::signals::generate_and_persist_signal;

    const LEADERBOARD_KEY: &str = "insight:leaderboard:apy";
    const INTERVAL_SECS: u64 = 120; // 2 minutes

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(INTERVAL_SECS)).await;

        // Step 1: Read leaderboard from Redis cache
        let leaderboard: Option<Vec<LeaderboardEntry>> =
            match db::redis::get_json(&state.redis, LEADERBOARD_KEY).await {
                Ok(data) => data,
                Err(e) => {
                    tracing::warn!("auto_signal: failed to read leaderboard from redis: {}", e);
                    None
                }
            };

        // Step 2: Get top 1 market
        let top_market = match leaderboard {
            Some(ref list) if !list.is_empty() => &list[0],
            _ => {
                tracing::debug!("auto_signal: no leaderboard data available, skipping");
                continue;
            }
        };

        let condition_id = &top_market.condition_id;
        let question = &top_market.question;
        let apy = top_market.apy;

        // Step 3: Construct context for LLM analysis
        let context = format!(
            "System detected a high APY opportunity on market: \"{}\". \
             Current APY: {:.2}%, Liquidity: ${:.0}. \
             Potential triggers: volume spike, liquidity injection, or market sentiment shift. \
             Please analyze the risk/reward and provide trading recommendation.",
            question,
            apy * 100.0,
            top_market.liquidity
        );

        // Step 4: Generate and persist signal
        let signal = generate_and_persist_signal(
            state.llm_adapter.as_deref(),
            &state.redis,
            &state.clickhouse,
            condition_id,
            &context,
        )
        .await;

        tracing::info!(
            "auto_signal: generated signal for condition_id={}, target_side={}, confidence={:.2}",
            signal.condition_id,
            signal.target_side,
            signal.confidence_score
        );
    }
}

/// Background task: track smart money / whale trades from Polymarket Subgraph.
/// Runs every 60 seconds, filters trades above $10k threshold, and persists to ClickHouse.
async fn smart_money_tracker_loop(clickhouse: db::clickhouse::ClickHousePool) {
    use crate::db::clickhouse::{insert_smart_money_trades_batch, SmartMoneyTradeRow};
    use crate::services::smart_money::{fetch_latest_whale_trades, WHALE_TRADE_THRESHOLD};
    use chrono::Utc;

    const INTERVAL_SECS: u64 = 60; // 1 minute

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(INTERVAL_SECS)).await;

        // Step 1: Fetch latest trades (from subgraph or mock)
        let trades = match fetch_latest_whale_trades().await {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!("smart_money: failed to fetch trades: {}", e);
                continue;
            }
        };

        // Step 2: Filter whale trades (above threshold) and convert to rows
        let now = Utc::now();
        let rows: Vec<SmartMoneyTradeRow> = trades
            .into_iter()
            .filter(|t| t.size >= WHALE_TRADE_THRESHOLD)
            .map(|t| {
                // Safely convert timestamp with sanity check
                let ts = chrono::DateTime::from_timestamp(t.timestamp as i64, 0)
                    .filter(|dt| dt.timestamp() > 0 && dt.timestamp() < 4102444800)
                    .unwrap_or(now);
                SmartMoneyTradeRow {
                    tx_hash: t.tx_hash,
                    wallet_address: t.wallet_address,
                    condition_id: t.condition_id,
                    side: t.side,
                    price: t.price,
                    size: t.size,
                    timestamp: ts,
                }
            })
            .collect();

        if rows.is_empty() {
            tracing::debug!("smart_money: no whale trades detected");
            continue;
        }

        // Step 3: Batch persist to ClickHouse
        match insert_smart_money_trades_batch(&clickhouse, rows).await {
            Ok(_) => {
                tracing::info!("smart_money: tracked {} whale trades this cycle", rows.len());
            }
            Err(e) => {
                tracing::warn!("smart_money: failed to batch insert trades: {}", e);
            }
        }
    }
}

/// Background task: Oracle arbitrage signal generator.
/// Monitors price discrepancy between Polymarket and external oracle (Binance).
/// Generates arbitrage signals when deviation exceeds threshold.
/// PRD Module 3: "当 Polymarket 概率与外部预言机偏差过大时，生成套利信号"
async fn oracle_arbitrage_loop(state: Arc<AppState>) {
    use crate::adapters::RestOracleAdapter;
    use crate::db::clickhouse::insert_ai_signal;
    use crate::models::LeaderboardEntry;
    use uuid::Uuid;

    const INTERVAL_SECS: u64 = 30; // 30 seconds
    const ARBITRAGE_THRESHOLD: f64 = 0.05; // 5% deviation threshold
    const LEADERBOARD_KEY: &str = "insight:leaderboard:apy";

    // Binance API base URL for price oracle
    const BINANCE_API_BASE: &str = "https://api.binance.com/api/v3";

    // Create a persistent HTTP client for oracle requests
    let oracle_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .pool_idle_timeout(std::time::Duration::from_secs(30))
        .pool_max_idle_per_host(5)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let oracle = RestOracleAdapter::new_with_client(BINANCE_API_BASE.to_string(), oracle_client);

    // Keywords to identify BTC-related markets
    let btc_keywords = ["BTC", "Bitcoin", "bitcoin", "100k", "100K", "bitcoin price"];

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(INTERVAL_SECS)).await;

        // Step 1: Get current BTC price from Binance oracle
        let btc_price = match oracle.fetch_btc_price().await {
            Ok(price) => price,
            Err(e) => {
                tracing::debug!("oracle_arb: failed to fetch BTC price from Binance: {}", e);
                continue;
            }
        };

        // Step 2: Get leaderboard to find BTC-related markets
        let leaderboard: Option<Vec<LeaderboardEntry>> =
            match db::redis::get_json(&state.redis, LEADERBOARD_KEY).await {
                Ok(data) => data,
                Err(e) => {
                    tracing::debug!("oracle_arb: failed to read leaderboard: {}", e);
                    None
                }
            };

        let markets = match leaderboard {
            Some(ref list) => list,
            None => continue,
        };

        // Step 3: Find BTC-related markets and check for arbitrage
        for market in markets.iter().take(10) {
            // Check if market is BTC-related
            let is_btc_related = btc_keywords
                .iter()
                .any(|kw| market.question.contains(kw));

            if !is_btc_related {
                continue;
            }

            // Step 4: Calculate implied probability from market (simplified heuristic)
            // In production, this would come from actual orderbook data
            // For now, use APY as a proxy for market activity
            let implied_prob = if market.apy > 0.3 {
                0.7 // High APY suggests bullish sentiment
            } else if market.apy > 0.1 {
                0.5
            } else {
                0.3 // Low APY suggests bearish sentiment
            };

            // Step 5: Compare with BTC price movement
            // BTC price above 100k suggests >50% probability for "BTC > 100k" markets
            let oracle_probability = if btc_price > 100000.0 {
                0.75
            } else if btc_price > 90000.0 {
                0.55
            } else if btc_price > 80000.0 {
                0.35
            } else {
                0.2
            };

            let deviation = (implied_prob - oracle_probability).abs();

            if deviation > ARBITRAGE_THRESHOLD {
                // Step 6: Generate arbitrage signal
                let target_side = if implied_prob > oracle_probability {
                    // Market overestimates probability -> SELL YES / BUY NO
                    "BUY_NO"
                } else {
                    // Market underestimates probability -> BUY YES
                    "BUY_YES"
                };

                let confidence = (deviation * 2.0).min(0.95); // Scale confidence by deviation
                let context = format!(
                    "ORACLE ARBITRAGE ALERT: BTC price ${:.0} vs market implied probability {:.1}%. \
                     Deviation: {:.1}%. Market question: \"{}\". \
                     Oracle suggests {:.1}% probability, market implies {:.1}%. \
                     Recommendation: {} with {:.0}% confidence.",
                    btc_price,
                    implied_prob * 100.0,
                    deviation * 100.0,
                    market.question,
                    oracle_probability * 100.0,
                    implied_prob * 100.0,
                    target_side,
                    confidence * 100.0
                );

                // Persist signal to ClickHouse and push to Redis Stream
                let signal_id = Uuid::new_v4();
                let signal = crate::models::AiAlphaSignal {
                    condition_id: market.condition_id.clone(),
                    target_side: target_side.to_string(),
                    target_fair_value: oracle_probability,
                    confidence_score: confidence,
                    reasoning: format!(
                        "Oracle arb: BTC ${:.0}, deviation {:.1}%",
                        btc_price,
                        deviation * 100.0
                    ),
                    source_event: "oracle_arbitrage".to_string(),
                };

                if let Err(e) = insert_ai_signal(
                    &state.clickhouse,
                    signal_id,
                    &market.condition_id,
                    target_side,
                    confidence,
                    &signal.reasoning,
                )
                .await
                {
                    tracing::warn!("oracle_arb: failed to persist signal: {}", e);
                } else {
                    tracing::info!(
                        "oracle_arb: generated arb signal for {} - BTC ${:.0}, deviation {:.1}%, action {}",
                        &market.condition_id[..16.min(market.condition_id.len())],
                        btc_price,
                        deviation * 100.0,
                        target_side
                    );

                    // Also push to Redis Stream for downstream consumers
                    let signal_json = match serde_json::to_string(&signal) {
                        Ok(j) => j,
                        Err(e) => {
                            tracing::warn!("oracle_arb: failed to serialize signal: {}", e);
                            continue;
                        }
                    };
                    let _ = db::redis::xadd(&state.redis, "stream:alpha_signals", "payload", &signal_json).await;
                }

                // Only generate one arbitrage signal per cycle
                break;
            }
        }
    }
}
