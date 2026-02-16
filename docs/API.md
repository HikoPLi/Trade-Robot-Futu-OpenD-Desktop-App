# Internal API

This document describes internal interfaces between the Tauri UI host and the Rust engine.

## IPC Commands (Tauri)

Defined in `apps/desktop/src-tauri/src/commands.rs`.

### Engine State + Trading

- `engine_snapshot() -> EngineSnapshot`
- `engine_set_watchlist(symbols: string[])`
- `engine_get_candles(symbol: string, interval_sec: number, limit: number) -> Candle[]`
- `engine_place_order(req: OrderRequest) -> Order`
- `engine_cancel_order(order_id: string) -> Order`
- `engine_engage_kill_switch(reason: string)`

### Risk + Profile

- `engine_update_risk_limits(limits: RiskLimits)`
- `engine_set_active_profile(profile: string)`
- `engine_set_kill_switch_hotkey(hotkey: string)`
  - registers new shortcut before removing old shortcut (best-effort safety)
- `engine_update_time_controls(time_controls: TimeControls)`

### OpenD Connectivity

- `engine_update_opend_config(opend: OpenDConfig)`
- `engine_update_opend_trade_env(env: OpenDTradeEnv)`
- `engine_test_opend_connection()`
  - performs OpenD protocol handshake connectivity validation

### Secrets (OS Keychain)

- `engine_secret_status(key: string) -> boolean`
- `engine_set_secret(key: string, value: string)`
- `engine_clear_secret(key: string)`

### Strategies

- `strategy_catalog() -> StrategyCatalog`
- `engine_start_strategy(req: StartStrategyRequest) -> instance_id`
- `engine_stop_strategy(instance_id: string)`
- `engine_list_strategy_defs() -> StrategyDefinition[]`
- `engine_upsert_strategy_def(req: StrategyUpsertRequest) -> StrategyDefinition`
- `engine_delete_strategy_def(id: string)`
- `engine_start_strategy_def(id: string) -> instance_id`

### Models

- `model_catalog() -> ModelCatalog`
- `engine_list_models() -> RegisteredModel[]`
- `engine_register_model(req: ModelRegisterRequest) -> RegisteredModel`
- `engine_delete_model(id: string)`
- `engine_evaluate_model(params: ModelEvalParams) -> ModelEvalRunResult`

### Model API Routing (LLM Providers)

- `engine_update_ai_provider(provider: AiProviderConfig)`
- `engine_update_ai_router(router: AiRouterConfig)`
- `engine_test_ai_provider(provider_id: string) -> AiSignalResponse`
- `engine_generate_ai_signal(req: AiSignalRequest) -> AiSignalResponse`
  - provider route is applied in order: `primary` then `fallbacks`
  - if all providers fail, engine-side caller should choose safe behavior (`hold` / no-order)

### Backtest + Audit

- `engine_generate_sample_candles_csv(symbol, interval_sec, limit) -> path`
- `engine_run_backtest(params: BacktestParams) -> BacktestRunResult`
- `engine_list_audit_events(limit, offset, event_type?, trace_id?) -> AuditEventRow[]`
- `engine_export_audit_jsonl() -> path`

### Live Unlock Workflow

- `enable_live_trading_unlock(confirmation_phrase, risk_non_default, kill_switch_configured)`
  - additional server-side checks include:
    - active profile must be `live`
    - OpenD trade env must be `real`
    - keychain secret `futu.trade_password` must exist
    - OpenD connection + global state check
    - OpenD trade unlock call

## Event Stream (Engine -> UI)

The host emits `engine_event` events to frontend subscribers.

Event variants (`crates/shared/src/lib.rs`):

- `quote` (`Quote`)
- `candle` (`Candle`)
- `order_updated` (`Order`)
- `fill` (`Fill`)
- `position_updated` (`Position`)
- `signal_fired` (`{ ts, trace_id, strategy_id, symbol, order, reason }`)
- `risk_halt` (`{ reason }`)
- `info` (`{ message }`)

## Shared Contract Sources

- Rust contract types: `crates/shared/src/lib.rs`
- TypeScript schemas (Zod): `packages/shared/src/index.ts`

These two packages are the authoritative data contract boundary for IPC and UI parsing.
