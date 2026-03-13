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
        let _ = clickhouse::init_schema(&clickhouse_client).await;
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
        redis_pool,
        config.gamma_api_base.clone(),
    ));

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
