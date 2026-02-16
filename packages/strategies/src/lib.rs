use anyhow::Context;
use chrono::Utc;
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::collections::BTreeMap;
use trader_shared::{
    Candle, OrderRequest, OrderSide, OrderType, Quote, Signal, Strategy, StrategyContext,
    StrategyMetadata,
};

pub fn built_in_strategies() -> Vec<StrategyMetadata> {
    vec![
        MaCrossoverStrategy::default().metadata(),
        MeanReversionStrategy::default().metadata(),
        ShortTermMomentumBot::default().metadata(),
        RhaiScriptStrategy::default().metadata(),
    ]
}

pub fn create_strategy(id: &str) -> anyhow::Result<Box<dyn Strategy>> {
    match id {
        "ma_crossover" => Ok(Box::new(MaCrossoverStrategy::default())),
        "mean_reversion" => Ok(Box::new(MeanReversionStrategy::default())),
        "short_term_momentum_bot" => Ok(Box::new(ShortTermMomentumBot::default())),
        "rhai_script" => Ok(Box::new(RhaiScriptStrategy::default())),
        _ => anyhow::bail!("unknown strategy id: {id}"),
    }
}

fn sma(values: &[f64], period: usize) -> Option<f64> {
    if period == 0 || values.len() < period {
        return None;
    }
    let slice = &values[values.len() - period..];
    Some(slice.iter().sum::<f64>() / period as f64)
}

#[derive(Debug, Clone)]
pub struct MaCrossoverStrategy {
    symbol: Option<String>,
    fast_period: usize,
    slow_period: usize,
    trade_qty: u32,

    closes: Vec<f64>,
    last_diff: Option<f64>,
    last_action: Option<OrderSide>,
}

impl Default for MaCrossoverStrategy {
    fn default() -> Self {
        Self {
            symbol: None,
            fast_period: 10,
            slow_period: 30,
            trade_qty: 10,
            closes: vec![],
            last_diff: None,
            last_action: None,
        }
    }
}

impl Strategy for MaCrossoverStrategy {
    fn metadata(&self) -> StrategyMetadata {
        StrategyMetadata {
            id: "ma_crossover".to_string(),
            name: "MA Crossover".to_string(),
            description:
                "Long-only: buys when fast SMA crosses above slow SMA; sells when it crosses below."
                    .to_string(),
            params_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "symbol": {"type": "string", "minLength": 1, "title": "Symbol"},
                    "fast_period": {"type": "integer", "minimum": 2, "maximum": 200, "default": 10},
                    "slow_period": {"type": "integer", "minimum": 3, "maximum": 400, "default": 30},
                    "trade_qty": {"type": "integer", "minimum": 1, "maximum": 1000, "default": 10}
                },
                "required": ["symbol", "fast_period", "slow_period", "trade_qty"],
                "additionalProperties": false
            }),
        }
    }

    fn set_params(&mut self, params: serde_json::Value) -> anyhow::Result<()> {
        #[derive(serde::Deserialize)]
        struct P {
            symbol: String,
            fast_period: usize,
            slow_period: usize,
            trade_qty: u32,
        }
        let p: P = serde_json::from_value(params).context("invalid params")?;
        if p.fast_period >= p.slow_period {
            anyhow::bail!("fast_period must be < slow_period");
        }
        self.symbol = Some(p.symbol);
        self.fast_period = p.fast_period;
        self.slow_period = p.slow_period;
        self.trade_qty = p.trade_qty;
        Ok(())
    }

    fn on_bar(&mut self, ctx: &StrategyContext, bar: &Candle) -> Vec<Signal> {
        if let Some(sym) = &self.symbol {
            if &bar.symbol != sym {
                return vec![];
            }
        }

        self.closes.push(bar.close);
        let fast = match sma(&self.closes, self.fast_period) {
            Some(v) => v,
            None => return vec![],
        };
        let slow = match sma(&self.closes, self.slow_period) {
            Some(v) => v,
            None => return vec![],
        };
        let diff = fast - slow;
        let prev = self.last_diff.replace(diff);

        let Some(prev) = prev else { return vec![] };

        // Cross up: buy. Cross down: sell.
        if prev <= 0.0 && diff > 0.0 {
            if self.last_action == Some(OrderSide::Buy) {
                return vec![];
            }
            self.last_action = Some(OrderSide::Buy);
            return vec![Signal {
                order: OrderRequest {
                    symbol: bar.symbol.clone(),
                    side: OrderSide::Buy,
                    qty: self.trade_qty,
                    order_type: OrderType::Market,
                    limit_price: None,
                    client_order_id: format!(
                        "strategy:{}:{}",
                        ctx.trace_id,
                        Utc::now().timestamp_millis()
                    ),
                },
                reason: format!("MA cross up (fast={fast:.4}, slow={slow:.4})"),
            }];
        }

        if prev >= 0.0 && diff < 0.0 {
            if self.last_action == Some(OrderSide::Sell) {
                return vec![];
            }
            self.last_action = Some(OrderSide::Sell);
            return vec![Signal {
                order: OrderRequest {
                    symbol: bar.symbol.clone(),
                    side: OrderSide::Sell,
                    qty: self.trade_qty,
                    order_type: OrderType::Market,
                    limit_price: None,
                    client_order_id: format!(
                        "strategy:{}:{}",
                        ctx.trace_id,
                        Utc::now().timestamp_millis()
                    ),
                },
                reason: format!("MA cross down (fast={fast:.4}, slow={slow:.4})"),
            }];
        }

        vec![]
    }
}

