# 👁️ PolyInsight Engine 智能数据中枢与 AI 信号引擎

**—— 产品需求与技术架构白皮书 (V2.0 Zero Tech-Debt Edition)**

## 序言：重塑预测市场的“三位一体” (The Holy Trinity)

在 Polyverse Trading Suite（预测市场量化套件）中，我们构建了业内首个由 AI Agent 驱动的闭环交易网络：

* 🎯 **PolyStrike (高频狙击)** 是“肌肉”，负责在微秒级时间内穿透订单簿，极速扣动扳机（Execution）。
* 🛡️ **PolyMatrix (做市坦克)** 是“骨骼”，负责稳健的阵地防守，默默铺设网格吸纳官方流动性奖励（Market Making）。
* 👁️ **PolyInsight (AI 大脑与全知之眼)** 则是核心的“信号发生器”。它不直接触碰资金，但它 7x24 小时阅读全网新闻、民调、以及巨鲸的底牌。它决定了步枪瞄准哪里，坦克开向何方。

**我们不赌博，我们用 AI 消除信息差，用工程榨取确定性。**

---

# 第一部分：产品需求文档 (PRD)

## 1. 产品愿景与定位

* **定位**：预测市场领域的 Bloomberg Terminal（彭博终端） + 基于大模型的 Alpha 信号发生器。
* **愿景**：让 Polymarket 上的资金暗流、概率偏差和高回报机会**肉眼可见、机器可读**，并实现从“数据洞察”到“自动扣扳机”的无缝衔接。
* **目标用户**：
1. **内部大脑**：作为 PolyStrike 和 PolyMatrix 的上游信号源（API 喂价与策略指令）。
2. **2C/2B 商业化**：为专业交易员、Crypto 基金提供数据看板 SaaS 订阅及 AI 分析简报。



## 2. 核心功能模块 (Epics)

### 📊 模块一：宏观热力与流动性监控 (Market Heatmap & Yield Radar)

* **全局资金流向榜 (Volume Spikes)**：实时监控全网数千个市场，捕获 1小时/4小时 内交易量突然放大的“异动盘口”，在前端以热力图展示。
* **做市 APY 排行榜 (Yield Leaderboard)**：
* 结合 Gamma API 的 `rewardsDailyRate`、`rewardsMinSize` 及实时盘口 Spread，秒级计算全网**做市年化收益率 (APY) 最高且竞争度最低**的 Top 20 市场。
* **联动**：一键将高优市场通过 API 推送给 PolyMatrix，引导坦克集群前往收割。



### 🤖 模块二：大模型 Alpha 信号引擎 (LLM-Driven Alpha Generators)

* **突发事件秒级解析与套利 (Event-Driven NLP Arb)**：
* 接入 Twitter (X) Firehose、彭博社、路透社的实时流。使用标准 OpenAPI 协议调用极速模型（如 DeepSeek-V3 或 GPT-4o-mini）进行高频流式阅读。
* 当突发新闻爆出（如“某官推宣布降息”），AI 立刻输出结构化 JSON 信号（包含方向与置信度）。
* **联动**：PolyStrike 收到信号，在人类散户反应过来前，瞬间吃光盘口上的所有错价单。


* **复杂结算规则的“智能判盘” (RAG-based Due Diligence)**：
* 针对结算规则极度复杂的盘口，构建 RAG（检索增强生成）知识库。AI 根据规则自主检索客观事实。
* **联动**：若 AI 判定某盘口 100% 结算为 NO，而当前价格仅 0.70，立刻指挥 PolyMatrix 重仓吃入，锁定无风险收益。



### 📡 模块三：可插拔的高阶预言机矩阵 (Pluggable Oracle Matrix)

* **政治与宏观**：接入 RealClearPolitics / FiveThirtyEight 民调数据。
* **体育赛事**：接入 **The Odds API**，聚合全球传统博彩公司（Pinnacle, Betfair）实时赔率。
* **Crypto 盘口**：接入 Binance / OKX 现货，以及 **Coinglass API** 获取期权隐含波动率（IV）与清算热力图。
* **套利发现**：当 Polymarket 的隐含概率与外部权威预言机数据产生超过设定阈值的偏差时，生成强烈的 **无风险套利信号 (Arb Signal)**。

