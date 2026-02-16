import { z } from "zod";

export const ProfileMode = z.enum(["paper", "live", "research"]);
export type ProfileMode = z.infer<typeof ProfileMode>;

export const OpenDTradeEnvSchema = z.enum(["simulate", "real"]);
export type OpenDTradeEnv = z.infer<typeof OpenDTradeEnvSchema>;

export const SessionWindowUtcSchema = z.object({
  weekdays: z.array(z.number().int()),
  start_hhmm: z.string(),
  end_hhmm: z.string(),
});
export type SessionWindowUtc = z.infer<typeof SessionWindowUtcSchema>;

export const TimeControlsSchema = z.object({
  enabled: z.boolean(),
  sessions_utc: z.array(SessionWindowUtcSchema),
  blackout_utc: z.array(SessionWindowUtcSchema),
  cooldown_sec: z.number().int().min(0),
});
export type TimeControls = z.infer<typeof TimeControlsSchema>;

export const OpenDConfigSchema = z.object({
  host: z.string(),
  port: z.number().int().min(1).max(65535),
  use_tls: z.boolean(),
});
export type OpenDConfig = z.infer<typeof OpenDConfigSchema>;

export const AiProviderKindSchema = z.enum([
  "openai",
  "deepseek",
  "qwen",
  "grok",
  "ollama",
  "openai_compatible",
]);
export type AiProviderKind = z.infer<typeof AiProviderKindSchema>;

export const AiProviderConfigSchema = z.object({
  id: z.string(),
  kind: AiProviderKindSchema,
  enabled: z.boolean(),
  base_url: z.string(),
  model: z.string(),
  api_key_secret: z.string(),
  timeout_ms: z.number().int().min(500),
  max_tokens: z.number().int().min(1),
  temperature: z.number().min(0).max(2),
});
export type AiProviderConfig = z.infer<typeof AiProviderConfigSchema>;

export const AiRouterConfigSchema = z.object({
  primary: z.string(),
  fallbacks: z.array(z.string()),
});
export type AiRouterConfig = z.infer<typeof AiRouterConfigSchema>;

export const RiskLimitsSchema = z.object({
  max_position_qty: z.number().int().min(0),
  max_order_qty: z.number().int().min(0),
  max_orders_per_minute: z.number().int().min(1),
  max_daily_loss_usd: z.number().finite().min(0),
  symbol_allowlist: z.array(z.string()),
  allowed_markets: z.array(z.string()),
  price_band_pct: z.number().finite().min(0).max(1),
});
export type RiskLimits = z.infer<typeof RiskLimitsSchema>;

export const ProfileConfigSchema = z.object({
  mode: ProfileMode,
  opend: OpenDConfigSchema,
  opend_trd_env: OpenDTradeEnvSchema,
  risk: RiskLimitsSchema,
  time_controls: TimeControlsSchema,
  live_trading_unlocked: z.boolean(),
  kill_switch_hotkey: z.string(),
  ai_router: AiRouterConfigSchema,
  ai_providers: z.record(z.string(), AiProviderConfigSchema),
});
export type ProfileConfig = z.infer<typeof ProfileConfigSchema>;

export const AppConfigSchema = z.object({
  active_profile: z.string(),
  profiles: z.record(z.string(), ProfileConfigSchema),
});
export type AppConfig = z.infer<typeof AppConfigSchema>;

export const QuoteSchema = z.object({
  symbol: z.string(),
  ts: z.string(),
  bid: z.number(),
  ask: z.number(),
  last: z.number(),
  volume: z.number(),
});
export type Quote = z.infer<typeof QuoteSchema>;

export const CandleSchema = z.object({
  symbol: z.string(),
  ts: z.string(),
  interval_sec: z.number().int(),
  open: z.number(),
  high: z.number(),
  low: z.number(),
  close: z.number(),
  volume: z.number(),
});
export type Candle = z.infer<typeof CandleSchema>;

export const OrderSideSchema = z.enum(["buy", "sell"]);
export type OrderSide = z.infer<typeof OrderSideSchema>;

export const OrderTypeSchema = z.enum(["market", "limit"]);
export type OrderType = z.infer<typeof OrderTypeSchema>;

export const AiTradeActionSchema = z.enum(["buy", "sell", "hold"]);
export type AiTradeAction = z.infer<typeof AiTradeActionSchema>;

export const OrderStatusSchema = z.enum([
  "pending_submit",
  "submitted",
  "filled",
  "cancelled",
  "rejected",
]);
export type OrderStatus = z.infer<typeof OrderStatusSchema>;