#[derive(Debug, Clone)]
pub struct MeanReversionStrategy {
    symbol: Option<String>,
    lookback: usize,
    entry_z: f64,
    exit_z: f64,
    trade_qty: u32,
    closes: Vec<f64>,
}

impl Default for MeanReversionStrategy {
    fn default() -> Self {
        Self {
            symbol: None,
            lookback: 50,
            entry_z: 2.0,
            exit_z: 0.5,
            trade_qty: 10,
            closes: vec![],
        }
    }
}

impl Strategy for MeanReversionStrategy {
    fn metadata(&self) -> StrategyMetadata {
        StrategyMetadata {
            id: "mean_reversion".to_string(),
            name: "Mean Reversion".to_string(),
            description: "Long-only: buys when price is far below mean (z <= -entry); exits when it mean-reverts (z >= -exit).".to_string(),
            params_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "symbol": {"type": "string", "minLength": 1},
                    "lookback": {"type": "integer", "minimum": 10, "maximum": 500, "default": 50},
                    "entry_z": {"type": "number", "minimum": 0.5, "maximum": 10, "default": 2.0},
                    "exit_z": {"type": "number", "minimum": 0.0, "maximum": 10, "default": 0.5},
                    "trade_qty": {"type": "integer", "minimum": 1, "maximum": 1000, "default": 10}
                },
                "required": ["symbol", "lookback", "entry_z", "exit_z", "trade_qty"],
                "additionalProperties": false
            }),
        }
    }

    fn set_params(&mut self, params: serde_json::Value) -> anyhow::Result<()> {
        #[derive(serde::Deserialize)]
        struct P {
            symbol: String,
            lookback: usize,
            entry_z: f64,
            exit_z: f64,
            trade_qty: u32,
        }
        let p: P = serde_json::from_value(params).context("invalid params")?;
        if p.lookback < 10 {
            anyhow::bail!("lookback too small");
        }
        if p.exit_z > p.entry_z {
            anyhow::bail!("exit_z should be <= entry_z");
        }
        self.symbol = Some(p.symbol);
        self.lookback = p.lookback;
        self.entry_z = p.entry_z;
        self.exit_z = p.exit_z;
        self.trade_qty = p.trade_qty;
        Ok(())
    }

    fn on_bar(&mut self, ctx: &StrategyContext, bar: &Candle) -> Vec<Signal> {
        if let Some(sym) = &self.symbol {
            if &bar.symbol != sym {
                return vec![];
            }
        }

        self.closes.push(bar.close);
        if self.closes.len() < self.lookback {
            return vec![];
        }
        let slice = &self.closes[self.closes.len() - self.lookback..];
        let mean: f64 = slice.iter().sum::<f64>() / self.lookback as f64;
        let var: f64 = slice
            .iter()
            .map(|v| {
                let d = v - mean;
                d * d
            })
            .sum::<f64>()
            / (self.lookback as f64);
        let std = var.sqrt();
        let z = if std == 0.0 {
            0.0
        } else {
            (bar.close - mean) / std
        };

        let pos_qty = ctx.position.as_ref().map(|p| p.qty).unwrap_or(0);
        if pos_qty <= 0 && z <= -self.entry_z {
            return vec![Signal {
                order: OrderRequest {
                    symbol: bar.symbol.clone(),
                    side: OrderSide::Buy,
                    qty: self.trade_qty,
                    order_type: OrderType::Market,
                    limit_price: None,
                    client_order_id: format!(
                        "strategy:{}:{}",
                        ctx.trace_id,
                        Utc::now().timestamp_millis()
                    ),
                },
                reason: format!("Mean reversion entry (z={z:.3})"),
            }];
        }

        if pos_qty > 0 && z >= -self.exit_z {
            // Exit to flat.
            let qty = pos_qty.unsigned_abs() as u32;
            if qty == 0 {
                return vec![];
            }
            return vec![Signal {
                order: OrderRequest {
                    symbol: bar.symbol.clone(),
                    side: OrderSide::Sell,
                    qty,
                    order_type: OrderType::Market,
                    limit_price: None,
                    client_order_id: format!(
                        "strategy:{}:{}",
                        ctx.trace_id,
                        Utc::now().timestamp_millis()
                    ),
                },
                reason: format!("Mean reversion exit (z={z:.3})"),
            }];
        }

        vec![]
    }
}

