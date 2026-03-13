# 👁️ PolyInsight Engine: Technical Design Document (TDD)

> **To Cursor / AI Coding Assistant**:
> This is the master blueprint for the PolyInsight Engine. Read this document thoroughly before generating any code. Your goal is to implement a high-performance, Rust-based data backend and a Next.js 15 frontend, connected via Axum REST/SSE APIs. Keep the system loosely coupled. Do not assume integration with external trading engines other than publishing to a Redis Stream/PubSub.

## 1. 系统概述 (System Overview)

PolyInsight 是一套面向预测市场（Polymarket）的数据中枢与大模型 Alpha 信号生成器。

* **主要职责**：抓取并清洗异构数据（链上、预言机、新闻）、计算做市 APY 排行榜、追踪聪明钱、利用 LLM 分析异动并生成结构化交易信号。
* **非职责**：不处理私钥，不直接调用交易所下单接口（与交易执行引擎物理隔离）。

### 1.1 核心业务模块 (PRD 映射)

1. **宏观热力与流动性监控**：实时拉取 Gamma API，基于 `rewardsDailyRate` 和 `liquidity` 计算 Top 20 做市 APY 榜单。
2. **可插拔预言机矩阵 (Oracle Matrix)**：标准化接入外部数据（Binance/OKX 现货，The Odds API 体育赔率等）。
3. **大模型 Alpha 引擎**：接入外部新闻源/Twitter，通过标准 OpenAPI 适配器请求大模型（如 GPT-4o），输出结构化的套利信号。
4. **极客前端美学**：Next.js + Tailwind + TradingView Lightweight Charts 展示实时深度与大模型研判简报。

---

## 2. 技术栈选型 (Technology Stack)

### 2.1 后端 (Backend - Rust)

* **Web 框架**: `axum` (提供 REST API 与 SSE 实时推送)
* **异步运行时**: `tokio`
* **HTTP 客户端**: `reqwest`
* **WebSocket 客户端**: `tokio-tungstenite`
* **数据清洗与矩阵运算**: `polars` (Rust 版，用于内存中极速计算 APY 排名与数据聚合)
* **序列化/反序列化**: `serde`, `serde_json`
* **数据库驱动**: `clickhouse-rs`, `redis` (async)

### 2.2 数据库 (Databases)

* **热数据/消息总线**: **Redis** (缓存实时 Orderbook、状态锁、发布 `stream:alpha_signals` 预留给未来交易引擎使用)。
* **时序与持久化数据**: **ClickHouse** (存储全网 Tick 历史、聪明钱交易历史、大模型分析日志)。

### 2.3 前端 (Frontend - Next.js)

* **框架**: Next.js 15 (App Router)
* **语言**: TypeScript
* **样式**: Tailwind CSS (Dark Mode 优先，极客赛博朋克风)
* **图表库**: `lightweight-charts` (TradingView 核心库，渲染 K 线与流动性深度)，`echarts` (关系图/热力图)
* **状态管理**: Zustand 或 SWR/React Query

---

## 3. 核心接口与模块设计 (Core Architecture Design)

### 3.1 可插拔预言机系统 (Oracle Matrix)

**设计目标**：统一异构数据源，便于未来无限扩展。
**Rust Trait 定义**：

```rust
#[async_trait]
pub trait OracleAdapter: Send + Sync {
    /// 预言机名称，如 "Binance", "TheOddsAPI"
    fn source_name(&self) -> &'static str;
    
    /// 获取标准化后的价格或概率 (0.0 ~ 1.0 或 具体价格)
    async fn fetch_latest_price(&self, symbol: &str) -> Result<f64, anyhow::Error>;
    
    /// 启动 WebSocket 流监听（如果支持），通过 mpsc channel 吐出标准化 Tick
    async fn subscribe_stream(&self, symbols: Vec<String>, tx: mpsc::Sender<OracleTick>);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OracleTick {
    pub source: String,
    pub symbol: String,
    pub implied_probability: f64, // 统一换算为 0~1 的概率
    pub timestamp: u64,
}

```

### 3.2 大模型适配器与信号结构 (LLM Adapter & Signal)

**设计目标**：防幻觉，强制输出结构化 JSON，作为标准 Alpha 信号。
**Rust 侧定义**：

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct AiAlphaSignal {
    pub condition_id: String,
    pub target_side: String, // "BUY_YES" | "BUY_NO" | "EXIT"
    pub target_fair_value: f64,
    pub confidence_score: f64, // 0.0 ~ 1.0 (极高置信度才会被外部执行引擎采用)
    pub reasoning: String, // 大模型给出的简短判盘理由，用于前端展示
    pub source_event: String, // 触发本次思考的原始新闻或异动
}

pub struct LlmAdapter {
    pub base_url: String, // 兼容 OpenAI 格式
    pub api_key: String,
    pub model: String,
}

```

**解耦预留口**：当生成 `AiAlphaSignal` 后，后端不仅将其存入 ClickHouse 供前端展示，同时执行 `redis_client.xadd("stream:alpha_signals", signal_json)`。未来的 PolyStrike/PolyMatrix 只需消费这个 Redis Stream 即可，实现完全解耦。

### 3.3 Polars 聚合打分器 (Yield Scorer)

**处理逻辑**：
利用 Rust 版的 `polars` 创建 DataFrame，将 Gamma API 拉取的全量市场（包含 `liquidity`, `volume24hr`, `rewardsDailyRate`）进行矩阵计算：
`APY_Score = (rewardsDailyRate / liquidity) * volume_weight - spread_penalty`
计算完成后，将 Top 20 序列化并存入 Redis Key `insight:leaderboard:apy`。

---

## 4. 数据库 Schema 设计 (Database Design)

### 4.1 ClickHouse (时序与持久化)

用于支撑前端复杂查询和历史回测。

```sql
-- 1. Polymarket 盘口快照历史
CREATE TABLE IF NOT EXISTS market_snapshots (
    condition_id String,
    timestamp DateTime64(3, 'UTC'),
    yes_price Float64,
    no_price Float64,
    liquidity Float64,
    volume_24h Float64
) ENGINE = MergeTree()
ORDER BY (condition_id, timestamp);