export const OrderSchema = z.object({
  id: z.string(),
  symbol: z.string(),
  side: OrderSideSchema,
  qty: z.number().int(),
  order_type: OrderTypeSchema,
  limit_price: z.number().nullable().optional(),
  status: OrderStatusSchema,
  filled_qty: z.number().int(),
  avg_fill_price: z.number().nullable().optional(),
  created_at: z.string(),
  updated_at: z.string(),
  client_order_id: z.string(),
  last_error: z.string().nullable().optional(),
});
export type Order = z.infer<typeof OrderSchema>;

export const OrderRequestSchema = z.object({
  symbol: z.string(),
  side: OrderSideSchema,
  qty: z.number().int().min(1),
  order_type: OrderTypeSchema,
  limit_price: z.number().nullable().optional(),
  client_order_id: z.string(),
});
export type OrderRequest = z.infer<typeof OrderRequestSchema>;

export const AiSignalRequestSchema = z.object({
  symbol: z.string(),
  strategy_id: z.string(),
  proposed_side: OrderSideSchema,
  reason: z.string(),
  last_price: z.number(),
  spread_bps: z.number(),
  horizon_sec: z.number().int().min(1),
});
export type AiSignalRequest = z.infer<typeof AiSignalRequestSchema>;

export const AiSignalResponseSchema = z.object({
  provider_id: z.string(),
  model: z.string(),
  action: AiTradeActionSchema,
  confidence: z.number(),
  reason: z.string(),
  safeguards: z.array(z.string()),
  raw: z.unknown(),
});
export type AiSignalResponse = z.infer<typeof AiSignalResponseSchema>;

export const PositionSchema = z.object({
  symbol: z.string(),
  qty: z.number().int(),
  avg_cost: z.number(),
  realized_pnl: z.number(),
  updated_at: z.string(),
});
export type Position = z.infer<typeof PositionSchema>;

export const MetricsSnapshotSchema = z.object({
  quote_updates: z.number().int(),
  candles_built: z.number().int(),
  orders_placed: z.number().int(),
  orders_rejected: z.number().int(),
  fills: z.number().int(),
});
export type MetricsSnapshot = z.infer<typeof MetricsSnapshotSchema>;

export const EngineStatusSchema = z.union([
  z.object({ type: z.literal("running") }),
  z.object({
    type: z.literal("halted"),
    reason: z.string(),
    at: z.string(),
  }),
]);
export type EngineStatus = z.infer<typeof EngineStatusSchema>;

export const RunningStrategyInfoSchema = z.object({
  instance_id: z.string(),
  strategy_id: z.string(),
  symbol: z.string(),
  started_at: z.string(),
  params: z.unknown(),
});
export type RunningStrategyInfo = z.infer<typeof RunningStrategyInfoSchema>;

export const EngineSnapshotSchema = z.object({
  status: EngineStatusSchema,
  safe_mode: z.boolean(),
  kill_switch_engaged: z.boolean(),
  config: AppConfigSchema,
  active_profile: ProfileConfigSchema,
  watchlist: z.array(z.string()),
  quotes: z.array(QuoteSchema),
  cash: z.number(),
  equity: z.number(),
  realized_pnl: z.number(),
  unrealized_pnl: z.number(),
  orders: z.array(OrderSchema),
  positions: z.array(PositionSchema),
  strategies: z.array(RunningStrategyInfoSchema),
  metrics: MetricsSnapshotSchema,
});
export type EngineSnapshot = z.infer<typeof EngineSnapshotSchema>;

export const StartStrategyRequestSchema = z.object({
  strategy_id: z.string(),
  params: z.unknown(),
});
export type StartStrategyRequest = z.infer<typeof StartStrategyRequestSchema>;

export const EnableLiveTradingPhrase = "I UNDERSTAND LIVE TRADING RISK";

export const StrategyMetadataSchema = z.object({
  id: z.string(),
  name: z.string(),
  description: z.string(),
  params_schema: z.unknown(),
});
export type StrategyMetadata = z.infer<typeof StrategyMetadataSchema>;

export const StrategyCatalogSchema = z.record(z.string(), StrategyMetadataSchema);
export type StrategyCatalog = z.infer<typeof StrategyCatalogSchema>;

export const StrategyLifecycleSchema = z.enum(["draft", "paper", "live"]);
export type StrategyLifecycle = z.infer<typeof StrategyLifecycleSchema>;

export const StrategyDefinitionSchema = z.object({
  id: z.string(),
  name: z.string(),
  strategy_id: z.string(),
  params: z.unknown(),
  lifecycle: StrategyLifecycleSchema,
  created_at: z.string(),
  updated_at: z.string(),
});
export type StrategyDefinition = z.infer<typeof StrategyDefinitionSchema>;

export const StrategyUpsertRequestSchema = z.object({
  id: z.string().nullable().optional(),
  name: z.string(),
  strategy_id: z.string(),
  params: z.unknown(),
  lifecycle: StrategyLifecycleSchema,
});
export type StrategyUpsertRequest = z.infer<typeof StrategyUpsertRequestSchema>;

