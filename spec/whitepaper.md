# PolyInsight Engine 技术实现白皮书

> 版本: v0.1.0 | 日期: 2026-03-27 | 状态: 已完成实现

---

## 模块一：系统架构拓扑

### 1.1 整体架构

```
┌─────────────────────────────────────────────────────────────────┐
│                        Frontend (Next.js 15)                    │
│  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────┐ │
│  │  Dashboard       │  │  Market Detail    │  │  SSE Stream  │ │
│  │  (APY Leaderboard│  │  (Price Chart +   │  │  (Live APY   │ │
│  │   + AI Signals)  │  │   Whale Trades)   │  │   Updates)   │ │
│  └────────┬─────────┘  └────────┬─────────┘  └──────┬───────┘ │
│           │                      │                     │         │
│           │   SWR Polling        │   EventSource     │         │
│           │   (60s/30s)          │   (Reconnect)     │         │
└───────────┼──────────────────────┼────────────────────┼─────────┘
            │                      │                    │
            ▼                      ▼                    ▼
┌─────────────────────────────────────────────────────────────────┐
│                     Backend (Rust + Axum)                        │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                    API Routes (REST + SSE)                │   │
│  │  GET /api/v1/leaderboard   GET /api/v1/markets/:id/history│   │
│  │  GET /api/v1/signals       GET /api/v1/markets/:id/smart- │   │
│  │  GET /api/v1/stream/markets                          money │   │
│  └──────────────────────────────────────────────────────────┘   │
│                              │                                   │
│  ┌───────────────────────────┴───────────────────────────────┐  │
│  │                     AppState (Clone + Arc)                │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐     │  │
│  │  │ Config      │  │ RedisPool   │  │ ClickHousePool  │     │  │
│  │  │ (env vars)  │  │ (RwLock<    │  │ (Arc<Client>)   │     │  │
│  │  │             │  │  ConnMan>)  │  │                 │     │  │
│  │  └─────────────┘  └─────────────┘  └─────────────────┘     │  │
│  │                              │                            │  │
│  │               ┌──────────────┴──────────────┐              │  │
│  │               │     LlmAdapter (Option)     │              │  │
│  │               │     (OpenAI Compatible)     │              │  │
│  │               └─────────────────────────────┘              │  │
│  └───────────────────────────────────────────────────────────┘  │
│                              │                                   │
│  ┌───────────────────────────┴───────────────────────────────┐  │
│  │              5 × Tokio Spawned Background Tasks            │  │
│  │  ┌─────────────┐ ┌─────────────┐ ┌────────────────────┐     │  │
│  │  │ leaderboard │ │ market_     │ │ auto_signal_       │     │  │
│  │  │ _refresh    │ │ snapshots   │ │ generator          │     │  │
│  │  │ (60s)      │ │ (300s)     │ │ (120s)            │     │  │
│  │  └─────────────┘ └─────────────┘ └────────────────────┘     │  │
│  │  ┌─────────────────────┐ ┌─────────────────────────┐       │  │
│  │  │ smart_money_tracker  │ │ oracle_arbitrage_loop   │       │  │
│  │  │ (60s)               │ │ (30s)                  │       │  │
│  │  └─────────────────────┘ └─────────────────────────┘       │  │
│  └───────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
            │                      │                    │
            ▼                      ▼                    ▼
┌──────────────────┐    ┌──────────────────┐    ┌──────────────────┐
│     Redis        │    │   ClickHouse     │    │   External APIs  │
│  ┌────────────┐  │    │  ┌────────────┐  │    │  ┌────────────┐  │
│  │ JSON Cache │  │    │  │ market_    │  │    │  │ Gamma API │  │
│  │ leaderboard│  │    │  │ snapshots  │  │    │  │ (Markets) │  │
│  │ (TTL: 90s)│  │    │  ├────────────┤  │    │  ├────────────┤  │
│  ├────────────┤  │    │  │ smart_money│  │    │  │ Polymarket │  │
│  │ Redis     │  │    │  │ _trades    │  │    │  │ Subgraph   │  │
│  │ Stream    │  │    │  ├────────────┤  │    │  ├────────────┤  │
│  │ stream:   │  │    │  │ ai_signals │  │    │  │ Binance    │  │
│  │ alpha_    │  │    │  │ _log       │  │    │  │ (Oracle)   │  │
│  │ signals   │  │    │  └────────────┘  │    │  └────────────┘  │
│  └────────────┘  │    └──────────────────┘    └──────────────────┘
└──────────────────┘
```