-- 2. 聪明钱交易追踪
CREATE TABLE IF NOT EXISTS smart_money_trades (
    tx_hash String,
    wallet_address String,
    condition_id String,
    side String, -- 'YES' or 'NO'
    price Float64,
    size Float64,
    timestamp DateTime64(3, 'UTC')
) ENGINE = MergeTree()
ORDER BY (wallet_address, timestamp);

-- 3. AI 研判信号归档
CREATE TABLE IF NOT EXISTS ai_signals_log (
    signal_id UUID,
    condition_id String,
    target_side String,
    confidence Float64,
    reasoning String,
    timestamp DateTime64(3, 'UTC')
) ENGINE = MergeTree()
ORDER BY (timestamp, condition_id);

```

### 4.2 Redis 缓存与事件总线 (Cache & PubSub)

* `insight:market:latest:{condition_id}` -> HASH (存储最新的盘口信息，供 Axum 极速读取)。
* `insight:leaderboard:apy` -> JSON (Polars 计算出的 Top 20 APY 列表，每分钟更新)。
* `stream:alpha_signals` -> Redis Stream (留给未来外部交易引擎的下发通道)。

---

## 5. API 接口设计 (Axum API for Frontend)

所有接口前缀：`/api/v1`

### 5.1 REST API

1. `GET /api/v1/markets/leaderboard`
* **功能**: 获取当前做市收益率（APY）最高的 Top 20 市场。
* **返回**: JSON Array (带 `condition_id`, `question`, `apy`, `liquidity`)。


2. `GET /api/v1/markets/:condition_id/history?range=24h`
* **功能**: 从 ClickHouse 提取历史价格，用于渲染前端 K 线/折线图。


3. `GET /api/v1/signals/latest`
* **功能**: 获取最近一小时内 AI 判定生成的 Alpha 信号简报。



### 5.2 Server-Sent Events (SSE) 推送

* `GET /api/v1/stream/markets`
* **功能**: 前端连上 SSE 后，Rust 后端通过 Tokio Channel 接收 Redis 中的最新价格变化，以增量方式（Delta）推送给 Next.js 前端。
* **目的**: 驱动前端 TradingView 图表和盘口深度的 60FPS 平滑跳动，不需要前端疯狂发 HTTP 请求。



---

## 6. 前端架构设计 (Next.js Frontend)

**页面路由结构**：

1. `/` (Dashboard Index): 宏观热力图概览，核心分为左侧【做市 APY 排行榜】，右侧【最新 AI 异动研判信息流】。
2. `/market/[id]`: 单个盘口的“彭博终端”视图。
* 上方：TradingView 折线图（展示 Polymarket 价格与外部 Oracle 价格的交叉对比）。
* 下方左侧：实时 Orderbook 深度图。
* 下方右侧：该盘口的“聪明钱”资金流入流出分布（ECharts）。



**UI 视觉规范 (Tailwind)**：

* 主背景色：`bg-slate-950` 或纯黑 `#0a0a0a`。
* 边框：极其细微的深色边框 `border-slate-800/50`，卡片带微弱发光阴影。
* 字体：全局 `Inter`，数字部分一律使用等宽字体 `font-mono` (如 Roboto Mono)。
* 点缀色：做多/买入/信号强用 `text-emerald-400`，做空/风险用 `text-rose-500`。

---

## 7. Cursor 执行指南 (Implementation Phases for Cursor)

> **To Cursor**: Execute the implementation in the following phases. Do not proceed to the next phase until the current one is fully functional.

* **Phase 1: Project Scaffolding & Infra**
* Initialize the Rust project (`cargo init backend`).
* Initialize the Next.js project (`npx create-next-app@latest frontend --typescript --tailwind --app`).
* Create `docker-compose.yml` containing Redis and ClickHouse services.


* **Phase 2: Rust Backend - Core & Database**
* Set up Axum server and route structure.
* Implement ClickHouse and Redis connection pools.
* Implement the DataFusion/Polars script to mock calculate APY from a dummy JSON file and store it in Redis.


* **Phase 3: Rust Backend - Oracle & LLM Adapters**
* Implement `OracleAdapter` trait. Create a mock implementation that fetches standard Crypto prices via REST.
* Implement `LlmAdapter` using `reqwest` that calls an OpenAI-compatible endpoint, prompting it to return the `AiAlphaSignal` JSON schema.


* **Phase 4: Backend API & SSE**
* Implement the REST endpoints defined in section 5.1.
* Implement the SSE endpoint (`/api/v1/stream/markets`) using `axum::response::sse`.


* **Phase 5: Next.js Frontend - UI & Integration**
* Create the Dark Mode Cyberpunk layout.
* Integrate `lightweight-charts` on the Market Detail page.
* Consume the REST APIs for the Leaderboard and the SSE endpoint for live price updates.
* Render the `AiAlphaSignal` outputs in a visually striking "AI Reasoning" feed.