export const ModelMetadataSchema = z.object({
  id: z.string(),
  name: z.string(),
  description: z.string(),
  version: z.string(),
  params_schema: z.unknown(),
});
export type ModelMetadata = z.infer<typeof ModelMetadataSchema>;

export const ModelCatalogSchema = z.record(z.string(), ModelMetadataSchema);
export type ModelCatalog = z.infer<typeof ModelCatalogSchema>;

export const ModelKindSchema = z.enum(["builtin", "onnx"]);
export type ModelKind = z.infer<typeof ModelKindSchema>;

export const RegisteredModelSchema = z.object({
  id: z.string(),
  base_id: z.string(),
  kind: ModelKindSchema,
  name: z.string(),
  version: z.string(),
  checksum: z.string(),
  artifact_path: z.string().nullable().optional(),
  params: z.unknown(),
  created_at: z.string(),
  updated_at: z.string(),
});
export type RegisteredModel = z.infer<typeof RegisteredModelSchema>;

export const ModelRegisterRequestSchema = z.object({
  id: z.string().nullable().optional(),
  base_id: z.string(),
  kind: ModelKindSchema,
  name: z.string(),
  version: z.string(),
  artifact_source_path: z.string().nullable().optional(),
  params: z.unknown(),
});
export type ModelRegisterRequest = z.infer<typeof ModelRegisterRequestSchema>;

export const ModelEvalParamsSchema = z.object({
  model_id: z.string(),
  candles_csv_path: z.string(),
  symbol: z.string(),
  seed: z.number().int(),
});
export type ModelEvalParams = z.infer<typeof ModelEvalParamsSchema>;

export const Confusion2x2Schema = z.object({
  tp: z.number().int(),
  fp: z.number().int(),
  tn: z.number().int(),
  fn_: z.number().int(),
});
export type Confusion2x2 = z.infer<typeof Confusion2x2Schema>;

export const ModelEvalMetricsSchema = z.object({
  samples: z.number().int(),
  ic: z.number(),
  accuracy: z.number(),
  confusion: Confusion2x2Schema,
});
export type ModelEvalMetrics = z.infer<typeof ModelEvalMetricsSchema>;

export const ModelEvalReportSchema = z.object({
  params: ModelEvalParamsSchema,
  started_at: z.string(),
  finished_at: z.string(),
  metrics: ModelEvalMetricsSchema,
});
export type ModelEvalReport = z.infer<typeof ModelEvalReportSchema>;

export const ModelEvalRunResultSchema = z.object({
  report: ModelEvalReportSchema,
  report_json_path: z.string(),
  report_html_path: z.string(),
});
export type ModelEvalRunResult = z.infer<typeof ModelEvalRunResultSchema>;

export const BacktestParamsSchema = z.object({
  symbol: z.string(),
  strategy_id: z.string(),
  strategy_params: z.unknown(),
  starting_cash: z.number(),
  fee_per_trade: z.number(),
  slippage_bps: z.number(),
  candles_csv_path: z.string(),
});
export type BacktestParams = z.infer<typeof BacktestParamsSchema>;

export const BacktestTradeSchema = z.object({
  ts: z.string(),
  symbol: z.string(),
  side: OrderSideSchema,
  qty: z.number().int(),
  price: z.number(),
  fee: z.number(),
  reason: z.string(),
});
export type BacktestTrade = z.infer<typeof BacktestTradeSchema>;

export const BacktestReportSchema = z.object({
  params: BacktestParamsSchema,
  started_at: z.string(),
  finished_at: z.string(),
  ending_cash: z.number(),
  ending_position_qty: z.number().int(),
  ending_equity: z.number(),
  total_return_pct: z.number(),
  max_drawdown_pct: z.number(),
  sharpe_ratio: z.number(),
  turnover: z.number(),
  hit_rate: z.number(),
  trade_count: z.number().int(),
  equity_curve: z.array(z.tuple([z.string(), z.number()])),
  trades: z.array(BacktestTradeSchema),
});
export type BacktestReport = z.infer<typeof BacktestReportSchema>;

export const BacktestRunResultSchema = z.object({
  report: BacktestReportSchema,
  report_json_path: z.string(),
  report_html_path: z.string(),
});
export type BacktestRunResult = z.infer<typeof BacktestRunResultSchema>;

export const AuditEventRowSchema = z.object({
  id: z.number().int(),
  ts: z.string(),
  event_type: z.string(),
  payload_json: z.unknown(),
  trace_id: z.string().nullable().optional(),
  prev_hash: z.string().nullable().optional(),
  hash: z.string(),
});
export type AuditEventRow = z.infer<typeof AuditEventRowSchema>;