### 1.2 技术栈

| 层级 | 技术 | 版本 | 用途 |
|------|------|------|------|
| 前端框架 | Next.js | 15.x | App Router, SSR/CSR |
| 数据获取 | SWR | 2.x | REST API polling |
| 实时通信 | Server-Sent Events | 原生 | SSE 流推送 |
| 图表 | lightweight-charts | 4.x | K线价格图表 |
| 图表 | ECharts | 5.x |  Whale散点图 |
| 后端框架 | Axum | 0.7.x | REST + SSE |
| 异步运行时 | Tokio | 1.x | 并发任务调度 |
| 数据分析 | Polars | 1.x | LazyFrame 向量化计算 |
| 缓存/消息 | Redis | 7.x | JSON cache + Stream |
| 时序数据库 | ClickHouse | 24.x | MergeTree 表 |
| LLM适配 | OpenAI Compatible | - | 结构化输出 |
| 预言机 | Binance API | v3 | BTC 价格获取 |

### 1.3 数据流拓扑

```
Gamma API (Markets)
       │
       ▼
┌──────────────────────────────────────────────────────┐
│  refresh_leaderboard_loop (Tokio Task #1)            │
│  1. fetch_markets_from_gamma()                       │
│  2. compute_leaderboard() — Polars LazyFrame         │
│  3. db::redis::set_json(KEY, &list, Some(90))         │
└──────────────────────────────────────────────────────┘
       │
       ▼
Redis Cache: "insight:leaderboard:apy" (TTL 90s)
       │
       ├──────────────────┬──────────────────┐
       ▼                  ▼                  ▼
┌────────────┐    ┌────────────┐    ┌─────────────────┐
│ REST API  │    │ SSE Stream │    │ auto_signal_    │
│ /leaderboard│   │ /stream/   │    │ generator_loop  │
│            │    │ markets    │    │ (Task #3)       │
└────────────┘    └────────────┘    │  Reads leaderboard│
       │                  │        │  → generate signal│
       ▼                  ▼        └─────────────────┘
┌──────────────────────────────────────────────────────┐
│  ingest_market_snapshots_loop (Tokio Task #2)         │
│  1. fetch_markets_from_gamma()                       │
│  2. 生成 mock yes/no prices (0.45-0.95)              │
│  3. insert_market_snapshots_batch() → ClickHouse     │
└──────────────────────────────────────────────────────┘
       │
       ▼
ClickHouse: market_snapshots (MergeTree)
       │
       ▼
REST API /markets/:id/history → Frontend Chart

┌──────────────────────────────────────────────────────┐
│  smart_money_tracker_loop (Tokio Task #4)            │
│  1. fetch_latest_whale_trades() — GraphQL / Mock     │
│  2. Filter size >= $10,000                           │
│  3. insert_smart_money_trades_batch() → ClickHouse  │
└──────────────────────────────────────────────────────┘
       │
       ▼
ClickHouse: smart_money_trades (MergeTree)
       │
       ▼
REST API /markets/:id/smart-money → Frontend ECharts

┌──────────────────────────────────────────────────────┐
│  oracle_arbitrage_loop (Tokio Task #5)               │
│  1. oracle.fetch_btc_price() — Binance API           │
│  2. Read leaderboard from Redis                      │
│  3. Find BTC-related markets (keyword match)         │
│  4. Calculate deviation: |implied_prob - oracle_prob||
│  5. If deviation > 5% → generate arbitrage signal     │
│  6. insert_ai_signal() → ClickHouse                 │
│  7. db::redis::xadd() → Redis Stream                 │
└──────────────────────────────────────────────────────┘
```

