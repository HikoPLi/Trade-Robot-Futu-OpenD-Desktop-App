import { invoke } from "@tauri-apps/api/core";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { listen } from "@tauri-apps/api/event";
import {
  AiProviderConfigSchema,
  AiRouterConfigSchema,
  AiSignalRequestSchema,
  AiSignalResponseSchema,
  AuditEventRowSchema,
  BacktestRunResultSchema,
  EngineSnapshotSchema,
  ModelCatalogSchema,
  ModelEvalRunResultSchema,
  ModelRegisterRequestSchema,
  OrderRequestSchema,
  OrderSchema,
  StartStrategyRequestSchema,
  StrategyCatalogSchema,
  StrategyDefinitionSchema,
  StrategyUpsertRequestSchema,
  RegisteredModelSchema,
  type AiProviderConfig,
  type AiRouterConfig,
  type AiSignalRequest,
  type AiSignalResponse,
  type AuditEventRow,
  type BacktestParams,
  type BacktestRunResult,
  type EngineSnapshot,
  type ModelCatalog,
  type ModelEvalParams,
  type ModelEvalRunResult,
  type ModelRegisterRequest,
  type OpenDConfig,
  type OpenDTradeEnv,
  type Order,
  type OrderRequest,
  type RegisteredModel,
  type RiskLimits,
  type StartStrategyRequest,
  type StrategyCatalog,
  type StrategyDefinition,
  type StrategyUpsertRequest,
  type TimeControls,
} from "@trade-robot/shared";

export type EngineEvent = unknown;

export async function engineSnapshot(): Promise<EngineSnapshot> {
  const raw = await invoke("engine_snapshot");
  return EngineSnapshotSchema.parse(raw);
}

export async function engineSetWatchlist(symbols: string[]): Promise<void> {
  await invoke("engine_set_watchlist", { symbols });
}

export async function engineGetCandles(symbol: string, interval_sec: number, limit: number) {
  const raw = await invoke("engine_get_candles", { symbol, interval_sec, limit });
  // Candle parsing is handled at use-sites; keep as-is for speed.
  return raw as unknown;
}

export async function enginePlaceOrder(req: OrderRequest): Promise<Order> {
  const parsed = OrderRequestSchema.parse(req);
  const raw = await invoke("engine_place_order", { req: parsed });
  return OrderSchema.parse(raw);
}

export async function engineCancelOrder(order_id: string): Promise<Order> {
  const raw = await invoke("engine_cancel_order", { order_id });
  return OrderSchema.parse(raw);
}

export async function engineStrategyCatalog(): Promise<StrategyCatalog> {
  const raw = await invoke("strategy_catalog");
  return StrategyCatalogSchema.parse(raw);
}

export async function engineStartStrategy(req: StartStrategyRequest): Promise<string> {
  const parsed = StartStrategyRequestSchema.parse(req);
  const raw = await invoke("engine_start_strategy", { req: parsed });
  return String(raw);
}

export async function engineStopStrategy(instance_id: string): Promise<void> {
  await invoke("engine_stop_strategy", { instance_id });
}

export async function engineEngageKillSwitch(reason: string): Promise<void> {
  await invoke("engine_engage_kill_switch", { reason });
}

export async function engineUpdateRiskLimits(limits: RiskLimits): Promise<void> {
  await invoke("engine_update_risk_limits", { limits });
}

export async function engineUpdateOpenDConfig(opend: OpenDConfig): Promise<void> {
  await invoke("engine_update_opend_config", { opend });
}

export async function engineUpdateOpenDTradeEnv(env: OpenDTradeEnv): Promise<void> {
  await invoke("engine_update_opend_trade_env", { env });
}

export async function engineUpdateTimeControls(time_controls: TimeControls): Promise<void> {
  await invoke("engine_update_time_controls", { time_controls });
}

export async function engineUpdateAiProvider(provider: AiProviderConfig): Promise<void> {
  const parsed = AiProviderConfigSchema.parse(provider);
  await invoke("engine_update_ai_provider", { provider: parsed });
}

export async function engineUpdateAiRouter(router: AiRouterConfig): Promise<void> {
  const parsed = AiRouterConfigSchema.parse(router);
  await invoke("engine_update_ai_router", { router: parsed });
}

export async function engineTestAiProvider(provider_id: string): Promise<AiSignalResponse> {
  const raw = await invoke("engine_test_ai_provider", { provider_id });
  return AiSignalResponseSchema.parse(raw);
}

