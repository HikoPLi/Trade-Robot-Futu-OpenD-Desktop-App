use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type TraceId = Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileMode {
    Paper,
    Live,
    Research,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenDTradeEnv {
    /// OpenD simulated trading environment (Futu "simulate").
    Simulate,
    /// OpenD real trading environment.
    Real,
}

impl Default for OpenDTradeEnv {
    fn default() -> Self {
        // Safer default: simulated.
        Self::Simulate
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionWindowUtc {
    /// 1=Mon ... 7=Sun. Empty means every day.
    pub weekdays: Vec<u8>,
    /// "HH:MM" in UTC.
    pub start_hhmm: String,
    /// "HH:MM" in UTC.
    pub end_hhmm: String,
}

impl Default for SessionWindowUtc {
    fn default() -> Self {
        Self {
            weekdays: vec![],
            start_hhmm: "00:00".to_string(),
            end_hhmm: "23:59".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TimeControls {
    /// If enabled, orders/signals are only allowed inside `sessions_utc` and not inside
    /// `blackout_utc`.
    pub enabled: bool,
    pub sessions_utc: Vec<SessionWindowUtc>,
    pub blackout_utc: Vec<SessionWindowUtc>,
    /// Minimum seconds between orders per symbol (engine-level cooldown).
    pub cooldown_sec: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OpenDConfig {
    pub host: String,
    pub port: u16,
    pub use_tls: bool,
}

impl Default for OpenDConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 11111,
            use_tls: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RiskLimits {
    /// Max absolute position per symbol (shares).
    pub max_position_qty: u32,

    /// Max order size (shares) for a single order.
    pub max_order_qty: u32,

    /// Max orders/minute across the engine.
    pub max_orders_per_minute: u32,

    /// If (realized + unrealized) PnL goes below -max_daily_loss_usd, the engine halts.
    pub max_daily_loss_usd: f64,

    /// If non-empty, only these symbols are tradable.
    pub symbol_allowlist: Vec<String>,

    /// If non-empty, only these markets are tradable (symbol prefixes), e.g. ["US", "HK"].
    pub allowed_markets: Vec<String>,

    /// Reject limit orders whose price is outside `last_price +/- last_price*price_band_pct`.
    pub price_band_pct: f64,
}

impl Default for RiskLimits {
    fn default() -> Self {
        Self {
            max_position_qty: 100,
            max_order_qty: 25,
            max_orders_per_minute: 10,
            max_daily_loss_usd: 50.0,
            symbol_allowlist: vec![],
            allowed_markets: vec![],
            price_band_pct: 0.10,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProfileConfig {
    pub mode: ProfileMode,
    pub opend: OpenDConfig,
    pub opend_trd_env: OpenDTradeEnv,
    pub risk: RiskLimits,
    pub time_controls: TimeControls,

    /// Live trading is locked behind an explicit workflow.
    pub live_trading_unlocked: bool,

    /// Global kill-switch hotkey, e.g. "CmdOrCtrl+Alt+K".
    pub kill_switch_hotkey: String,
}

impl Default for ProfileConfig {
    fn default() -> Self {
        Self {
            mode: ProfileMode::Paper,
            opend: OpenDConfig::default(),
            opend_trd_env: OpenDTradeEnv::default(),
            risk: RiskLimits::default(),
            time_controls: TimeControls::default(),
            live_trading_unlocked: false,
            kill_switch_hotkey: "CmdOrCtrl+Alt+K".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub active_profile: String,
    pub profiles: std::collections::BTreeMap<String, ProfileConfig>,
}

impl Default for AppConfig {
    fn default() -> Self {
        let mut profiles = std::collections::BTreeMap::new();
        profiles.insert("paper".to_string(), ProfileConfig::default());
        profiles.insert(
            "research".to_string(),
            ProfileConfig {
                mode: ProfileMode::Research,
                ..ProfileConfig::default()
            },
        );
        profiles.insert(
            "live".to_string(),
            ProfileConfig {
                mode: ProfileMode::Live,
                live_trading_unlocked: false,
                ..ProfileConfig::default()
            },
        );

        Self {
            active_profile: "paper".to_string(),
            profiles,
        }
    }
}

pub type Symbol = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quote {
    pub symbol: Symbol,
    pub ts: DateTime<Utc>,
    pub bid: f64,
    pub ask: f64,
    pub last: f64,
    pub volume: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candle {
    pub symbol: Symbol,
    pub ts: DateTime<Utc>,
    pub interval_sec: u32,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderType {
    Market,
    Limit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    PendingSubmit,
    Submitted,
    Filled,
    Cancelled,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderRequest {
    pub symbol: Symbol,
    pub side: OrderSide,
    pub qty: u32,
    pub order_type: OrderType,
    pub limit_price: Option<f64>,

    /// Client-provided idempotency key.
    pub client_order_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: String,
    pub symbol: Symbol,
    pub side: OrderSide,
    pub qty: u32,
    pub order_type: OrderType,
    pub limit_price: Option<f64>,

    pub status: OrderStatus,
    pub filled_qty: u32,
    pub avg_fill_price: Option<f64>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    pub client_order_id: String,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fill {
    pub order_id: String,
    pub symbol: Symbol,
    pub side: OrderSide,
    pub qty: u32,
    pub price: f64,
    pub fee: f64,
    pub ts: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub symbol: Symbol,
    pub qty: i64,
    pub avg_cost: f64,
    pub realized_pnl: f64,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineEvent {
    Quote(Quote),
    Candle(Candle),
    OrderUpdated(Order),
    Fill(Fill),
    PositionUpdated(Position),
    SignalFired {
        ts: DateTime<Utc>,
        trace_id: TraceId,
        strategy_id: String,
        symbol: Symbol,
        order: OrderRequest,
        reason: String,
    },
    RiskHalt { reason: String },
    Info { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal {
    pub order: OrderRequest,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyMetadata {
    pub id: String,
    pub name: String,
    pub description: String,
    /// JSON Schema for UI parameter forms.
    pub params_schema: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StrategyLifecycle {
    Draft,
    Paper,
    Live,
}

impl Default for StrategyLifecycle {
    fn default() -> Self {
        Self::Draft
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyDefinition {
    pub id: String,
    pub name: String,
    pub strategy_id: String,
    pub params: serde_json::Value,
    pub lifecycle: StrategyLifecycle,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyUpsertRequest {
    pub id: Option<String>,
    pub name: String,
    pub strategy_id: String,
    pub params: serde_json::Value,
    pub lifecycle: StrategyLifecycle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    /// JSON Schema for UI parameter forms.
    pub params_schema: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelKind {
    Builtin,
    Onnx,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredModel {
    pub id: String,
    pub base_id: String,
    pub kind: ModelKind,
    pub name: String,
    pub version: String,
    pub checksum: String,
    pub artifact_path: Option<String>,
    pub params: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRegisterRequest {
    pub id: Option<String>,
    pub base_id: String,
    pub kind: ModelKind,
    pub name: String,
    pub version: String,
    /// Source path on disk (UI-selected). The engine will copy it into the app data dir.
    pub artifact_source_path: Option<String>,
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEvalParams {
    pub model_id: String,
    pub candles_csv_path: String,
    pub symbol: String,
    pub seed: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Confusion2x2 {
    pub tp: u32,
    pub fp: u32,
    pub tn: u32,
    pub fn_: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEvalMetrics {
    pub samples: u32,
    /// Information coefficient: correlation(score, next_return).
    pub ic: f64,
    pub accuracy: f64,
    pub confusion: Confusion2x2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEvalReport {
    pub params: ModelEvalParams,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub metrics: ModelEvalMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEvalRunResult {
    pub report: ModelEvalReport,
    pub report_json_path: String,
    pub report_html_path: String,
}

#[derive(Debug, Clone)]
pub struct StrategyContext {
    pub trace_id: TraceId,
    pub now: DateTime<Utc>,
    pub last_quote: Option<Quote>,
    pub position: Option<Position>,
}

pub trait Strategy: Send {
    fn metadata(&self) -> StrategyMetadata;
    fn set_params(&mut self, params: serde_json::Value) -> anyhow::Result<()>;

    /// Called on each bar. Return 0..N signals.
    fn on_bar(&mut self, ctx: &StrategyContext, bar: &Candle) -> Vec<Signal>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestParams {
    pub symbol: Symbol,
    pub strategy_id: String,
    pub strategy_params: serde_json::Value,
    pub starting_cash: f64,
    pub fee_per_trade: f64,
    pub slippage_bps: f64,

    /// Path to a CSV file of candles.
    pub candles_csv_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestTrade {
    pub ts: DateTime<Utc>,
    pub symbol: Symbol,
    pub side: OrderSide,
    pub qty: u32,
    pub price: f64,
    pub fee: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestReport {
    pub params: BacktestParams,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,

    pub ending_cash: f64,
    pub ending_position_qty: i64,
    pub ending_equity: f64,

    pub total_return_pct: f64,
    pub max_drawdown_pct: f64,
    /// Annualized Sharpe ratio assuming zero risk-free rate.
    pub sharpe_ratio: f64,
    /// Turnover ratio = total traded notional / average equity.
    pub turnover: f64,
    pub hit_rate: f64,
    pub trade_count: u32,

    pub equity_curve: Vec<(DateTime<Utc>, f64)>,
    pub trades: Vec<BacktestTrade>,
}