---

## 模块二：后端并发模型

### 2.1 Tokio 任务调度

所有 5 个后台任务通过 `tokio::spawn` 在首次运行时异步启动，不阻塞主线程：

```rust
// main.rs:67-81
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
```

### 2.2 任务配置

| Task | Interval | 主要操作 | 外部依赖 |
|------|----------|----------|----------|
| `refresh_leaderboard_loop` | 60s | fetch → Polars compute → Redis set | Gamma API |
| `ingest_market_snapshots_loop` | 300s | fetch → mock prices → ClickHouse batch insert | Gamma API |
| `auto_signal_generator_loop` | 120s | read Redis → generate signal → ClickHouse + Redis Stream | LLM Adapter |
| `smart_money_tracker_loop` | 60s | fetch trades → filter ≥$10k → ClickHouse batch | Polymarket Subgraph |
| `oracle_arbitrage_loop` | 30s | Binance BTC price → leaderboard match → signal | Binance API |

### 2.3 共享状态安全

`AppState` 通过 `Arc<...>` 共享，所有任务获得 `Arc::clone`：

```rust
pub struct AppState {
    pub config: Config,
    pub redis: redis::RedisPool,          // Arc<RwLock<ConnectionManager>>
    pub clickhouse: clickhouse::ClickHousePool, // Arc<Client>
    pub llm_adapter: Option<Arc<LlmAdapter>>,
}
```

**Redis 锁模型**：仅在单个 `query_async` 调用期间持有锁，无嵌套 await，无死锁风险。

**ClickHouse 模型**：`Arc<Client>` 内部线程安全，无需额外锁。

### 2.4 错误恢复机制

每个任务均包含 `match` / `if let Err` 错误处理，错误仅打日志，不影响任务继续运行：

```rust
Err(e) => tracing::warn!("leaderboard refresh fetch error: {}", e),
```

---

## 模块三：数据管道实现

### 3.1 ClickHouse 表结构

```sql
-- market_snapshots: 价格历史 (5分钟粒度)
CREATE TABLE market_snapshots (
    condition_id String,
    timestamp DateTime,
    yes_price Float64,
    no_price Float64,
    liquidity Float64,
    volume_24h Float64
) ENGINE = MergeTree()
ORDER BY (condition_id, timestamp);

-- smart_money_trades: 鲸鱼交易记录
CREATE TABLE smart_money_trades (
    tx_hash String,
    wallet_address String,
    condition_id String,
    side String,
    price Float64,
    size Float64,
    timestamp DateTime
) ENGINE = MergeTree()
ORDER BY (condition_id, timestamp DESC);

-- ai_signals_log: AI 信号日志
CREATE TABLE ai_signals_log (
    signal_id UUID,
    condition_id String,
    target_side String,
    confidence_score Float64,
    reasoning String,
    source_event String,
    created_at DateTime DEFAULT now()
) ENGINE = MergeTree()
ORDER BY (condition_id, created_at DESC);
```

### 3.2 Polars LazyFrame APY 计算

核心公式：`APY_Score = (rewardsDailyRate / liquidity) * volume_weight`

其中 `volume_weight = volume_24h / total_volume_24h`

