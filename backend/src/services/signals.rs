use crate::adapters::llm::{mock_alpha_signal, LlmAdapter};
use crate::db;
use crate::models::AiAlphaSignal;
use uuid::Uuid;

const STREAM_ALPHA_SIGNALS: &str = "stream:alpha_signals";

/// Generate one AI signal: call LLM or fallback to mock; persist to ClickHouse and push to Redis Stream.
pub async fn generate_and_persist_signal(
    llm: Option<&LlmAdapter>,
    redis: &db::redis::RedisPool,
    clickhouse: &db::clickhouse::ClickHousePool,
    condition_id: &str,
    context: &str,
) -> AiAlphaSignal {
    let signal = match llm {
        Some(adapter) => match adapter.generate_signal(condition_id, context).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("LLM signal failed, using mock: {}", e);
                mock_alpha_signal(condition_id)
            }
        },
        None => mock_alpha_signal(condition_id),
    };

    let signal_id = Uuid::new_v4();
    let json = serde_json::to_string(&signal).unwrap_or_default();
    let _ = db::redis::xadd(redis, STREAM_ALPHA_SIGNALS, "payload", &json).await;
    let _ = db::clickhouse::insert_ai_signal(
        clickhouse,
        signal_id,
        &signal.condition_id,
        &signal.target_side,
        signal.confidence_score,
        &signal.reasoning,
    )
    .await;

    signal
}
