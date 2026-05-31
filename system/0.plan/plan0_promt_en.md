# PHASE 0 CODING PROMPT
Build **PHASE 0 - Data Pipeline & Preprocessing** for a Binance Futures trading desktop application.

## OBJECTIVE
Implement a **reliable, modular, realtime market data pipeline** that:
- ingests Binance Futures public market data
- separates **live candle** and **closed candle**
- validates and normalizes incoming data
- computes indicators
- stores recent data in hot cache
- persists required data to SQLite
- emits standardized internal events
- handles reconnect, gap filling, and health monitoring safely

## IN SCOPE
Implement:
- symbol filtering
- historical warm-up loader
- websocket stream manager
- REST backfill queue
- market data normalizer
- candle state manager
- indicator engine
- hot cache manager
- SQLite persistence
- data validation guard
- health monitor
- internal event publisher

## OUT OF SCOPE
Do NOT implement:
- signal generation
- setup scoring
- order execution
- position management
- account private trading logic
- portfolio management
- strategy optimization

## HARD CONSTRAINTS
You MUST use the following technologies:

- **Desktop shell**: Tauri
- **Frontend UI**: Vue 3 + TypeScript
- **Core runtime and pipeline logic**: Rust
- **Async runtime**: Tokio
- **Serialization**: Serde
- **Persistent storage**: SQLite
- **SQLite library**: SQLx or rusqlite
- **Logging**: tracing
- **Error handling**: anyhow or thiserror
- **Configuration**: TOML or YAML

You MUST NOT:
- use Python for core runtime implementation
- use Node.js as the primary backend runtime
- use Electron as desktop shell
- move pipeline logic into the frontend
- implement core logic as a monolithic single file
- bypass validation rules
- publish unconfirmed candles as confirmed downstream data

## MARKET RULES
Track only symbols that satisfy all conditions:
- Binance **USDT-M Perpetual Futures**
- listing age > 30 days
- 24h volume >= 5,000,000 USDT
- not in blacklist
- whitelist override supported

Refresh active symbol list every 24 hours.

## TIMEFRAMES
Support exactly:
- `1d`
- `4h`
- `15m`

Use Binance kline streams directly for these timeframes.
Do not manually aggregate timeframes in the first implementation.

## DATA SOURCE RULES
Use:
- **WebSocket** as the primary realtime source
- **REST API** only for:
  - historical warm-up
  - gap filling
  - reconnect recovery
  - metadata refresh

Explicitly distinguish:
- **Live Candle**
- **Closed Candle**
- **Market Events**

Only **Closed Candle** is valid confirmed input for downstream modules.

## INDICATORS
Compute at minimum:
- EMA20
- EMA50
- EMA200
- ATR14
- ADX14

Rules:
- compute per symbol and timeframe
- live candle indicators are provisional only
- closed candle indicators are confirmed
- if warm-up is insufficient:
  - `is_warmup = true`
  - `indicator_ready = false`

## VALIDATION RULES
A candle is valid only if:
- symbol is active
- timeframe is supported
- timestamp is valid
- `high >= max(open, close, low)`
- `low <= min(open, close, high)`
- `volume >= 0`

Duplicate policy:
- identify duplicate by `symbol + timeframe + open_time + event_type`

Gap policy:
- detect missing intervals
- mark affected scope as gap-pending
- pause confirmed downstream publication for that scope
- enqueue REST backfill
- resume only after reconciliation succeeds

Time sync:
- sync with Binance server time every 1 hour

## STORAGE RULES
Implement:
- **Hot Cache**: latest 200 to 500 candles per symbol per timeframe
- **Persistent Store**: SQLite for:
  - closed candles
  - indicator snapshots
  - health events
  - gap events
  - reconnect events
  - symbol registry metadata

Cold archive:
- define interface only, no full implementation required

## EVENT RULES
Implement these internal events:
- `market.candle.updated`
- `market.candle.closed`
- `market.indicator.updated`
- `market.depth.updated`
- `market.funding.updated`
- `system.health.changed`
- `system.data_gap.detected`
- `system.data_gap.resolved`

Rules:
- `market.candle.closed` is the primary downstream trigger
- all events must include timestamp
- market events must include symbol and timeframe when relevant
- event payloads must be deterministic

## HEALTH RULES
Health states:
- `Healthy`: latency < 1000 ms and no unresolved gap
- `Degraded`: latency 1000 to 5000 ms or repeated reconnects
- `Critical`: latency > 5000 ms, unresolved critical gap, or stream unavailable

Actions:
- if `Critical`, stop publishing confirmed downstream data for affected scope
- emit `system.health.changed`
- persist health transition

## SAFETY RULES
REST safety:
- all REST requests must go through a centralized queue
- support retry and exponential backoff
- keep safe margin under exchange rate limits

Reconnect safety:
- on websocket disconnect:
  - mark stream unhealthy
  - wait 5 seconds
  - retry with backoff
  - after reconnect, detect and fill missing data before resuming confirmed publication

Shutdown safety:
- flush pending writes
- persist essential state
- close streams safely

## IMPLEMENTATION STYLE
Code must be:
- modular
- typed
- testable
- restart-safe
- observable
- configuration-driven
- low-coupling

Avoid:
- hidden global mutable state
- silent failure
- hardcoded magic constants
- business logic inside UI components

## REQUIRED OUTPUTS
Return in this order:
1. architecture summary
2. module tree
3. core Rust data models
4. event contracts
5. implementation code
6. SQLite schema
7. config example
8. test plan
9. assumptions

## ACCEPTANCE CRITERIA
Implementation is acceptable only if:
- symbol filtering works correctly
- historical warm-up works
- websocket streaming works
- live and closed candles are separated
- indicators are computed correctly
- invalid data is rejected
- gaps trigger recovery workflow
- health state changes are tracked
- events are emitted consistently
- SQLite persistence works
- codebase is modular and testable

## RESPONSE RULE
Return concrete deliverables only.
Do not explain trading philosophy.
Do not expand beyond PHASE 0.