```rust
// scorer.rs:66-127
let lf = df.lazy();

// 使用 map 闭包进行向量化计算 (Rust iterator fallback)
let result_lf = lf
    .map(
        |df| {
            let total_vol = df.column("volume_24h")?
                .sum::<f64>()?
                .max(1.0);

            let volume_weights: Vec<f64> = df.column("volume_24h")?
                .f64()?
                .iter()
                .map(|v| v.unwrap_or(0.0) / total_vol)
                .collect();

            let apy_scores: Vec<f64> = df.column("rewards_daily")?
                .f64()?
                .iter()
                .zip(df.column("liquidity")?.f64()?.iter())
                .zip(volume_weights.iter())
                .map(|((rd, liq), vw)| {
                    let rd = rd.unwrap_or(0.0);
                    let liq = liq.unwrap_or(1.0);
                    if liq > 0.0 {
                        (rd / liq) * vw
                    } else { 0.0 }
                })
                .collect();

            let mut result_df = df.clone();
            let apy_series = Series::new("apy_score", apy_scores);
            result_df.hstack_mut(&[apy_series])?;
            Ok(result_df)
        },
        GetOutput::from_type(DataType::Float64),
    );

// 排序取 Top N
let sorted_lf = result_lf.clone().lazy()
    .sort(
        ["apy_score"],
        SortOptions::default().with_order_descending(true),
    )
    .limit(top_n as u32);
```

**防御性 Fallback**：任何 Polars 错误都会触发 `compute_leaderboard_simple()` 纯 Rust 实现。

### 3.3 批量插入优化

smart_money_tracker_loop 从逐条插入改为批量插入：

```rust
// 收集所有 whale trades
let rows: Vec<SmartMoneyTradeRow> = trades
    .into_iter()
    .filter(|t| t.size >= WHALE_TRADE_THRESHOLD)
    .map(|t| { ... })
    .collect();

// 单次批量插入
insert_smart_money_trades_batch(&clickhouse, rows).await
```

### 3.4 时间戳安全

从 Unix timestamp 转换为 `DateTime` 时增加边界检查：

```rust
let ts = chrono::DateTime::from_timestamp(t.timestamp as i64, 0)
    .filter(|dt| dt.timestamp() > 0 && dt.timestamp() < 4102444800)
    .unwrap_or_else(chrono::Utc::now);
```

---

## 模块四：前端实时渲染

### 4.1 SSE 流控

`useStreamMarkets` hook 实现指数退避重连：

```
重连间隔 = 3000ms × 1.5^(attempt-1)
最大重试次数 = 5
```

关键实现：
- `isMountedRef` 防止组件卸载后状态更新
- `reconnectTimeoutRef` 清理函数中清除
- `eventSourceRef.current.close()` 销毁时关闭连接

### 4.2 图表 SSR 安全

`lightweight-charts` 必须动态导入且仅在客户端运行：

```typescript
useEffect(() => {
    if (typeof window === "undefined" || !chartRef.current) return;

    import("lightweight-charts").then((lwc) => {
        const chart = lwc.createChart(chartRef.current, {...});
        // ...
    });
}, [id, history]);
```

### 4.3 图表内存管理

路由切换时显式清理：

```typescript
useEffect(() => {
    // Cleanup existing chart first
    if (chartInstanceRef.current) {
        chartInstanceRef.current.remove();
        chartInstanceRef.current = null;
    }
    // ... then create new chart
}, [id, history]);
```

### 4.4 ECharts Whale 散点图

