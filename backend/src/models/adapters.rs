use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiAlphaSignal {
    pub condition_id: String,
    pub target_side: String,
    pub target_fair_value: f64,
    pub confidence_score: f64,
    pub reasoning: String,
    pub source_event: String,
}