export async function engineGenerateAiSignal(req: AiSignalRequest): Promise<AiSignalResponse> {
  const parsed = AiSignalRequestSchema.parse(req);
  const raw = await invoke("engine_generate_ai_signal", { req: parsed });
  return AiSignalResponseSchema.parse(raw);
}

export async function engineTestOpenDConnection(): Promise<void> {
  await invoke("engine_test_opend_connection");
}

export async function engineSecretStatus(key: string): Promise<boolean> {
  const raw = await invoke("engine_secret_status", { key });
  return Boolean(raw);
}

export async function engineSetSecret(key: string, value: string): Promise<void> {
  await invoke("engine_set_secret", { key, value });
}

export async function engineClearSecret(key: string): Promise<void> {
  await invoke("engine_clear_secret", { key });
}

export async function engineGenerateSampleCandlesCsv(
  symbol: string,
  interval_sec: number,
  limit: number,
): Promise<string> {
  const raw = await invoke("engine_generate_sample_candles_csv", { symbol, interval_sec, limit });
  return String(raw);
}

export async function engineSetKillSwitchHotkey(hotkey: string): Promise<void> {
  await invoke("engine_set_kill_switch_hotkey", { hotkey });
}

export async function engineSetActiveProfile(profile: string): Promise<void> {
  await invoke("engine_set_active_profile", { profile });
}

export async function engineListAuditEvents(args: {
  limit: number;
  offset: number;
  event_type?: string;
  trace_id?: string;
}): Promise<AuditEventRow[]> {
  const raw = await invoke("engine_list_audit_events", {
    limit: args.limit,
    offset: args.offset,
    event_type: args.event_type ?? null,
    trace_id: args.trace_id ?? null,
  });
  const arr = raw as unknown[];
  return arr.map((x) => AuditEventRowSchema.parse(x));
}

export async function engineExportAuditJsonl(): Promise<string> {
  const raw = await invoke("engine_export_audit_jsonl");
  return String(raw);
}

export async function engineRunBacktest(params: BacktestParams): Promise<BacktestRunResult> {
  const raw = await invoke("engine_run_backtest", { params });
  return BacktestRunResultSchema.parse(raw);
}

export async function engineModelCatalog(): Promise<ModelCatalog> {
  const raw = await invoke("model_catalog");
  return ModelCatalogSchema.parse(raw);
}

export async function engineListStrategyDefs(): Promise<StrategyDefinition[]> {
  const raw = await invoke("engine_list_strategy_defs");
  const arr = raw as unknown[];
  return arr.map((x) => StrategyDefinitionSchema.parse(x));
}

export async function engineUpsertStrategyDef(req: StrategyUpsertRequest): Promise<StrategyDefinition> {
  const parsed = StrategyUpsertRequestSchema.parse(req);
  const raw = await invoke("engine_upsert_strategy_def", { req: parsed });
  return StrategyDefinitionSchema.parse(raw);
}

export async function engineDeleteStrategyDef(id: string): Promise<void> {
  await invoke("engine_delete_strategy_def", { id });
}

export async function engineStartStrategyDef(id: string): Promise<string> {
  const raw = await invoke("engine_start_strategy_def", { id });
  return String(raw);
}

export async function engineListModels(): Promise<RegisteredModel[]> {
  const raw = await invoke("engine_list_models");
  const arr = raw as unknown[];
  return arr.map((x) => RegisteredModelSchema.parse(x));
}

export async function engineRegisterModel(req: ModelRegisterRequest): Promise<RegisteredModel> {
  const parsed = ModelRegisterRequestSchema.parse(req);
  const raw = await invoke("engine_register_model", { req: parsed });
  return RegisteredModelSchema.parse(raw);
}

export async function engineDeleteModel(id: string): Promise<void> {
  await invoke("engine_delete_model", { id });
}

export async function engineEvaluateModel(params: ModelEvalParams): Promise<ModelEvalRunResult> {
  const raw = await invoke("engine_evaluate_model", { params });
  return ModelEvalRunResultSchema.parse(raw);
}

export async function engineEnableLiveTradingUnlock(args: {
  confirmation_phrase: string;
  risk_non_default: boolean;
  kill_switch_configured: boolean;
}): Promise<void> {
  await invoke("enable_live_trading_unlock", {
    confirmation_phrase: args.confirmation_phrase,
    risk_non_default: args.risk_non_default,
    kill_switch_configured: args.kill_switch_configured,
  });
}

export async function listenEngineEvents(cb: (evt: EngineEvent) => void): Promise<UnlistenFn> {
  return listen("engine_event", (e) => cb(e.payload));
}