#[derive(Debug, Clone)]
pub struct ShortTermMomentumBot {
    symbol: Option<String>,
    lookback: usize,
    entry_momentum_bps: f64,
    min_volatility_bps: f64,
    take_profit_bps: f64,
    stop_loss_bps: f64,
    horizon_sec: u32,
    trade_qty: u32,
    cooldown_bars: u32,
    ai_confirm: bool,
    min_ai_confidence: f64,

    closes: Vec<f64>,
    bars_in_position: u32,
    cooldown_left: u32,
    entry_price: Option<f64>,
}

impl Default for ShortTermMomentumBot {
    fn default() -> Self {
        Self {
            symbol: None,
            lookback: 12,
            entry_momentum_bps: 28.0,
            min_volatility_bps: 8.0,
            take_profit_bps: 35.0,
            stop_loss_bps: 18.0,
            horizon_sec: 180,
            trade_qty: 5,
            cooldown_bars: 4,
            ai_confirm: false,
            min_ai_confidence: 0.65,
            closes: Vec::new(),
            bars_in_position: 0,
            cooldown_left: 0,
            entry_price: None,
        }
    }
}

impl Strategy for ShortTermMomentumBot {
    fn metadata(&self) -> StrategyMetadata {
        StrategyMetadata {
            id: "short_term_momentum_bot".to_string(),
            name: "Short-Term Momentum Bot".to_string(),
            description: "Short-term momentum strategy with stop/take/horizon guards and optional AI confirmation gate.".to_string(),
            params_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "symbol": {"type": "string", "minLength": 1},
                    "lookback": {"type": "integer", "minimum": 5, "maximum": 120, "default": 12},
                    "entry_momentum_bps": {"type": "number", "minimum": 1, "maximum": 500, "default": 28},
                    "min_volatility_bps": {"type": "number", "minimum": 0, "maximum": 500, "default": 8},
                    "take_profit_bps": {"type": "number", "minimum": 1, "maximum": 1000, "default": 35},
                    "stop_loss_bps": {"type": "number", "minimum": 1, "maximum": 1000, "default": 18},
                    "horizon_sec": {"type": "integer", "minimum": 15, "maximum": 7200, "default": 180},
                    "trade_qty": {"type": "integer", "minimum": 1, "maximum": 1000, "default": 5},
                    "cooldown_bars": {"type": "integer", "minimum": 0, "maximum": 500, "default": 4},
                    "ai_confirm": {"type": "boolean", "default": false},
                    "min_ai_confidence": {"type": "number", "minimum": 0, "maximum": 1, "default": 0.65}
                },
                "required": [
                    "symbol",
                    "lookback",
                    "entry_momentum_bps",
                    "min_volatility_bps",
                    "take_profit_bps",
                    "stop_loss_bps",
                    "horizon_sec",
                    "trade_qty",
                    "cooldown_bars",
                    "ai_confirm",
                    "min_ai_confidence"
                ],
                "additionalProperties": false
            }),
        }
    }

    fn set_params(&mut self, params: serde_json::Value) -> anyhow::Result<()> {
        #[derive(serde::Deserialize)]
        struct P {
            symbol: String,
            lookback: usize,
            entry_momentum_bps: f64,
            min_volatility_bps: f64,
            take_profit_bps: f64,
            stop_loss_bps: f64,
            horizon_sec: u32,
            trade_qty: u32,
            cooldown_bars: u32,
            ai_confirm: bool,
            min_ai_confidence: f64,
        }
        let p: P = serde_json::from_value(params).context("invalid params")?;
        if p.lookback < 5 || p.lookback > 120 {
            anyhow::bail!("lookback out of range");
        }
        if p.entry_momentum_bps <= 0.0 || p.entry_momentum_bps > 500.0 {
            anyhow::bail!("entry_momentum_bps out of range");
        }
        if p.min_volatility_bps < 0.0 || p.min_volatility_bps > 500.0 {
            anyhow::bail!("min_volatility_bps out of range");
        }
        if p.take_profit_bps <= 0.0 || p.stop_loss_bps <= 0.0 {
            anyhow::bail!("take_profit_bps and stop_loss_bps must be > 0");
        }
        if p.horizon_sec < 15 || p.horizon_sec > 7200 {
            anyhow::bail!("horizon_sec out of range");
        }
        if !(0.0..=1.0).contains(&p.min_ai_confidence) {
            anyhow::bail!("min_ai_confidence must be in [0,1]");
        }

        self.symbol = Some(p.symbol);
        self.lookback = p.lookback;
        self.entry_momentum_bps = p.entry_momentum_bps;
        self.min_volatility_bps = p.min_volatility_bps;
        self.take_profit_bps = p.take_profit_bps;
        self.stop_loss_bps = p.stop_loss_bps;
        self.horizon_sec = p.horizon_sec;
        self.trade_qty = p.trade_qty;
        self.cooldown_bars = p.cooldown_bars;
        self.ai_confirm = p.ai_confirm;
        self.min_ai_confidence = p.min_ai_confidence;
        Ok(())
    }

    fn on_bar(&mut self, ctx: &StrategyContext, bar: &Candle) -> Vec<Signal> {
        if let Some(sym) = &self.symbol {
            if &bar.symbol != sym {
                return vec![];
            }
        }
        self.closes.push(bar.close);
        if self.closes.len() > 2048 {
            let drop_n = self.closes.len().saturating_sub(2048);
            if drop_n > 0 {
                self.closes.drain(0..drop_n);
            }
        }
        if self.cooldown_left > 0 {
            self.cooldown_left -= 1;
        }

        let pos_qty = ctx.position.as_ref().map(|p| p.qty).unwrap_or(0);

        if pos_qty > 0 {
            self.bars_in_position = self.bars_in_position.saturating_add(1);
            let entry = ctx
                .position
                .as_ref()
                .map(|p| p.avg_cost)
                .filter(|v| *v > 0.0)
                .or(self.entry_price)
                .unwrap_or(bar.close);
            let pnl_bps = if entry > 0.0 {
                (bar.close / entry - 1.0) * 10_000.0
            } else {
                0.0
            };
            let elapsed_sec = self.bars_in_position.saturating_mul(bar.interval_sec);
            let should_exit = pnl_bps >= self.take_profit_bps
                || pnl_bps <= -self.stop_loss_bps
                || elapsed_sec >= self.horizon_sec;
            if should_exit {
                let qty = pos_qty.max(0) as u32;
                if qty == 0 {
                    return vec![];
                }
                self.entry_price = None;
                self.bars_in_position = 0;
                self.cooldown_left = self.cooldown_bars;
                return vec![Signal {
                    order: OrderRequest {
                        symbol: bar.symbol.clone(),
                        side: OrderSide::Sell,
                        qty,
                        order_type: OrderType::Market,
                        limit_price: None,
                        client_order_id: format!(
                            "strategy:{}:{}",
                            ctx.trace_id,
                            Utc::now().timestamp_millis()
                        ),
                    },
                    reason: format!(
                        "Short-term exit (pnl_bps={pnl_bps:.1}, elapsed={elapsed_sec}s)"
                    ),
                }];
            }
            return vec![];
        } else {
            self.bars_in_position = 0;
            self.entry_price = None;
        }

        if self.cooldown_left > 0 {
            return vec![];
        }
        if self.closes.len() <= self.lookback {
            return vec![];
        }

        let base_idx = self.closes.len() - 1 - self.lookback;
        let base = self.closes[base_idx];
        if base <= 0.0 {
            return vec![];
        }
        let momentum_bps = (bar.close / base - 1.0) * 10_000.0;
        let vol_bps = rolling_vol_bps(&self.closes, self.lookback).unwrap_or(0.0);
        if momentum_bps >= self.entry_momentum_bps && vol_bps >= self.min_volatility_bps {
            self.entry_price = Some(bar.close);
            self.bars_in_position = 0;
            let mut reason =
                format!("Short-term entry (mom_bps={momentum_bps:.1}, vol_bps={vol_bps:.1})");
            if self.ai_confirm {
                reason.push_str(&format!(
                    ", ai_confirm=true,min_conf={:.2}",
                    self.min_ai_confidence
                ));
            }
            return vec![Signal {
                order: OrderRequest {
                    symbol: bar.symbol.clone(),
                    side: OrderSide::Buy,
                    qty: self.trade_qty,
                    order_type: OrderType::Market,
                    limit_price: None,
                    client_order_id: format!(
                        "strategy:{}:{}",
                        ctx.trace_id,
                        Utc::now().timestamp_millis()
                    ),
                },
                reason,
            }];
        }
        vec![]
    }
}