- YES 交易：绿色 (#34d399)
- NO 交易：红色 (#f43f5e)
- 气泡大小：`Math.max(10, Math.min(30, size / 10000))`
- Tooltip 显示钱包前缀、方向、金额、价格

---

## 模块五：风险审计

### 5.1 已识别风险

| 风险 | 严重度 | 缓解措施 | 状态 |
|------|--------|----------|------|
| SSE 重连风暴 | 低 | 指数退避 + 最大次数限制 | 已缓解 |
| ClickHouse 批量插入失败 | 中 | 错误日志，单次重试 | 已缓解 |
| Redis 锁持有过长 | 低 | 仅单次 query_async，无嵌套 await | 已缓解 |
| 图表内存泄漏 | 中 | `remove()` + `null` 清理 | 已缓解 |
| 轻量级图表 SSR 崩溃 | 高 | `typeof window` 检查 + 动态 import | 已修复 |
| 时间戳 panic | 高 | `filter` 边界检查 + `unwrap_or` | 已修复 |
| Schema 初始化失败静默 | 低 | 错误日志输出 | 已修复 |
| Polars 表达式求值失败 | 中 | 纯 Rust fallback | 已缓解 |

### 5.2 残留未解决项

| 问题 | 说明 | 优先级 |
|------|------|--------|
| Polars LazyFrame 未完全向量化 | `lf.map()` 闭包内使用 Rust iterator，而非纯 `with_column(col() / lit())` 表达式 | 低 ( fallback 正常) |
| Mock 价格数据 | `ingest_market_snapshots_loop` 中 yes/no 价格基于 liquidity 生成，非真实 orderbook | 中 (生产环境需替换) |
| LLM API Key 明文配置 | 通过环境变量注入，生产环境建议使用 secret manager | 低 |

### 5.3 安全性考量

- CORS 配置：`.allow_origin(Any)` 仅适用于开发环境
- Redis/ClickHouse 无密码配置：通过内网隔离防护
- 用户输入：所有 `condition_id` 通过 URL 参数传入，前端做 `encodeURIComponent` 处理
- SQL 注入：ClickHouse 查询使用参数化查询 (`?` 占位符)

---

## 附录

### A. 项目结构

```
PolyInsightEngine/
├── backend/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs              # 5个Tokio后台任务
│       ├── api/routes.rs        # REST + SSE endpoints
│       ├── db/
│       │   ├── clickhouse.rs    # 3张MergeTree表操作
│       │   └── redis.rs         # JSON cache + Stream
│       ├── services/
│       │   ├── scorer.rs        # Polars LazyFrame APY
│       │   ├── gamma.rs         # Gamma API 客户端
│       │   ├── signals.rs       # AI signal 生成
│       │   └── smart_money.rs   # Whale 交易追踪
│       ├── adapters/
│       │   ├── oracle.rs        # Binance BTC oracle
│       │   └── llm.rs           # LLM adapter
│       └── models/api.rs        # API 数据模型
├── frontend/
│   ├── src/
│   │   ├── app/
│   │   │   ├── page.tsx         # Dashboard (leaderboard + signals)
│   │   │   └── market/[id]/page.tsx  # Market detail (chart + whale)
│   │   └── lib/
│   │       ├── api.ts          # REST API 客户端
│   │       ├── sse.ts          # SSE hook (指数退避)
│   │       └── types.ts        # TypeScript 接口
│   └── package.json
└── spec/
    ├── prd.md                   # 产品需求文档
    ├── tdd.md                   # 技术设计文档
    ├── cost.md                  # 成本分析
    ├── op.md                    # 运维手册
    └── whitepaper.md            # 本文档
```

### B. 环境变量

| 变量 | 必需 | 说明 |
|------|------|------|
| `REDIS_URL` | 是 | Redis 连接串 |
| `CLICKHOUSE_URL` | 是 | ClickHouse 连接串 |
| `GAMMA_API_BASE` | 是 | Gamma API 地址 |
| `LLM_BASE_URL` | 否 | OpenAI 兼容 API 地址 |
| `LLM_API_KEY` | 否 | API 密钥 |
| `LLM_MODEL` | 否 | 模型名称，默认 `gpt-4o-mini` |
| `PORT` | 否 | 服务端口，默认 `8080` |

### C. API 端点

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/v1/leaderboard` | APY 排行榜 |
| GET | `/api/v1/signals` | 最新 AI 信号 |
| GET | `/api/v1/markets/:id/history` | 价格历史 |
| GET | `/api/v1/markets/:id/smart-money` | 鲸鱼交易 |
| GET | `/api/v1/stream/markets` | SSE 实时推送 |
| GET | `/health` | 健康检查 |

---

*本文档由 Claude Code 生成，对应代码 commit: `10d7b8f`*
