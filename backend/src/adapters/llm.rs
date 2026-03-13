use crate::models::AiAlphaSignal;
use serde::Deserialize;
use serde_json::json;

const MOCK_REASONING: &str = "Mock signal: no LLM configured or request failed.";
const MOCK_SOURCE_EVENT: &str = "system_mock";

#[derive(Debug, Clone)]
pub struct LlmAdapter {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub client: reqwest::Client,
}

#[derive(Deserialize)]
struct OpenAIMessage {
    content: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct OpenAIChoice {
    message: Option<OpenAIMessage>,
}

#[derive(Deserialize)]
struct OpenAIResponse {
    choices: Option<Vec<OpenAIChoice>>,
}

impl LlmAdapter {
    pub fn new(base_url: String, api_key: String, model: String) -> Self {
        Self {
            base_url,
            api_key,
            model,
            client: reqwest::Client::new(),
        }
    }

    /// Call OpenAI-compatible chat completion and parse response as AiAlphaSignal.
    /// Returns Err or invalid JSON -> use mock.
    pub async fn generate_signal(
        &self,
        condition_id: &str,
        context: &str,
    ) -> Result<AiAlphaSignal, anyhow::Error> {
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let body = json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": "You are a prediction market analyst. Reply with a single JSON object only, no markdown, with keys: condition_id, target_side (one of BUY_YES, BUY_NO, EXIT), target_fair_value (0-1), confidence_score (0-1), reasoning (short string), source_event (string)."}
                ,
                {"role": "user", "content": format!("Context: {}. Condition: {}. Output JSON only.", context, condition_id)}
            ],
            "temperature": 0.2
        });
        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            anyhow::bail!("LLM API error: {}", status);
        }
        let api_resp: OpenAIResponse = resp.json().await?;
        let content = api_resp
            .choices
            .and_then(|c| c.into_iter().next())
            .and_then(|c| c.message)
            .and_then(|m| m.content);
        let content_str = match content {
            Some(serde_json::Value::String(s)) => s,
            _ => anyhow::bail!("No content in LLM response"),
        };
        let signal: AiAlphaSignal = serde_json::from_str(content_str.trim())?;
        if signal.condition_id != condition_id {
            anyhow::bail!("condition_id mismatch");
        }
        Ok(signal)
    }
}

/// Produce a deterministic mock signal when LLM is unavailable or fails.
pub fn mock_alpha_signal(condition_id: &str) -> AiAlphaSignal {
    AiAlphaSignal {
        condition_id: condition_id.to_string(),
        target_side: "EXIT".to_string(),
        target_fair_value: 0.5,
        confidence_score: 0.0,
        reasoning: MOCK_REASONING.to_string(),
        source_event: MOCK_SOURCE_EVENT.to_string(),
    }
}