fn rolling_vol_bps(closes: &[f64], lookback: usize) -> Option<f64> {
    if closes.len() < lookback + 1 || lookback < 2 {
        return None;
    }
    let slice = &closes[closes.len() - (lookback + 1)..];
    let mut rets = Vec::with_capacity(lookback);
    for i in 1..slice.len() {
        let p0 = slice[i - 1];
        let p1 = slice[i];
        if p0 <= 0.0 {
            return None;
        }
        rets.push((p1 / p0 - 1.0) * 10_000.0);
    }
    let mean = rets.iter().sum::<f64>() / (rets.len() as f64);
    let var = rets
        .iter()
        .map(|r| {
            let d = *r - mean;
            d * d
        })
        .sum::<f64>()
        / (rets.len() as f64);
    Some(var.sqrt())
}

#[derive(Debug)]
pub struct RhaiScriptStrategy {
    symbol: Option<String>,
    trade_qty: u32,
    /// Rhai source code.
    script: String,

    // Runtime
    engine: rhai::Engine,
    ast: Option<rhai::AST>,
    closes: Vec<f64>,

    // Determinism helper: per-strategy seeded RNG.
    rng: StdRng,
}

impl Default for RhaiScriptStrategy {
    fn default() -> Self {
        let mut engine = rhai::Engine::new_raw();

        // Hard limits for safer execution.
        engine.set_max_operations(50_000);
        engine.set_max_call_levels(32);
        engine.set_max_expr_depths(128, 64);
        engine.set_max_string_size(16 * 1024);

        // Provide a minimal API: `sma(period)` and `rand01()` for research.
        engine.register_fn("sma", |closes: rhai::Array, period: i64| -> rhai::Dynamic {
            let period: usize = if period <= 0 { 0 } else { period as usize };
            let mut v = Vec::with_capacity(closes.len());
            for d in closes {
                if let Some(f) = d.clone().try_cast::<f64>() {
                    v.push(f);
                }
            }
            match sma(&v, period) {
                Some(x) => rhai::Dynamic::from_float(x),
                None => rhai::Dynamic::UNIT,
            }
        });

        Self {
            symbol: None,
            trade_qty: 1,
            script: default_rhai_script(),
            engine,
            ast: None,
            closes: vec![],
            rng: StdRng::seed_from_u64(42),
        }
    }
}

