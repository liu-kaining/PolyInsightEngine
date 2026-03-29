use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub redis_url: String,
    pub clickhouse_url: String,
    pub gamma_api_base: String,
    /// Polymarket CLOB HTTP base (midpoint / last-trade pricing).
    pub clob_api_base: String,
    /// Polymarket (or Goldsky) subgraph HTTP endpoint for on-chain trades.
    pub polymarket_subgraph_url: String,
    pub llm_base_url: Option<String>,
    pub llm_api_key: Option<String>,
    pub llm_model: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            port: env::var("PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(8080),
            redis_url: env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into()),
            clickhouse_url: env::var("CLICKHOUSE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8123".into()),
            gamma_api_base: env::var("GAMMA_API_BASE")
                .unwrap_or_else(|_| "https://gamma-api.polymarket.com".into()),
            clob_api_base: env::var("CLOB_API_BASE")
                .unwrap_or_else(|_| "https://clob.polymarket.com".into()),
            polymarket_subgraph_url: env::var("POLYMARKET_SUBGRAPH_URL").unwrap_or_else(|_| {
                "https://api.thegraph.com/subgraphs/name/polymarket/polymarket".into()
            }),
            llm_base_url: env::var("LLM_BASE_URL").ok().filter(|s| !s.is_empty()),
            llm_api_key: env::var("LLM_API_KEY").ok().filter(|s| !s.is_empty()),
            llm_model: env::var("LLM_MODEL").unwrap_or_else(|_| "gpt-4o-mini".into()),
        }
    }

    pub fn llm_configured(&self) -> bool {
        self.llm_base_url.is_some() && self.llm_api_key.as_ref().map_or(false, |k| !k.is_empty())
    }
}
