# PolyInsight Engine

A **data and signal engine** for [Polymarket](https://polymarket.com): it ingests market data from the Gamma API, scores markets by APY, persists time-series and AI-generated signals, and exposes a REST API plus an SSE stream for dashboards and downstream systems (e.g. [PolyStrike](https://github.com/liu-kaining/PolyStrike), [PolyMatrix](https://github.com/liu-kaining/PolyMatrix)).

---

## Features

- **Markets leaderboard** — Top markets by implied APY, cached in Redis and refreshed periodically.
- **Market history** — Per-condition time-series (yes/no price, liquidity, volume) from ClickHouse.
- **AI signals** — Optional LLM-based “alpha” signals with an Oracle adapter; signals are stored in Redis and ClickHouse.
- **Real-time stream** — SSE endpoint that pushes the leaderboard snapshot on an interval for live dashboards.

---

## Tech stack

| Layer        | Stack |
|-------------|--------|
| **Backend** | Rust (Axum), Redis, ClickHouse, Gamma API |
| **Frontend**| Next.js, SWR, Tailwind |
| **Data**    | Gamma API (Polymarket), optional LLM (OpenAI-compatible), optional Oracle (e.g. The Odds API) |

---

## Quick start

### 1. Data layer (Redis + ClickHouse)

```bash
docker compose up -d
```

### 2. Backend

```bash
cp .env.example .env
# Edit .env if needed (Redis/ClickHouse URLs; optional LLM/Oracle keys)

cd backend
cargo run --release
```

API base: `http://localhost:8080` (see [API routes](#api) below).

### 3. Frontend

```bash
cd frontend
npm install
npm run dev
```

Set `NEXT_PUBLIC_API_BASE=http://localhost:8080` in `.env` (or `.env.local`) if the API is not on that URL.

---

## Configuration

Copy `.env.example` to `.env` and adjust:

| Variable | Description |
|----------|-------------|
| `PORT` | Backend HTTP port (default `8080`) |
| `REDIS_URL` | Redis connection URL |
| `CLICKHOUSE_URL` | ClickHouse HTTP URL |
| `GAMMA_API_BASE` | Polymarket Gamma API base (default `https://gamma-api.polymarket.com`) |
| `CLOB_API_BASE` | Polymarket CLOB API base for order-book midpoint / last-trade prices (default `https://clob.polymarket.com`) |
| `POLYMARKET_SUBGRAPH_URL` | The Graph–hosted (or Goldsky / self-hosted) Polymarket subgraph HTTP endpoint; used to ingest on-chain large trades for the Smart Money tracker |
| `LLM_BASE_URL`, `LLM_API_KEY`, `LLM_MODEL` | Optional; omit to use mock LLM responses |
| `NEXT_PUBLIC_API_BASE` | Frontend: backend base URL (e.g. `http://localhost:8080`) |

---

## API

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Liveness |
| GET | `/api/v1/markets/leaderboard` | Top markets by APY (JSON) |
| GET | `/api/v1/markets/:condition_id/history?range=24h` | Market history (JSON) |
| GET | `/api/v1/markets/:condition_id/smart-money` | 巨鲸 / 聪明钱历史交易记录 (JSON) |
| GET | `/api/v1/signals/latest` | Latest AI signals (JSON) |
| POST | `/api/v1/signals/generate` | Generate and persist one signal (body: `condition_id`, optional `context`) |
| GET | `/api/v1/stream/markets` | SSE stream of leaderboard snapshots |

---

## Project layout

```
PolyInsightEngine/
├── backend/          # Rust API (Axum, Redis, ClickHouse, Gamma, scorer, LLM/Oracle adapters)
├── frontend/         # Next.js dashboard (leaderboard, signals, market detail, SSE)
├── spec/             # Specs and deployment guides (e.g. op.md, cost.md)
├── scripts/          # Utilities (e.g. history rewrite for Git)
├── docker-compose.yml
└── .env.example
```

---

## Production deployment

For deployment options (bare metal, GCP + Cloudflare, cost tiers), see the docs in `spec/` (e.g. `spec/op.md`, `spec/cost.md`).

---

## License

See repository license file.