### 🐋 模块四：链上聪明钱追踪与意图解析 (Smart Money Tracker)

* **地址画像 (Wallet Profiling)**：通过 **The Graph (GraphQL)** 免费且极速地清洗 Polymarket 历史成交，计算每个地址的胜率 (Win Rate) 与总盈亏 (Total PnL)。
* **巨鲸意图深度解析 (AI Reasoning)**：
* 当 Top 100 的“聪明钱”砸入重金时，AI 结合该地址历史风格自动生成**“异动分析简报”**。
* **输出示例**：“该地址历史胜率 82%。本次买入疑似受一小时前某民调数据微调影响。AI评估：情绪化溢价，无实质利好，建议在其上方铺设 SELL 网格。”



### 🎨 模块五：极客级数据美学控制台 (Globalized Data Aesthetics)

* **极致图表引擎**：全面采用 **TradingView Lightweight Charts**（基于 WebGL/Canvas），渲染 10 万个数据点依然保持 60FPS 丝滑流畅。
* **全球化与国际化 (i18n)**：原生支持多语言切换，面向全球用户发售。
* **暗黑极客 UI (Cyberpunk UI)**：深空灰底色、荧光色高亮、等宽数字字体（Monospace），营造极致专业的华尔街交易员压迫感。

---

# 第二部分：技术架构说明 (Technical Architecture)

为彻底杜绝技术债，PolyInsight 摒弃传统的 Python 脚本流，全面采用 **Rust + 现代数据栈 (Modern Data Stack) + 标准化 AI Adapter**，实现真正的降维打击。

## 1. 核心技术栈选型 (Zero Tech-Debt Stack)

* **底层后端与采集层**：**Rust (`tokio` + `reqwest` + `tungstenite`)**。性能极高，内存安全，完美复用 PolyStrike 的高频组件，无 GC 停顿。
* **流处理与消息总线**：**Redis Streams**。极轻量，延迟极低，支撑微秒级信号下发。
* **数据聚合引擎**：**Apache DataFusion (Rust原生)**。彻底取代 Pandas，直接在内存中以极速清洗海量 Tick 数据，内存零拷贝（Zero-copy）。
* **高性能时序数据库**：**ClickHouse**。海量 Tick 数据的终极归宿，高压缩率，毫秒级聚合上亿条 K 线数据。
* **大模型适配器**：**Standard OpenAPI Adapter (Rust)**。不被冗杂框架绑架，通过标准 HTTP 接口无缝切换 GPT-4o / Claude 3.5 / DeepSeek-V3。
* **前端与可视化**：**Next.js 15 (React) + Tailwind CSS**，结合 **TradingView Lightweight Charts** 与 **Apache ECharts**。

## 2. 全景架构拓扑图 (Ultimate System Topology)

```text
┌─────────────────────────────────────────────────────────────────────────────────┐
│                       Pluggable Oracle Matrix (可插拔预言机矩阵)                │
│  [Crypto: Binance/Coinglass]  [Sports: The Odds API]  [On-chain: The Graph]     │
└────────┬──────────────────────────────┬─────────────────────────┬───────────────┘
         │ (WebSocket)                  │ (REST)                  │ (GraphQL)
         ▼                              ▼                         ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│                        PolyInsight Core (Rust + Tokio)                          │
│                                                                                 │
│  ┌──────────────────┐  ┌─────────────────────────┐  ┌────────────────────────┐  │
│  │ Oracle Adapters  │  │  DataFusion ETL Engine  │  │   LLM Router Adapter   │  │
│  │ (统一 Trait 接口)│  │ (零拷贝/极速多线程聚合) │  │ (兼容 OpenAI 协议)     │  │
│  └────────┬─────────┘  └──────────────┬──────────┘  └──────────┬─────────────┘  │
└───────────┼───────────────────────────┼────────────────────────┼────────────────┘
            │                           │                        │
            ▼                           ▼                        ▼
┌───────────────────────┐   ┌───────────────────────┐   ┌─────────────────────────┐
│     Redis Streams     │   │     ClickHouse DB     │   │      External LLM       │
│ (微秒级热数据/信号流) │   │ (海量时序归档与分析)  │   │ (GPT-4o / DeepSeek-V3)  │
└────┬──────────────────┘   └───────────┬───────────┘   └─────────────────────────┘
     │ 信号与指令下发                   │ API 查库
     ▼                                  ▼
┌─────────────────────────┐ ┌─────────────────────────────────────────────────────┐
│    Execution Engines    │ │        PolyInsight Global Dashboard (Next.js)       │
│ ┌─────────────────────┐ │ │ • 60FPS TradingView 流动性图表 (Canvas 渲染)      │
│ │ 🎯 PolyStrike (Rust)│ │ │ • 全球化多语言支持 (i18n)                         │
│ │ 🛡️ PolyMatrix (Py)  │ │ │ • 实时做市 APY 排行榜 / 聪明钱异动监控大屏        │
│ └─────────────────────┘ │ │ • 独立的大模型研判浮窗 (AI Signal Reasoning)      │
└─────────────────────────┘ └─────────────────────────────────────────────────────┘

```