fn default_rhai_script() -> String {
    // This is intentionally minimal: no IO, no network.
    r#"
// Returns: "buy", "sell", or "hold".
fn on_bar(close) {
  // Example: buy when close dips below SMA(20), sell when above.
  let action = "hold";
  action
}
"#
    .to_string()
}

impl Strategy for RhaiScriptStrategy {
    fn metadata(&self) -> StrategyMetadata {
        StrategyMetadata {
            id: "rhai_script".to_string(),
            name: "Custom Script (Rhai)".to_string(),
            description: "Sandboxed custom strategy script. Paper/research only in MVP."
                .to_string(),
            params_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "symbol": {"type": "string", "minLength": 1},
                    "trade_qty": {"type": "integer", "minimum": 1, "maximum": 1000, "default": 1},
                    "script": {"type": "string", "minLength": 1, "title": "Rhai script"}
                },
                "required": ["symbol", "trade_qty", "script"],
                "additionalProperties": false
            }),
        }
    }

    fn set_params(&mut self, params: serde_json::Value) -> anyhow::Result<()> {
        #[derive(serde::Deserialize)]
        struct P {
            symbol: String,
            trade_qty: u32,
            script: String,
        }
        let p: P = serde_json::from_value(params).context("invalid params")?;
        self.symbol = Some(p.symbol);
        self.trade_qty = p.trade_qty;
        self.script = p.script;

        // Compile eagerly so we can reject invalid scripts before running.
        let ast = self
            .engine
            .compile(self.script.clone())
            .context("failed to compile Rhai script")?;
        self.ast = Some(ast);
        Ok(())
    }

    fn on_bar(&mut self, ctx: &StrategyContext, bar: &Candle) -> Vec<Signal> {
        let Some(sym) = &self.symbol else {
            return vec![];
        };
        if &bar.symbol != sym {
            return vec![];
        }
        let Some(ast) = self.ast.as_ref() else {
            return vec![];
        };

        self.closes.push(bar.close);

        // Execute `on_bar(close)`.
        let mut scope = rhai::Scope::new();
        scope.push("close", bar.close);

        let result: Result<rhai::Dynamic, _> =
            self.engine.call_fn(&mut scope, ast, "on_bar", (bar.close,));
        let action = match result {
            Ok(d) => d.try_cast::<String>().unwrap_or_else(|| "hold".to_string()),
            Err(_) => return vec![],
        };

        let action = action.to_lowercase();
        if action == "buy" {
            return vec![Signal {
                order: OrderRequest {
                    symbol: bar.symbol.clone(),
                    side: OrderSide::Buy,
                    qty: self.trade_qty,
                    order_type: OrderType::Market,
                    limit_price: None,
                    client_order_id: format!(
                        "strategy:{}:{}",
                        ctx.trace_id,
                        Utc::now().timestamp_millis()
                    ),
                },
                reason: "Rhai script buy".to_string(),
            }];
        }
        if action == "sell" {
            return vec![Signal {
                order: OrderRequest {
                    symbol: bar.symbol.clone(),
                    side: OrderSide::Sell,
                    qty: self.trade_qty,
                    order_type: OrderType::Market,
                    limit_price: None,
                    client_order_id: format!(
                        "strategy:{}:{}",
                        ctx.trace_id,
                        Utc::now().timestamp_millis()
                    ),
                },
                reason: "Rhai script sell".to_string(),
            }];
        }

        // Example of deterministic randomness usage (unused by default):
        let _ = self.rng.gen::<f64>();

        vec![]
    }
}

