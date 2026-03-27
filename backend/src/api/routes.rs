use axum::extract::{Query, Path, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{get, post};
use axum::Json;
use axum::Router;
use futures_util::stream::Stream;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio_stream::StreamExt;

use crate::db;
use crate::models::{LeaderboardEntry, MarketHistoryPoint, SignalSummary, SmartMoneyTrade};
use crate::services::{scorer, signals};
use crate::AppState;

const LEADERBOARD_REDIS_KEY: &str = "insight:leaderboard:apy";
const LEADERBOARD_TOP_N: usize = 20;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .nest("/api/v1", api_v1())
        .route("/health", get(health))
}

fn api_v1() -> Router<Arc<AppState>> {
    Router::new()
        .route("/markets/leaderboard", get(leaderboard))
        .route("/markets/:condition_id/history", get(market_history))
        .route("/markets/:condition_id/smart-money", get(market_smart_money))
        .route("/signals/latest", get(signals_latest))
        .route("/signals/generate", post(signals_generate))
        .route("/stream/markets", get(stream_markets))
}

async fn health() -> &'static str {
    "ok"
}

async fn leaderboard(State(state): State<Arc<AppState>>) -> Json<Vec<LeaderboardEntry>> {
    let cached: Option<Vec<LeaderboardEntry>> =
        match db::redis::get_json(&state.redis, LEADERBOARD_REDIS_KEY).await {
            Ok(Some(v)) => Some(v),
            _ => None,
        };
    if let Some(ref list) = cached {
        if !list.is_empty() {
            return Json(list.to_vec());
        }
    }
    match scorer::fetch_and_score(&state.config.gamma_api_base, LEADERBOARD_TOP_N).await {
        Ok(list) => {
            let _ = db::redis::set_json(
                &state.redis,
                LEADERBOARD_REDIS_KEY,
                &list,
                Some(60),
            ).await;
            Json(list)
        }
        Err(_) => Json(cached.unwrap_or_default()),
    }
}

#[derive(serde::Deserialize)]
struct HistoryQuery {
    #[serde(default = "default_range")]
    range: String,
}

fn default_range() -> String {
    "24h".to_string()
}

async fn market_history(
    State(state): State<Arc<AppState>>,
    Path(condition_id): Path<String>,
    Query(q): Query<HistoryQuery>,
) -> Json<Vec<MarketHistoryPoint>> {
    let hours = q
        .range
        .strip_suffix('h')
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(24);
    let rows = match db::clickhouse::fetch_market_history(&state.clickhouse, &condition_id, hours).await {
        Ok(r) => r,
        Err(_) => return Json(Vec::new()),
    };
    let list = rows
        .into_iter()
        .map(|r| MarketHistoryPoint {
            timestamp: r.timestamp.to_rfc3339(),
            yes_price: r.yes_price,
            no_price: r.no_price,
            liquidity: r.liquidity,
            volume_24h: r.volume_24h,
        })
        .collect();
    Json(list)
}

#[derive(serde::Deserialize)]
struct GenerateSignalBody {
    condition_id: String,
    #[serde(default)]
    context: String,
}

async fn signals_generate(
    State(state): State<Arc<AppState>>,
    Json(body): Json<GenerateSignalBody>,
) -> Json<crate::models::AiAlphaSignal> {
    let signal = signals::generate_and_persist_signal(
        state.llm_adapter.as_deref(),
        &state.redis,
        &state.clickhouse,
        &body.condition_id,
        if body.context.is_empty() {
            "No context provided."
        } else {
            &body.context
        },
    )
    .await;
    Json(signal)
}

async fn signals_latest(State(state): State<Arc<AppState>>) -> Json<Vec<SignalSummary>> {
    let list = match db::clickhouse::fetch_recent_ai_signals(&state.clickhouse, 50).await {
        Ok(rows) => rows
            .into_iter()
            .map(|r| SignalSummary {
                signal_id: r.signal_id.to_string(),
                condition_id: r.condition_id,
                target_side: r.target_side,
                confidence: r.confidence,
                reasoning: r.reasoning,
                timestamp: r.timestamp.to_rfc3339(),
            })
            .collect(),
        Err(_) => Vec::new(),
    };
    Json(list)
}

async fn market_smart_money(
    State(state): State<Arc<AppState>>,
    Path(condition_id): Path<String>,
) -> Json<Vec<SmartMoneyTrade>> {
    const LIMIT: u32 = 50;
    let list = match db::clickhouse::fetch_smart_money_trades(&state.clickhouse, &condition_id, LIMIT).await {
        Ok(rows) => rows
            .into_iter()
            .map(|r| SmartMoneyTrade {
                tx_hash: r.tx_hash,
                wallet_address: r.wallet_address,
                side: r.side,
                price: r.price,
                size: r.size,
                timestamp: r.timestamp.to_rfc3339(),
            })
            .collect(),
        Err(e) => {
            tracing::warn!("smart_money: failed to fetch trades for {}: {}", condition_id, e);
            Vec::new()
        }
    };
    Json(list)
}

async fn stream_markets(State(state): State<Arc<AppState>>) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let gamma_base = state.config.gamma_api_base.clone();
    let stream = tokio_stream::wrappers::IntervalStream::new(tokio::time::interval(Duration::from_secs(5)))
        .then(move |_| {
            let gamma_base = gamma_base.clone();
            async move {
                let list = scorer::fetch_and_score(&gamma_base, 20).await.unwrap_or_default();
                let json = serde_json::to_string(&list).unwrap_or_else(|_| "[]".into());
                Event::default().data(json)
            }
        })
        .map(Ok::<_, Infallible>);
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)).text("ping"))
}