## 3. 核心子系统设计

### 3.1 极速聚合打分器 (DataFusion Scorer)

借助 Rust 的 Apache DataFusion，系统可以在极低资源占用下运行轮询任务：

1. 毫秒级从 ClickHouse 提取 24h Volume，从 Redis 提取实时 Spread 和 Liquidity。
2. 结合 Gamma `rewardsDailyRate` 进行 DataFrame 内存级运算，算出全网 5000 活跃盘口的 “Opportunity Score”。
3. 将 Top 20 热点盘口推入 `insight:matrix_targets` 队列，供 PolyMatrix 自动换仓。

### 3.2 AI 防幻觉风控闸门 (Hallucination Defense Dispatcher)

在金融交易中，**“AI 提议，规则裁决，系统执行”** 是不可逾越的底线。

* **信号严格结构化**：AI 输出必须是校验过的 JSON，包含 `suggested_side`、`target_price` 和量化的 `confidence_score`。
* **物理级规则拦截**：Dispatcher 接收到信号后，强制核对全局预算 (`GLOBAL_MAX_BUDGET`) 与硬性敞口上限。若 AI 发生幻觉导致给出越界指令，闸门将其静默拦截并向前端抛出红色警告。

### 3.3 跨引擎信号分发器 (Signal Broadcaster)

Insight 作为全知之眼，通过 **Redis Streams / PubSub** 指挥执行引擎：

* **紧急射击信号 (`alpha:strike:target`)**：当 AI 秒级判定新闻使候选人胜率暴跌，立刻下发带 `HIGH` 优先级的 JSON。PolyStrike 收到后无需思考，瞬间向目标价位清空弹夹。
* **阵地转移信号 (`alpha:matrix:rotate`)**：发现聪明钱出逃或赔率大逆转，向 PolyMatrix 下发换仓指令，后者立刻进入 `Graceful Exit` 模式，保护资金平滑撤退。

---

# 👨‍💻 落地排期与商业化演进 (Roadmap)

集齐 **PolyInsight** 后，系统即构成了**高维打击闭环**，可直接面向顶尖机构或 DAO 金库进行商业化变现。建议分三步走：

1. **Phase 1: 数据美学与彭博终端 (The Face - MVP)**
* 用 Rust 后端拉取 Gamma API，清洗生成“做市 APY 排行榜”与“聪明钱异动”。
* 采用 Next.js 极客风模板将数据推到前端。**在向资方展示或进行推特营销时，这层极致的 UI 将使底层代码的商业估值实现指数级跃升。**


2. **Phase 2: 接入 LLM 异动解说员 (The Brain)**
* 实现 OpenAI 标准 Adapter。当盘口 Volume 暴增时，自动抓取推特/新闻交由 AI 研判，并在 Dashboard 实时弹窗：“盘口异动原因：特朗普刚刚发推提及此事。”
* 实现与传统体育/Crypto 免费 API（如 The Odds API, Coinglass）的交叉比对可视化。


3. **Phase 3: 无人值守的智能闭环 (The Holy Grail)**
* 彻底打通 Redis Streams 与风控 Dispatcher 闸门。
* 让 AI 信号真正开始指挥 PolyStrike 扣动扳机，指挥 PolyMatrix 轮换网格，实现首个完全自动化的 AI Web3 对冲基金形态。