// Helper: used by the UI for quick validation previews.
pub fn validate_strategy_params(id: &str, params: serde_json::Value) -> anyhow::Result<()> {
    let mut s = create_strategy(id)?;
    s.set_params(params)
}

pub fn strategy_catalog_json() -> serde_json::Value {
    let strategies = built_in_strategies();
    let mut map = BTreeMap::new();
    for s in strategies {
        map.insert(s.id.clone(), s);
    }
    serde_json::to_value(map).expect("serializable")
}

// Keep this in case we want to expand the script API.
#[allow(dead_code)]
fn quote_to_map(q: &Quote) -> BTreeMap<String, serde_json::Value> {
    let mut m = BTreeMap::new();
    m.insert("symbol".to_string(), serde_json::json!(q.symbol));
    m.insert("last".to_string(), serde_json::json!(q.last));
    m.insert("bid".to_string(), serde_json::json!(q.bid));
    m.insert("ask".to_string(), serde_json::json!(q.ask));
    m.insert("ts".to_string(), serde_json::json!(q.ts.to_rfc3339()));
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use trader_shared::{Position, Strategy};

    fn candle(symbol: &str, ts: chrono::DateTime<Utc>, close: f64) -> Candle {
        Candle {
            symbol: symbol.to_string(),
            ts,
            interval_sec: 60,
            open: close,
            high: close,
            low: close,
            close,
            volume: 1000.0,
        }
    }

    #[test]
    fn short_term_momentum_bot_enters_and_exits() {
        let mut s = ShortTermMomentumBot::default();
        s.set_params(serde_json::json!({
            "symbol": "US.AAPL",
            "lookback": 5,
            "entry_momentum_bps": 20.0,
            "min_volatility_bps": 0.0,
            "take_profit_bps": 10.0,
            "stop_loss_bps": 50.0,
            "horizon_sec": 300,
            "trade_qty": 2,
            "cooldown_bars": 0,
            "ai_confirm": false,
            "min_ai_confidence": 0.65
        }))
        .unwrap();

        let t0 = Utc::now();
        let mut pos: Option<Position> = None;
        let mut saw_buy = false;

        let prices = [100.0, 100.3, 100.6, 101.0, 101.3, 101.6, 102.0];
        for (i, p) in prices.iter().enumerate() {
            let ctx = StrategyContext {
                trace_id: trader_shared::TraceId::new_v4(),
                now: t0 + chrono::Duration::seconds((i as i64) * 60),
                last_quote: None,
                position: pos.clone(),
            };
            let sigs = s.on_bar(&ctx, &candle("US.AAPL", ctx.now, *p));
            if let Some(sig) = sigs.first() {
                if sig.order.side == OrderSide::Buy {
                    saw_buy = true;
                    pos = Some(Position {
                        symbol: "US.AAPL".to_string(),
                        qty: sig.order.qty as i64,
                        avg_cost: *p,
                        realized_pnl: 0.0,
                        updated_at: ctx.now,
                    });
                    break;
                }
            }
        }
        assert!(saw_buy, "expected buy signal");

        // Pump price enough to trigger take-profit sell.
        let ctx = StrategyContext {
            trace_id: trader_shared::TraceId::new_v4(),
            now: t0 + chrono::Duration::seconds(600),
            last_quote: None,
            position: pos.clone(),
        };
        let sigs = s.on_bar(&ctx, &candle("US.AAPL", ctx.now, 103.0));
        assert!(!sigs.is_empty(), "expected exit signal");
        assert_eq!(sigs[0].order.side, OrderSide::Sell);
    }
}
