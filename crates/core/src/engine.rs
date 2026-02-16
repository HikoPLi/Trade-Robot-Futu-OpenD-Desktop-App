use crate::{
    audit::AuditLog,
    backtest, config, logging,
    metrics::{Metrics, MetricsSnapshot},
    paper::{PaperExecution, PaperExecutionConfig},
    paths::{app_paths, AppPaths},
    risk::{OrderValidationCtx, RiskEngine, RiskReject},
    state_db::StateDb,
};
use anyhow::Context;
use chrono::{DateTime, NaiveDate, Utc};
use parking_lot::{Mutex, RwLock};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::sync::broadcast;
use tokio_stream::StreamExt;
use trader_futu_connector::{
    MarketSubscription, MockMarketData, MockMarketDataConfig, OpenDClient,
    OpenDConfig as ConnectorOpenDConfig,
};
use trader_shared::{
    AiProviderConfig, AiRouterConfig, AiSignalRequest, AiSignalResponse, AiTradeAction, AppConfig,
    BacktestParams, BacktestReport, EngineEvent, ModelEvalParams, ModelEvalReport,
    ModelEvalRunResult, ModelKind, ModelRegisterRequest, Order, OrderRequest, Position,
    ProfileConfig, Quote, RegisteredModel, RiskLimits, StrategyContext, StrategyDefinition,
    StrategyLifecycle, StrategyUpsertRequest,
};
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EngineStatus {
    Running,
    Halted { reason: String, at: DateTime<Utc> },
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RunningStrategyInfo {
    pub instance_id: String,
    pub strategy_id: String,
    pub symbol: String,
    pub started_at: DateTime<Utc>,
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EngineSnapshot {
    pub status: EngineStatus,
    pub safe_mode: bool,
    pub kill_switch_engaged: bool,

    pub config: AppConfig,
    pub active_profile: ProfileConfig,

    pub watchlist: Vec<String>,
    pub quotes: Vec<Quote>,

    pub cash: f64,
    pub equity: f64,
    pub realized_pnl: f64,
    pub unrealized_pnl: f64,

    pub orders: Vec<Order>,
    pub positions: Vec<Position>,
    pub strategies: Vec<RunningStrategyInfo>,

    pub metrics: MetricsSnapshot,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct StartStrategyRequest {
    pub strategy_id: String,
    pub params: serde_json::Value,
}

#[derive(Debug)]
struct PortfolioState {
    cash: f64,
    positions: BTreeMap<String, Position>,
    realized_pnl: f64,
    day: NaiveDate,
    day_start_equity: f64,
}

struct LiveState {
    client: Option<OpenDClient>,
    /// Map of `trd_market` -> header for routing.
    headers: BTreeMap<i32, trader_futu_connector::OpenDTradeHeader>,
    trade_unlocked: bool,

    idempotency: BTreeMap<String, String>, // client_order_id -> order_id
    orders: BTreeMap<String, Order>,
    positions: BTreeMap<String, Position>,

    cash: f64,
    equity: f64,
    day: NaiveDate,
    day_start_equity: f64,

    last_quote_ts: Option<DateTime<Utc>>,
    last_reconcile_at: Option<DateTime<Utc>>,
}

impl Default for LiveState {
    fn default() -> Self {
        Self {
            client: None,
            headers: BTreeMap::new(),
            trade_unlocked: false,
            idempotency: BTreeMap::new(),
            orders: BTreeMap::new(),
            positions: BTreeMap::new(),
            cash: 0.0,
            equity: 0.0,
            day: Utc::now().date_naive(),
            day_start_equity: 0.0,
            last_quote_ts: None,
            last_reconcile_at: None,
        }
    }
}

struct StrategyInstance {
    instance_id: String,
    strategy_id: String,
    symbol: String,
    params: serde_json::Value,
    started_at: DateTime<Utc>,
    strategy: Box<dyn trader_shared::Strategy>,
}

struct EngineInner {
    paths: AppPaths,

    cfg: RwLock<AppConfig>,
    active_profile: RwLock<ProfileConfig>,

    audit: AuditLog,
    state_db: StateDb,
    metrics: Metrics,

    status: RwLock<EngineStatus>,
    safe_mode: bool,
    kill_switch: AtomicBool,

    watchlist: RwLock<BTreeSet<String>>,
    quotes: RwLock<BTreeMap<String, Quote>>,

    paper: Mutex<PaperExecution>,
    portfolio: Mutex<PortfolioState>,
    live: Mutex<LiveState>,
    risk: Mutex<RiskEngine>,

    strategies: Mutex<Vec<StrategyInstance>>,

    event_tx: broadcast::Sender<EngineEvent>,

    mock_market: MockMarketData,
    model_api: crate::model_api::ModelApiRuntime,
}

#[derive(Clone)]
pub struct EngineHandle {
    inner: Arc<EngineInner>,
}

impl EngineHandle {
    pub async fn new() -> anyhow::Result<Self> {
        let paths = app_paths()?;
        logging::init_logging(&paths)?;

        let safe_mode = paths.crash_marker_path.exists();
        write_crash_marker(&paths).await.ok();

        let cfg = config::load_or_init(&paths).await?;
        let active_profile = cfg
            .profiles
            .get(&cfg.active_profile)
            .cloned()
            .unwrap_or_default();

        let audit = AuditLog::open(&paths).await?;
        let state_db = StateDb::open(&paths).await?;
        let (event_tx, _) = broadcast::channel(1024);
        let model_api = crate::model_api::ModelApiRuntime::new()?;

        let risk = RiskEngine::new(active_profile.risk.clone());

        let inner = Arc::new(EngineInner {
            paths,
            cfg: RwLock::new(cfg.clone()),
            active_profile: RwLock::new(active_profile.clone()),
            audit,
            state_db,
            metrics: Metrics::default(),
            status: RwLock::new(if safe_mode {
                EngineStatus::Halted {
                    reason: "Safe mode: previous crash detected".to_string(),
                    at: Utc::now(),
                }
            } else {
                EngineStatus::Running
            }),
            safe_mode,
            kill_switch: AtomicBool::new(false),
            watchlist: RwLock::new(BTreeSet::new()),
            quotes: RwLock::new(BTreeMap::new()),
            paper: Mutex::new(PaperExecution::new(PaperExecutionConfig::default())),
            portfolio: Mutex::new(PortfolioState {
                cash: 10_000.0,
                positions: BTreeMap::new(),
                realized_pnl: 0.0,
                day: Utc::now().date_naive(),
                day_start_equity: 10_000.0,
            }),
            live: Mutex::new(LiveState::default()),
            risk: Mutex::new(risk),
            strategies: Mutex::new(Vec::new()),
            event_tx,
            mock_market: MockMarketData::new(MockMarketDataConfig::default()),
            model_api,
        });

        let handle = Self { inner };
        handle
            .audit_info(
                "engine_start",
                serde_json::json!({"safe_mode": safe_mode}),
                None,
            )
            .await;

        handle.spawn_market_tasks();
        if !safe_mode {
            // Best-effort: connect live data source if the active profile is live.
            let _ = handle.apply_profile_side_effects().await;
        }
        Ok(handle)
    }

    fn spawn_market_tasks(&self) {
        let this = self.clone();
        tokio::spawn(async move {
            let mut stream = this.inner.mock_market.quote_stream().await;
            let mut bar_builder = BarBuilder::new(5);

            while let Some(q) = stream.next().await {
                this.inner.metrics.inc_quote();

                {
                    let mut m = this.inner.quotes.write();
                    m.insert(q.symbol.clone(), q.clone());
                }
                let _ = this.inner.event_tx.send(EngineEvent::Quote(q.clone()));

                // Limit-order fills.
                let fills = {
                    let mut paper = this.inner.paper.lock();
                    paper.on_quote(Utc::now(), &q)
                };
                for (order, fill) in fills {
                    this.inner.metrics.inc_fill();
                    this.apply_fill(&fill).await;
                    let _ = this
                        .inner
                        .event_tx
                        .send(EngineEvent::OrderUpdated(order.clone()));
                    let _ = this.inner.event_tx.send(EngineEvent::Fill(fill.clone()));
                    this.audit_info(
                        "order_filled",
                        serde_json::json!({"order": order, "fill": fill}),
                        None,
                    )
                    .await;
                }

                if let Some(candle) = bar_builder.on_quote(&q) {
                    this.inner.metrics.inc_candle();
                    let _ = this
                        .inner
                        .event_tx
                        .send(EngineEvent::Candle(candle.clone()));
                    this.handle_candle(candle).await;
                }
            }
        });

        // Live reconciliation / health loop (paper-safe: does nothing unless OpenD is connected).
        let this = self.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(std::time::Duration::from_secs(5));
            loop {
                ticker.tick().await;
                let mode = this.inner.active_profile.read().mode;
                if !matches!(mode, trader_shared::ProfileMode::Live) {
                    continue;
                }
                let kill = this.inner.kill_switch.load(Ordering::Relaxed);
                if kill {
                    continue;
                }
                let _ = this.live_reconcile().await;
            }
        });
    }

    async fn apply_profile_side_effects(&self) -> anyhow::Result<()> {
        let prof = self.inner.active_profile.read().clone();
        let symbols: Vec<String> = self.inner.watchlist.read().iter().cloned().collect();

        match prof.mode {
            trader_shared::ProfileMode::Live => {
                // Disable mock feed to avoid accidental paper fills while in live profile.
                self.inner
                    .mock_market
                    .set_subscription(MarketSubscription { symbols: vec![] })
                    .await;
                self.ensure_live_connected().await?;
                if !symbols.is_empty() {
                    self.live_subscribe_watchlist(symbols).await?;
                }
            }
            _ => {
                self.disconnect_live().await;
                self.inner
                    .mock_market
                    .set_subscription(MarketSubscription { symbols })
                    .await;
            }
        }
        Ok(())
    }

    async fn ensure_live_connected(&self) -> anyhow::Result<()> {
        let prof = self.inner.active_profile.read().clone();
        if !matches!(prof.mode, trader_shared::ProfileMode::Live) {
            return Ok(());
        }
        if self.inner.safe_mode {
            anyhow::bail!("safe mode is active; live connection is disabled");
        }

        let already = { self.inner.live.lock().client.is_some() };
        if already {
            return Ok(());
        }

        let cfg = ConnectorOpenDConfig {
            host: prof.opend.host.clone(),
            port: prof.opend.port,
            use_tls: prof.opend.use_tls,
        };

        let (client, _info) = OpenDClient::connect(cfg)
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        // Collect trade headers for routing.
        let accounts = client
            .trd_get_acc_list()
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        let desired_env = match prof.opend_trd_env {
            trader_shared::OpenDTradeEnv::Simulate => {
                trader_futu_connector::pb::trd_common::TrdEnv::Simulate as i32
            }
            trader_shared::OpenDTradeEnv::Real => {
                trader_futu_connector::pb::trd_common::TrdEnv::Real as i32
            }
        };

        let mut headers: BTreeMap<i32, trader_futu_connector::OpenDTradeHeader> = BTreeMap::new();
        let mut acc_ids: BTreeSet<u64> = BTreeSet::new();
        for acc in accounts {
            if acc.trd_env != desired_env {
                continue;
            }
            acc_ids.insert(acc.acc_id);
            for mkt in acc.trd_market_auth_list.iter().cloned() {
                headers
                    .entry(mkt)
                    .or_insert(trader_futu_connector::OpenDTradeHeader {
                        trd_env: acc.trd_env,
                        acc_id: acc.acc_id,
                        trd_market: mkt,
                    });
            }
        }

        if headers.is_empty() {
            anyhow::bail!("no trade accounts found for selected OpenD trading environment");
        }

        // Subscribe to account push (orders/fills).
        client
            .trd_sub_acc_push(acc_ids.into_iter().collect())
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        // Spawn live event loop.
        self.spawn_live_event_loop(client.clone());

        {
            let mut live = self.inner.live.lock();
            live.client = Some(client);
            live.headers = headers;
            live.trade_unlocked = false;
            live.last_reconcile_at = None;
        }

        self.audit_info(
            "opend_connected",
            serde_json::json!({"env": desired_env}),
            None,
        )
        .await;

        Ok(())
    }

    fn spawn_live_event_loop(&self, client: OpenDClient) {
        let this = self.clone();
        tokio::spawn(async move {
            let mut rx = client.subscribe_events();
            let mut bar_builder = BarBuilder::new(5);

            loop {
                match rx.recv().await {
                    Ok(evt) => match evt {
                        trader_futu_connector::OpenDEvent::Quote(q) => {
                            this.inner.metrics.inc_quote();
                            {
                                let mut m = this.inner.quotes.write();
                                m.insert(q.symbol.clone(), q.clone());
                            }
                            {
                                let mut live = this.inner.live.lock();
                                live.last_quote_ts = Some(q.ts);
                            }
                            let _ = this.inner.event_tx.send(EngineEvent::Quote(q.clone()));

                            if let Some(candle) = bar_builder.on_quote(&q) {
                                this.inner.metrics.inc_candle();
                                let _ = this
                                    .inner
                                    .event_tx
                                    .send(EngineEvent::Candle(candle.clone()));
                                this.handle_candle(candle).await;
                            }
                        }
                        trader_futu_connector::OpenDEvent::Candle(c) => {
                            this.inner.metrics.inc_candle();
                            let _ = this.inner.event_tx.send(EngineEvent::Candle(c.clone()));
                            this.handle_candle(c).await;
                        }
                        trader_futu_connector::OpenDEvent::OrderUpdated(o) => {
                            {
                                let mut live = this.inner.live.lock();
                                live.orders.insert(o.id.clone(), o.clone());
                            }
                            let _ = this
                                .inner
                                .event_tx
                                .send(EngineEvent::OrderUpdated(o.clone()));
                            this.audit_info(
                                "live_order_updated",
                                serde_json::json!({"order": o}),
                                None,
                            )
                            .await;
                        }
                        trader_futu_connector::OpenDEvent::Fill(f) => {
                            this.inner.metrics.inc_fill();
                            let _ = this.inner.event_tx.send(EngineEvent::Fill(f.clone()));
                            this.audit_info("live_fill", serde_json::json!({"fill": f}), None)
                                .await;
                        }
                        trader_futu_connector::OpenDEvent::Position(p) => {
                            {
                                let mut live = this.inner.live.lock();
                                live.positions.insert(p.symbol.clone(), p.clone());
                            }
                            let _ = this
                                .inner
                                .event_tx
                                .send(EngineEvent::PositionUpdated(p.clone()));
                        }
                        trader_futu_connector::OpenDEvent::Info { message } => {
                            let _ = this.inner.event_tx.send(EngineEvent::Info { message });
                        }
                    },
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        let _ = this.inner.event_tx.send(EngineEvent::Info {
                            message: "OpenD event receiver lagged; some events were dropped"
                                .to_string(),
                        });
                    }
                }
            }
        });
    }

    async fn disconnect_live(&self) {
        let client = {
            let mut live = self.inner.live.lock();
            let c = live.client.clone();
            *live = LiveState::default();
            c
        };
        if let Some(c) = client {
            c.close().await;
        }
    }

    async fn live_subscribe_watchlist(&self, symbols: Vec<String>) -> anyhow::Result<()> {
        let client = { self.inner.live.lock().client.clone() };
        let Some(client) = client else {
            anyhow::bail!("OpenD is not connected");
        };

        client
            .qot_subscribe_basic(symbols.clone())
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let quotes = client
            .qot_get_basic_qot(symbols)
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        for q in quotes {
            {
                let mut m = self.inner.quotes.write();
                m.insert(q.symbol.clone(), q.clone());
            }
            {
                let mut live = self.inner.live.lock();
                live.last_quote_ts = Some(q.ts);
            }
            let _ = self.inner.event_tx.send(EngineEvent::Quote(q));
        }

        self.audit_info("opend_watchlist_subscribed", serde_json::json!({}), None)
            .await;
        Ok(())
    }

    async fn live_unlock_trade(&self, client: &OpenDClient) -> anyhow::Result<()> {
        // Required for actual trade placement in OpenD.
        let profile = self.inner.cfg.read().active_profile.clone();
        let Some(pwd) =
            crate::secrets::get_secret(profile, "futu.trade_password".to_string()).await?
        else {
            anyhow::bail!("missing secret futu.trade_password (stored in OS keychain)");
        };
        let digest = md5::compute(pwd.as_bytes());
        let md5_hex = format!("{digest:x}");
        client
            .trd_unlock_trade(md5_hex)
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        Ok(())
    }

    async fn place_live_order(
        &self,
        now: DateTime<Utc>,
        req: OrderRequest,
        trace_id: Option<Uuid>,
    ) -> anyhow::Result<Order> {
        let (client, header, existing) = {
            let live = self.inner.live.lock();
            let existing_id = live.idempotency.get(&req.client_order_id).cloned();
            let existing = existing_id.and_then(|id| live.orders.get(&id).cloned());
            let client = live.client.clone();
            let header = live
                .headers
                .get(&trd_market_from_symbol(&req.symbol))
                .cloned();
            (client, header, existing)
        };

        if let Some(o) = existing {
            return Ok(o);
        }

        let Some(client) = client else {
            let _ = self
                .engage_kill_switch("OpenD is not connected (refusing live order)".to_string())
                .await;
            anyhow::bail!("OpenD is not connected");
        };
        let Some(header) = header else {
            anyhow::bail!(
                "no trade account header available for symbol {}",
                req.symbol
            );
        };

        let need_unlock = { !self.inner.live.lock().trade_unlocked };
        if need_unlock {
            self.live_unlock_trade(&client).await?;
            self.inner.live.lock().trade_unlocked = true;
        }

        let order_ref = client
            .trd_place_order(
                header,
                trader_futu_connector::OpenDPlaceOrderRequest {
                    symbol: req.symbol.clone(),
                    side: req.side,
                    qty: req.qty,
                    order_type: req.order_type,
                    limit_price: req.limit_price,
                    client_order_id: req.client_order_id.clone(),
                },
            )
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        let order_id = order_ref
            .order_id_ex
            .clone()
            .unwrap_or_else(|| order_ref.order_id.to_string());

        let order = Order {
            id: order_id.clone(),
            symbol: req.symbol.clone(),
            side: req.side,
            qty: req.qty,
            order_type: req.order_type,
            limit_price: req.limit_price,
            status: trader_shared::OrderStatus::Submitted,
            filled_qty: 0,
            avg_fill_price: None,
            created_at: now,
            updated_at: now,
            client_order_id: req.client_order_id.clone(),
            last_error: None,
        };

        {
            let mut live = self.inner.live.lock();
            live.idempotency
                .insert(req.client_order_id.clone(), order_id.clone());
            live.orders.insert(order_id.clone(), order.clone());
        }

        let _ = self
            .inner
            .event_tx
            .send(EngineEvent::OrderUpdated(order.clone()));
        self.audit_info(
            "live_order_submitted",
            serde_json::json!({"order": order, "order_ref": {"order_id": order_ref.order_id, "order_id_ex": order_ref.order_id_ex}, "trace_id": trace_id.map(|t| t.to_string())}),
            trace_id,
        )
        .await;

        Ok(order)
    }

    async fn cancel_live_order(&self, order_id: &str) -> anyhow::Result<Order> {
        let now = Utc::now();
        let (client, header, mut order) = {
            let live = self.inner.live.lock();
            let Some(client) = live.client.clone() else {
                anyhow::bail!("OpenD is not connected");
            };
            let Some(order) = live.orders.get(order_id).cloned() else {
                anyhow::bail!("unknown order_id: {order_id}");
            };
            let header = live
                .headers
                .get(&trd_market_from_symbol(&order.symbol))
                .cloned();
            (client, header, order)
        };
        let Some(header) = header else {
            anyhow::bail!("no trade header available for order {order_id}");
        };

        client
            .trd_cancel_order(header, order.id.clone())
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        order.status = trader_shared::OrderStatus::Cancelled;
        order.updated_at = now;
        {
            let mut live = self.inner.live.lock();
            live.orders.insert(order.id.clone(), order.clone());
        }

        let _ = self
            .inner
            .event_tx
            .send(EngineEvent::OrderUpdated(order.clone()));
        self.audit_info(
            "live_order_cancel_requested",
            serde_json::json!({"order_id": order_id}),
            None,
        )
        .await;
        Ok(order)
    }

    async fn live_reconcile(&self) -> anyhow::Result<()> {
        let (client, headers) = {
            let live = self.inner.live.lock();
            let Some(client) = live.client.clone() else {
                return Ok(());
            };
            let headers: Vec<trader_futu_connector::OpenDTradeHeader> =
                live.headers.values().cloned().collect();
            (client, headers)
        };

        let mut all_orders: BTreeMap<String, Order> = BTreeMap::new();
        let mut all_positions: BTreeMap<String, Position> = BTreeMap::new();
        let mut cash: Option<f64> = None;
        let mut equity: Option<f64> = None;

        for h in &headers {
            if let Ok(orders) = client.trd_get_orders(h.clone()).await {
                for o in orders {
                    all_orders.insert(o.id.clone(), o);
                }
            }
            if let Ok(positions) = client.trd_get_positions(h.clone()).await {
                for p in positions {
                    all_positions.insert(p.symbol.clone(), p);
                }
            }

            if cash.is_none() || equity.is_none() {
                if let Ok(Some(f)) = client.trd_get_funds(h.clone()).await {
                    cash = Some(f.cash);
                    equity = Some(f.total_assets);
                }
            }
        }

        let now = Utc::now();
        let limits = self.inner.active_profile.read().risk.clone();

        let (halt_reason, quote_stale_reason) = {
            let mut live = self.inner.live.lock();
            live.orders = all_orders;
            live.positions = all_positions;
            if let Some(c) = cash {
                live.cash = c;
            }
            if let Some(e) = equity {
                live.equity = e;
            }
            live.last_reconcile_at = Some(now);

            // Daily loss monitoring (only if we have an equity number).
            if now.date_naive() != live.day {
                live.day = now.date_naive();
                live.day_start_equity = live.equity;
            }
            if live.day_start_equity <= 0.0 && live.equity > 0.0 {
                live.day_start_equity = live.equity;
            }
            let halt_reason = if live.equity > 0.0 && live.day_start_equity > 0.0 {
                if live.equity < (live.day_start_equity - limits.max_daily_loss_usd) {
                    Some(format!(
                        "Daily loss limit triggered (equity={:.2}, day_start={:.2}, max_loss={:.2})",
                        live.equity, live.day_start_equity, limits.max_daily_loss_usd
                    ))
                } else {
                    None
                }
            } else {
                None
            };

            let quote_stale_reason = live.last_quote_ts.and_then(|ts| {
                let dt = now - ts;
                if dt > chrono::Duration::seconds(30) {
                    Some(format!(
                        "quote feed stale (last update {}s ago)",
                        dt.num_seconds()
                    ))
                } else {
                    None
                }
            });

            (halt_reason, quote_stale_reason)
        };

        if let Some(reason) = halt_reason.or(quote_stale_reason) {
            let _ = self.engage_kill_switch(reason).await;
        }

        Ok(())
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<EngineEvent> {
        self.inner.event_tx.subscribe()
    }

    pub async fn snapshot(&self) -> EngineSnapshot {
        let status = self.inner.status.read().clone();
        let safe_mode = self.inner.safe_mode;
        let kill_switch_engaged = self.inner.kill_switch.load(Ordering::Relaxed);
        let cfg = self.inner.cfg.read().clone();
        let active_profile = self.inner.active_profile.read().clone();

        let watchlist: Vec<String> = self.inner.watchlist.read().iter().cloned().collect();
        let quotes: Vec<Quote> = self.inner.quotes.read().values().cloned().collect();

        let (cash, positions, realized_pnl, unrealized_pnl, equity, orders) =
            match active_profile.mode {
                trader_shared::ProfileMode::Live => {
                    let live = self.inner.live.lock();
                    let cash = live.cash;
                    let positions: Vec<Position> = live.positions.values().cloned().collect();
                    let orders: Vec<Order> = live.orders.values().cloned().collect();
                    let quotes_map = self.inner.quotes.read();
                    let unreal = compute_unrealized_pnl(&positions, &quotes_map);
                    let equity = if live.equity > 0.0 {
                        live.equity
                    } else {
                        cash + compute_positions_value(&positions, &quotes_map)
                    };
                    (cash, positions, 0.0, unreal, equity, orders)
                }
                _ => {
                    let (cash, positions, realized_pnl, unrealized_pnl, equity) = {
                        let port = self.inner.portfolio.lock();
                        let cash = port.cash;
                        let positions: Vec<Position> = port.positions.values().cloned().collect();
                        let realized = port.realized_pnl;
                        let quotes_map = self.inner.quotes.read();
                        let unreal = compute_unrealized_pnl(&positions, &quotes_map);
                        let equity = cash + compute_positions_value(&positions, &quotes_map);
                        (cash, positions, realized, unreal, equity)
                    };
                    let orders: Vec<Order> =
                        { self.inner.paper.lock().orders().values().cloned().collect() };
                    (
                        cash,
                        positions,
                        realized_pnl,
                        unrealized_pnl,
                        equity,
                        orders,
                    )
                }
            };

        let strategies: Vec<RunningStrategyInfo> = {
            self.inner
                .strategies
                .lock()
                .iter()
                .map(|s| RunningStrategyInfo {
                    instance_id: s.instance_id.clone(),
                    strategy_id: s.strategy_id.clone(),
                    symbol: s.symbol.clone(),
                    started_at: s.started_at,
                    params: s.params.clone(),
                })
                .collect()
        };

        EngineSnapshot {
            status,
            safe_mode,
            kill_switch_engaged,
            config: cfg,
            active_profile,
            watchlist,
            quotes,
            cash,
            equity,
            realized_pnl,
            unrealized_pnl,
            orders,
            positions,
            strategies,
            metrics: self.inner.metrics.snapshot(),
        }
    }

    pub async fn set_watchlist(&self, symbols: Vec<String>) -> anyhow::Result<()> {
        let set: BTreeSet<String> = symbols
            .into_iter()
            .map(|s| normalize_symbol(&s))
            .filter(|s| !s.trim().is_empty())
            .collect();
        {
            let mut wl = self.inner.watchlist.write();
            *wl = set.clone();
        }

        let symbols: Vec<String> = set.iter().cloned().collect();
        let mode = self.inner.active_profile.read().mode;
        if matches!(mode, trader_shared::ProfileMode::Live) {
            self.ensure_live_connected().await?;
            self.live_subscribe_watchlist(symbols.clone()).await?;
        } else {
            self.inner
                .mock_market
                .set_subscription(MarketSubscription { symbols })
                .await;
        }

        self.audit_info("watchlist_set", serde_json::json!({"symbols": set}), None)
            .await;
        Ok(())
    }

    pub async fn get_candles(
        &self,
        symbol: &str,
        interval_sec: u32,
        limit: usize,
    ) -> anyhow::Result<Vec<trader_shared::Candle>> {
        let mode = self.inner.active_profile.read().mode;
        if matches!(mode, trader_shared::ProfileMode::Live) {
            let client = { self.inner.live.lock().client.clone() };
            let Some(client) = client else {
                anyhow::bail!("OpenD is not connected");
            };
            client
                .qot_get_kl(symbol.to_string(), interval_sec, limit as u32)
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))
        } else {
            Ok(self
                .inner
                .mock_market
                .get_candles(symbol, interval_sec, limit)
                .await)
        }
    }

    pub async fn place_order(
        &self,
        req: OrderRequest,
        trace_id: Option<Uuid>,
    ) -> anyhow::Result<Order> {
        self.place_order_inner(req, trace_id, None).await
    }

    pub async fn cancel_order(&self, order_id: &str) -> anyhow::Result<Order> {
        let mode = self.inner.active_profile.read().mode;
        if matches!(mode, trader_shared::ProfileMode::Live) {
            return self.cancel_live_order(order_id).await;
        }

        let now = Utc::now();
        let order = {
            let mut paper = self.inner.paper.lock();
            paper
                .cancel_order(now, order_id)
                .ok_or_else(|| anyhow::anyhow!("unknown order_id: {order_id}"))?
        };

        let _ = self
            .inner
            .event_tx
            .send(EngineEvent::OrderUpdated(order.clone()));
        self.audit_info(
            "order_cancelled",
            serde_json::json!({"order_id": order_id, "order": order}),
            None,
        )
        .await;
        Ok(order)
    }

    pub async fn generate_sample_candles_csv(
        &self,
        symbol: &str,
        interval_sec: u32,
        limit: usize,
    ) -> anyhow::Result<String> {
        let candles = self
            .inner
            .mock_market
            .get_candles(symbol, interval_sec, limit)
            .await;

        let dir = self.inner.paths.data_dir.join("historical");
        tokio::fs::create_dir_all(&dir).await.ok();

        let fname = format!(
            "sample_{}_{}_{}.csv",
            sanitize_symbol(symbol),
            interval_sec,
            Utc::now().timestamp()
        );
        let path = dir.join(fname);

        let mut wtr = csv::Writer::from_writer(vec![]);
        wtr.write_record([
            "ts",
            "open",
            "high",
            "low",
            "close",
            "volume",
            "interval_sec",
        ])?;
        for c in candles {
            wtr.write_record([
                c.ts.to_rfc3339(),
                c.open.to_string(),
                c.high.to_string(),
                c.low.to_string(),
                c.close.to_string(),
                c.volume.to_string(),
                c.interval_sec.to_string(),
            ])?;
        }
        let bytes = wtr.into_inner().context("finalize csv writer")?;
        tokio::fs::write(&path, bytes).await?;

        self.audit_info(
            "sample_candles_generated",
            serde_json::json!({"symbol": symbol, "interval_sec": interval_sec, "limit": limit, "path": path}),
            None,
        )
        .await;

        Ok(path.to_string_lossy().to_string())
    }

    pub async fn update_kill_switch_hotkey(&self, hotkey: String) -> anyhow::Result<()> {
        if hotkey.trim().is_empty() {
            anyhow::bail!("hotkey is empty");
        }
        {
            let mut ap = self.inner.active_profile.write();
            ap.kill_switch_hotkey = hotkey.clone();
        }
        self.persist_config().await?;
        self.audit_info(
            "kill_switch_hotkey_updated",
            serde_json::json!({"hotkey": hotkey}),
            None,
        )
        .await;
        Ok(())
    }

    pub async fn update_opend_config(
        &self,
        opend: trader_shared::OpenDConfig,
    ) -> anyhow::Result<()> {
        {
            let mut ap = self.inner.active_profile.write();
            ap.opend = opend.clone();
        }
        self.persist_config().await?;
        self.audit_info(
            "opend_config_updated",
            serde_json::json!({"opend": opend}),
            None,
        )
        .await;
        // Re-apply connection side-effects (best-effort). If OpenD was connected, reconnect.
        self.disconnect_live().await;
        let _ = self.apply_profile_side_effects().await;
        Ok(())
    }

    pub async fn update_opend_trade_env(
        &self,
        env: trader_shared::OpenDTradeEnv,
    ) -> anyhow::Result<()> {
        {
            let mut ap = self.inner.active_profile.write();
            ap.opend_trd_env = env;
        }
        self.persist_config().await?;
        self.audit_info(
            "opend_trade_env_updated",
            serde_json::json!({"env": format!("{env:?}")}),
            None,
        )
        .await;
        // Reconnect to pick up the new environment.
        self.disconnect_live().await;
        let _ = self.apply_profile_side_effects().await;
        Ok(())
    }

    pub async fn update_time_controls(
        &self,
        time_controls: trader_shared::TimeControls,
    ) -> anyhow::Result<()> {
        {
            let mut ap = self.inner.active_profile.write();
            ap.time_controls = time_controls.clone();
        }
        self.persist_config().await?;
        self.audit_info(
            "time_controls_updated",
            serde_json::json!({"time_controls": time_controls}),
            None,
        )
        .await;
        Ok(())
    }

    pub async fn update_ai_provider(&self, provider: AiProviderConfig) -> anyhow::Result<()> {
        let mut provider = provider;
        provider.id = provider.id.trim().to_lowercase();
        provider.base_url = provider.base_url.trim().to_string();
        provider.model = provider.model.trim().to_string();
        provider.api_key_secret = provider.api_key_secret.trim().to_string();
        validate_ai_provider_config(&provider)?;
        {
            let mut ap = self.inner.active_profile.write();
            ap.ai_providers
                .insert(provider.id.clone(), provider.clone());
            if ap.ai_router.primary.trim().is_empty() {
                ap.ai_router.primary = provider.id.clone();
            }
        }
        self.persist_config().await?;
        self.audit_info(
            "ai_provider_updated",
            serde_json::json!({"provider": provider}),
            None,
        )
        .await;
        Ok(())
    }

    pub async fn update_ai_router(&self, mut router: AiRouterConfig) -> anyhow::Result<()> {
        if router.primary.trim().is_empty() {
            anyhow::bail!("ai router primary provider is empty");
        }
        router.primary = router.primary.trim().to_lowercase();

        let mut dedup = Vec::new();
        for id in router.fallbacks.into_iter() {
            let id = id.trim().to_lowercase();
            if id.is_empty() || id == router.primary {
                continue;
            }
            if !dedup.iter().any(|x: &String| x == &id) {
                dedup.push(id);
            }
        }
        router.fallbacks = dedup;

        {
            let mut ap = self.inner.active_profile.write();
            ap.ai_router = router.clone();
        }
        self.persist_config().await?;
        self.audit_info(
            "ai_router_updated",
            serde_json::json!({"router": router}),
            None,
        )
        .await;
        Ok(())
    }

    pub async fn test_ai_provider(&self, provider_id: &str) -> anyhow::Result<AiSignalResponse> {
        let provider_id = provider_id.trim().to_lowercase();
        if provider_id.is_empty() {
            anyhow::bail!("provider id is empty");
        }

        let symbol = self
            .inner
            .watchlist
            .read()
            .iter()
            .next()
            .cloned()
            .unwrap_or_else(|| "US.AAPL".to_string());
        let last_price = self
            .inner
            .quotes
            .read()
            .get(&symbol)
            .map(|q| q.last)
            .unwrap_or(100.0);

        let req = AiSignalRequest {
            symbol,
            strategy_id: "ai_provider_test".to_string(),
            proposed_side: trader_shared::OrderSide::Buy,
            reason: "provider connectivity smoke test".to_string(),
            last_price,
            spread_bps: 3.0,
            horizon_sec: 60,
        };

        let profile_name = self.inner.cfg.read().active_profile.clone();
        let mut profile = self.inner.active_profile.read().clone();
        profile.ai_router.primary = provider_id.clone();
        profile.ai_router.fallbacks = Vec::new();

        let resp = self
            .inner
            .model_api
            .infer_signal(&profile_name, &profile, &req)
            .await?;

        self.audit_info(
            "ai_provider_tested",
            serde_json::json!({"provider_id": provider_id, "response": resp}),
            None,
        )
        .await;
        Ok(resp)
    }

    pub async fn generate_ai_signal(
        &self,
        req: AiSignalRequest,
    ) -> anyhow::Result<AiSignalResponse> {
        let mut req = req;
        req.symbol = normalize_symbol(&req.symbol);
        let resp = self.infer_ai_signal(req.clone()).await?;
        self.audit_info(
            "ai_signal_generated",
            serde_json::json!({"request": req, "response": resp}),
            None,
        )
        .await;
        Ok(resp)
    }

    async fn infer_ai_signal(&self, req: AiSignalRequest) -> anyhow::Result<AiSignalResponse> {
        let profile_name = self.inner.cfg.read().active_profile.clone();
        let profile = self.inner.active_profile.read().clone();
        self.inner
            .model_api
            .infer_signal(&profile_name, &profile, &req)
            .await
    }

    pub async fn test_opend_connection(&self) -> anyhow::Result<()> {
        let cfg = self.inner.active_profile.read().opend.clone();
        OpenDClient::test_connection(ConnectorOpenDConfig {
            host: cfg.host,
            port: cfg.port,
            use_tls: cfg.use_tls,
        })
        .await
        .map(|_info| ())
        .map_err(|e| anyhow::anyhow!(e.to_string()))
    }

    pub async fn secret_status(&self, key: &str) -> anyhow::Result<bool> {
        let profile = self.inner.cfg.read().active_profile.clone();
        crate::secrets::secret_exists(profile, key.to_string()).await
    }

    pub async fn set_secret(&self, key: &str, value: &str) -> anyhow::Result<()> {
        let profile = self.inner.cfg.read().active_profile.clone();
        crate::secrets::set_secret(profile, key.to_string(), value.to_string()).await?;
        self.audit_info("secret_set", serde_json::json!({"key": key}), None)
            .await;
        Ok(())
    }

    pub async fn clear_secret(&self, key: &str) -> anyhow::Result<()> {
        let profile = self.inner.cfg.read().active_profile.clone();
        crate::secrets::clear_secret(profile, key.to_string()).await?;
        self.audit_info("secret_cleared", serde_json::json!({"key": key}), None)
            .await;
        Ok(())
    }

    pub async fn start_strategy(&self, req: StartStrategyRequest) -> anyhow::Result<String> {
        let prof = self.inner.active_profile.read().clone();
        if matches!(prof.mode, trader_shared::ProfileMode::Live) && req.strategy_id == "rhai_script"
        {
            anyhow::bail!("custom scripts are disabled in live mode");
        }

        // Validate params before inserting.
        // Normalize symbol in params (so it matches normalized quotes/candles).
        let mut params = req.params.clone();
        if let Some(obj) = params.as_object_mut() {
            if let Some(sym) = obj.get("symbol").and_then(|v| v.as_str()) {
                obj.insert(
                    "symbol".to_string(),
                    serde_json::Value::String(normalize_symbol(sym)),
                );
            }
        }

        trader_strategies::validate_strategy_params(&req.strategy_id, params.clone())
            .context("invalid strategy params")?;

        // Extract symbol for routing.
        let symbol = params
            .get("symbol")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if symbol.trim().is_empty() {
            anyhow::bail!("strategy params must include symbol");
        }

        let mut strategy = trader_strategies::create_strategy(&req.strategy_id)?;
        strategy.set_params(params.clone())?;

        let instance_id = Uuid::new_v4().to_string();
        let inst = StrategyInstance {
            instance_id: instance_id.clone(),
            strategy_id: req.strategy_id.clone(),
            symbol: symbol.clone(),
            params: params.clone(),
            started_at: Utc::now(),
            strategy,
        };

        {
            self.inner.strategies.lock().push(inst);
        }

        self.audit_info(
            "strategy_started",
            serde_json::json!({"instance_id": instance_id, "strategy_id": req.strategy_id, "symbol": symbol, "params": params}),
            None,
        )
        .await;

        Ok(instance_id)
    }

    pub async fn stop_strategy(&self, instance_id: &str) -> anyhow::Result<()> {
        let removed = {
            let mut s = self.inner.strategies.lock();
            let before = s.len();
            s.retain(|x| x.instance_id != instance_id);
            before != s.len()
        };

        self.audit_info(
            "strategy_stopped",
            serde_json::json!({"instance_id": instance_id, "removed": removed}),
            None,
        )
        .await;
        Ok(())
    }

    pub async fn engage_kill_switch(&self, reason: String) -> anyhow::Result<()> {
        self.inner.kill_switch.store(true, Ordering::Relaxed);
        {
            let mut status = self.inner.status.write();
            *status = EngineStatus::Halted {
                reason: reason.clone(),
                at: Utc::now(),
            };
        }

        // Stop strategies.
        self.inner.strategies.lock().clear();

        // Cancel open orders.
        let cancelled = {
            let mut paper = self.inner.paper.lock();
            paper.cancel_all_open(Utc::now())
        };
        let cancelled_count = cancelled.len();

        for o in cancelled.iter() {
            let _ = self
                .inner
                .event_tx
                .send(EngineEvent::OrderUpdated(o.clone()));
        }

        // Best-effort cancel live orders if connected.
        let (live_client, live_headers, live_open_orders) = {
            let live = self.inner.live.lock();
            let client = live.client.clone();
            let headers = live.headers.clone();
            let open: Vec<Order> = live
                .orders
                .values()
                .filter(|o| {
                    matches!(
                        o.status,
                        trader_shared::OrderStatus::PendingSubmit
                            | trader_shared::OrderStatus::Submitted
                    )
                })
                .cloned()
                .collect();
            (client, headers, open)
        };
        let live_open_count = live_open_orders.len();
        if let Some(client) = live_client {
            for o in live_open_orders {
                if let Some(header) = live_headers
                    .get(&trd_market_from_symbol(&o.symbol))
                    .cloned()
                {
                    let _ = client.trd_cancel_order(header, o.id.clone()).await;
                }
            }
        }

        let _ = self.inner.event_tx.send(EngineEvent::RiskHalt {
            reason: reason.clone(),
        });
        self.audit_info(
            "kill_switch",
            serde_json::json!({"reason": reason, "cancelled_paper_orders": cancelled_count, "cancelled_live_orders_attempted": live_open_count}),
            None,
        )
        .await;
        Ok(())
    }

    pub async fn resume(&self) -> anyhow::Result<()> {
        if self.inner.safe_mode {
            anyhow::bail!("safe mode is active (restart after clearing crash marker)");
        }
        self.inner.kill_switch.store(false, Ordering::Relaxed);
        {
            let mut status = self.inner.status.write();
            *status = EngineStatus::Running;
        }
        self.audit_info("engine_resumed", serde_json::json!({}), None)
            .await;
        Ok(())
    }

    pub async fn update_risk_limits(&self, limits: RiskLimits) -> anyhow::Result<()> {
        {
            let mut ap = self.inner.active_profile.write();
            ap.risk = limits.clone();
        }
        {
            self.inner.risk.lock().update_limits(limits.clone());
        }
        self.persist_config().await?;
        self.audit_info(
            "risk_limits_updated",
            serde_json::json!({"limits": limits}),
            None,
        )
        .await;
        Ok(())
    }

    pub async fn set_active_profile(&self, profile: &str) -> anyhow::Result<()> {
        let mut cfg = self.inner.cfg.write().clone();
        if !cfg.profiles.contains_key(profile) {
            anyhow::bail!("unknown profile: {profile}");
        }
        cfg.active_profile = profile.to_string();
        let new_prof = cfg.profiles.get(profile).cloned().unwrap_or_default();
        {
            *self.inner.cfg.write() = cfg.clone();
            *self.inner.active_profile.write() = new_prof.clone();
        }
        {
            self.inner.risk.lock().update_limits(new_prof.risk.clone());
        }
        config::save(&self.inner.paths, &cfg).await?;
        self.audit_info(
            "profile_changed",
            serde_json::json!({"profile": profile}),
            None,
        )
        .await;
        // Apply data-source side-effects (connect/disconnect OpenD, switch mock feed).
        self.apply_profile_side_effects().await?;
        Ok(())
    }

    pub async fn list_strategy_defs(&self) -> anyhow::Result<Vec<StrategyDefinition>> {
        self.inner.state_db.list_strategy_defs().await
    }

    pub async fn upsert_strategy_def(
        &self,
        req: StrategyUpsertRequest,
    ) -> anyhow::Result<StrategyDefinition> {
        if req.name.trim().is_empty() {
            anyhow::bail!("name is empty");
        }
        if req.strategy_id.trim().is_empty() {
            anyhow::bail!("strategy_id is empty");
        }

        let mut params = req.params.clone();
        if let Some(obj) = params.as_object_mut() {
            if let Some(sym) = obj.get("symbol").and_then(|v| v.as_str()) {
                obj.insert(
                    "symbol".to_string(),
                    serde_json::Value::String(normalize_symbol(sym)),
                );
            }
        }

        trader_strategies::validate_strategy_params(&req.strategy_id, params.clone())
            .context("invalid strategy params")?;

        let id = req.id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let existing = self.inner.state_db.get_strategy_def(id.clone()).await?;
        let now = Utc::now();
        let created_at = existing.as_ref().map(|d| d.created_at).unwrap_or(now);

        let def = StrategyDefinition {
            id,
            name: req.name,
            strategy_id: req.strategy_id,
            params,
            lifecycle: req.lifecycle,
            created_at,
            updated_at: now,
        };

        let saved = self.inner.state_db.upsert_strategy_def(def).await?;
        self.audit_info(
            "strategy_def_upserted",
            serde_json::json!({"strategy": saved}),
            None,
        )
        .await;
        Ok(saved)
    }

    pub async fn delete_strategy_def(&self, id: &str) -> anyhow::Result<()> {
        self.inner
            .state_db
            .delete_strategy_def(id.to_string())
            .await?;
        self.audit_info("strategy_def_deleted", serde_json::json!({"id": id}), None)
            .await;
        Ok(())
    }

    pub async fn start_strategy_def(&self, id: &str) -> anyhow::Result<String> {
        let Some(def) = self.inner.state_db.get_strategy_def(id.to_string()).await? else {
            anyhow::bail!("unknown strategy_def id: {id}");
        };
        if matches!(def.lifecycle, StrategyLifecycle::Draft) {
            anyhow::bail!("strategy is in draft lifecycle; set lifecycle to paper/live to run");
        }

        // Extra guard: only allow running live-lifecycle strategies in live profile.
        let prof = self.inner.active_profile.read().clone();
        if matches!(def.lifecycle, StrategyLifecycle::Live)
            && !matches!(prof.mode, trader_shared::ProfileMode::Live)
        {
            anyhow::bail!("strategy lifecycle is live, but active profile is not live");
        }

        self.start_strategy(StartStrategyRequest {
            strategy_id: def.strategy_id,
            params: def.params,
        })
        .await
    }

    pub async fn list_models(&self) -> anyhow::Result<Vec<RegisteredModel>> {
        self.inner.state_db.list_models().await
    }

    pub async fn register_model(
        &self,
        req: ModelRegisterRequest,
    ) -> anyhow::Result<RegisteredModel> {
        if req.name.trim().is_empty() {
            anyhow::bail!("name is empty");
        }
        if req.base_id.trim().is_empty() {
            anyhow::bail!("base_id is empty");
        }
        if req.version.trim().is_empty() {
            anyhow::bail!("version is empty");
        }

        let id = req.id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let now = Utc::now();

        // Validate and normalize artifact handling.
        let (artifact_path, checksum) = match req.kind {
            ModelKind::Builtin => {
                // Ensure the builtin model exists and params are valid.
                let mut m = trader_models::create_model(&req.base_id)
                    .with_context(|| format!("unknown builtin model: {}", req.base_id))?;
                m.set_params(req.params.clone())
                    .context("invalid model params")?;

                let params_str =
                    serde_json::to_string(&req.params).unwrap_or_else(|_| "{}".to_string());
                let checksum = sha256_hex(
                    format!("builtin|{}|{}|{}", req.base_id, req.version, params_str).as_bytes(),
                );
                (None, checksum)
            }
            ModelKind::Onnx => {
                let src = req
                    .artifact_source_path
                    .clone()
                    .ok_or_else(|| anyhow::anyhow!("artifact_source_path is required for onnx"))?;
                let src_path = std::path::PathBuf::from(&src);
                if !src_path.exists() {
                    anyhow::bail!("artifact_source_path not found: {src}");
                }

                let dir = self.inner.paths.data_dir.join("models");
                tokio::fs::create_dir_all(&dir).await.ok();
                let fname = format!("model_{}.onnx", id.replace('-', ""));
                let dst_path = dir.join(fname);
                tokio::fs::copy(&src_path, &dst_path)
                    .await
                    .context("copy model artifact")?;

                let bytes = tokio::fs::read(&dst_path)
                    .await
                    .context("read copied model artifact")?;
                let checksum = sha256_hex(&bytes);
                (Some(dst_path.to_string_lossy().to_string()), checksum)
            }
        };

        let existing = self.inner.state_db.get_model(id.clone()).await?;
        let created_at = existing.as_ref().map(|m| m.created_at).unwrap_or(now);

        let m = RegisteredModel {
            id,
            base_id: req.base_id,
            kind: req.kind,
            name: req.name,
            version: req.version,
            checksum,
            artifact_path,
            params: req.params,
            created_at,
            updated_at: now,
        };

        let saved = self.inner.state_db.upsert_model(m).await?;
        self.audit_info(
            "model_registered",
            serde_json::json!({"model": saved}),
            None,
        )
        .await;
        Ok(saved)
    }

    pub async fn delete_model(&self, id: &str) -> anyhow::Result<()> {
        let existing = self.inner.state_db.get_model(id.to_string()).await?;
        self.inner.state_db.delete_model(id.to_string()).await?;

        // Best-effort delete artifact.
        if let Some(m) = existing {
            if let Some(p) = m.artifact_path {
                let path = std::path::PathBuf::from(p);
                let _ = tokio::fs::remove_file(path).await;
            }
        }

        self.audit_info("model_deleted", serde_json::json!({"id": id}), None)
            .await;
        Ok(())
    }

    pub async fn evaluate_model(
        &self,
        params: ModelEvalParams,
    ) -> anyhow::Result<ModelEvalRunResult> {
        let started_at = Utc::now();
        let Some(model) = self
            .inner
            .state_db
            .get_model(params.model_id.clone())
            .await?
        else {
            anyhow::bail!("unknown model_id: {}", params.model_id);
        };

        let candles = backtest::load_candles_csv(
            std::path::Path::new(&params.candles_csv_path),
            &params.symbol,
        )
        .context("load candles csv")?;

        let metrics = evaluate_model_on_candles(&model, &candles).context("evaluate model")?;
        let finished_at = Utc::now();

        let report = ModelEvalReport {
            params: params.clone(),
            started_at,
            finished_at,
            metrics,
        };

        let id = Uuid::new_v4().to_string();
        let dir = self.inner.paths.data_dir.join("model_evals").join(&id);
        tokio::fs::create_dir_all(&dir).await.ok();

        let json_path = dir.join("report.json");
        let html_path = dir.join("report.html");

        tokio::fs::write(&json_path, serde_json::to_vec_pretty(&report)?).await?;
        tokio::fs::write(&html_path, render_model_eval_html(&report, &model)).await?;

        self.inner
            .state_db
            .insert_model_eval_row(crate::state_db::ModelEvalRow {
                id,
                model_id: model.id.clone(),
                report_json_path: json_path.to_string_lossy().to_string(),
                report_html_path: html_path.to_string_lossy().to_string(),
                created_at: Utc::now(),
            })
            .await
            .ok();

        self.audit_info(
            "model_evaluated",
            serde_json::json!({"model_id": model.id, "json_path": json_path, "html_path": html_path, "metrics": report.metrics}),
            None,
        )
        .await;

        Ok(ModelEvalRunResult {
            report,
            report_json_path: json_path.to_string_lossy().to_string(),
            report_html_path: html_path.to_string_lossy().to_string(),
        })
    }

    pub async fn list_audit_events(
        &self,
        limit: u32,
        offset: u32,
        event_type: Option<String>,
        trace_id: Option<String>,
    ) -> anyhow::Result<Vec<crate::audit::AuditEventRow>> {
        self.inner
            .audit
            .list(limit, offset, event_type, trace_id)
            .await
    }

    pub async fn export_audit_jsonl(&self) -> anyhow::Result<String> {
        let out = self
            .inner
            .paths
            .data_dir
            .join("exports")
            .join(format!("audit_{}.jsonl", Utc::now().timestamp()));
        self.inner.audit.export_jsonl(out.clone()).await?;
        Ok(out.to_string_lossy().to_string())
    }

    pub async fn run_backtest(
        &self,
        params: BacktestParams,
    ) -> anyhow::Result<(BacktestReport, String, String)> {
        let report = backtest::run_backtest(params)?;

        let id = Uuid::new_v4().to_string();
        let dir = self.inner.paths.backtests_dir.join(&id);
        tokio::fs::create_dir_all(&dir).await.ok();

        let json_path = dir.join("report.json");
        let html_path = dir.join("report.html");

        tokio::fs::write(&json_path, serde_json::to_vec_pretty(&report)?).await?;
        tokio::fs::write(&html_path, backtest::render_report_html(&report)).await?;

        self.audit_info(
            "backtest_ran",
            serde_json::json!({"id": id, "json_path": json_path, "html_path": html_path}),
            None,
        )
        .await;

        Ok((
            report,
            json_path.to_string_lossy().to_string(),
            html_path.to_string_lossy().to_string(),
        ))
    }

    pub async fn enable_live_trading_unlock(
        &self,
        confirmation_phrase: &str,
        _risk_non_default: bool,
        _kill_switch_configured: bool,
    ) -> anyhow::Result<()> {
        const PHRASE: &str = "I UNDERSTAND LIVE TRADING RISK";
        if confirmation_phrase.trim() != PHRASE {
            anyhow::bail!("confirmation phrase mismatch");
        }

        let prof = self.inner.active_profile.read().clone();
        if !matches!(prof.mode, trader_shared::ProfileMode::Live) {
            anyhow::bail!("live trading can only be unlocked in the live profile");
        }

        let def = RiskLimits::default();
        let r = &prof.risk;
        let risk_non_default = r.max_position_qty != def.max_position_qty
            || r.max_order_qty != def.max_order_qty
            || r.max_orders_per_minute != def.max_orders_per_minute
            || (r.max_daily_loss_usd - def.max_daily_loss_usd).abs() > f64::EPSILON
            || r.symbol_allowlist != def.symbol_allowlist
            || (r.price_band_pct - def.price_band_pct).abs() > f64::EPSILON;
        if !risk_non_default {
            anyhow::bail!(
                "risk limits must be configured (non-default) before unlocking live trading"
            );
        }

        let kill_switch_configured = !prof.kill_switch_hotkey.trim().is_empty();
        if !kill_switch_configured {
            anyhow::bail!("kill switch hotkey must be configured before unlocking live trading");
        }

        if matches!(prof.opend_trd_env, trader_shared::OpenDTradeEnv::Simulate) {
            anyhow::bail!("OpenD trading environment is still set to simulate; switch to real before unlocking live trading");
        }

        // Require that the trade password is present (stored in OS keychain).
        let profile = self.inner.cfg.read().active_profile.clone();
        let ok = crate::secrets::secret_exists(profile, "futu.trade_password".to_string()).await?;
        if !ok {
            anyhow::bail!("missing secret futu.trade_password (stored in OS keychain)");
        }

        // Dry-run connectivity test: full OpenD handshake + basic state query on the *live* connection.
        self.ensure_live_connected()
            .await
            .context("OpenD connect failed")?;
        let client = { self.inner.live.lock().client.clone() };
        let Some(client) = client else {
            anyhow::bail!("OpenD is not connected after connect attempt");
        };
        client
            .get_global_state()
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))
            .context("OpenD get_global_state failed")?;

        // Unlock OpenD trading (requires trade password).
        self.live_unlock_trade(&client)
            .await
            .context("OpenD unlock trade failed")?;
        self.inner.live.lock().trade_unlocked = true;

        {
            let mut ap = self.inner.active_profile.write();
            ap.live_trading_unlocked = true;
        }
        self.persist_config().await?;

        self.audit_info("live_trading_unlocked", serde_json::json!({}), None)
            .await;
        Ok(())
    }

    async fn persist_config(&self) -> anyhow::Result<()> {
        let mut cfg = self.inner.cfg.write().clone();
        let active_name = cfg.active_profile.clone();
        cfg.profiles
            .insert(active_name, self.inner.active_profile.read().clone());
        *self.inner.cfg.write() = cfg.clone();
        config::save(&self.inner.paths, &cfg).await
    }

    async fn handle_candle(&self, candle: trader_shared::Candle) {
        if !matches!(*self.inner.status.read(), EngineStatus::Running) {
            return;
        }
        if self.inner.kill_switch.load(Ordering::Relaxed) {
            return;
        }

        let quote = { self.inner.quotes.read().get(&candle.symbol).cloned() };
        let mode = self.inner.active_profile.read().mode;
        let pos = if matches!(mode, trader_shared::ProfileMode::Live) {
            let live = self.inner.live.lock();
            live.positions.get(&candle.symbol).cloned()
        } else {
            let port = self.inner.portfolio.lock();
            port.positions.get(&candle.symbol).cloned()
        };

        // Run strategies without awaiting while holding the lock.
        let signals: Vec<(Uuid, String, serde_json::Value, trader_shared::Signal)> = {
            let mut out = Vec::new();
            let mut s = self.inner.strategies.lock();
            for inst in s.iter_mut() {
                if inst.symbol != candle.symbol {
                    continue;
                }
                let trace_id = Uuid::new_v4();
                let ctx = StrategyContext {
                    trace_id,
                    now: candle.ts,
                    last_quote: quote.clone(),
                    position: pos.clone(),
                };
                let sigs = inst.strategy.on_bar(&ctx, &candle);
                for sig in sigs {
                    out.push((trace_id, inst.strategy_id.clone(), inst.params.clone(), sig));
                }
            }
            out
        };

        for (trace_id, strategy_id, strategy_params, sig) in signals {
            let _ = self.inner.event_tx.send(EngineEvent::SignalFired {
                ts: candle.ts,
                trace_id,
                strategy_id: strategy_id.clone(),
                symbol: candle.symbol.clone(),
                order: sig.order.clone(),
                reason: sig.reason.clone(),
            });
            self.audit_info(
                "signal_fired",
                serde_json::json!({
                    "strategy_id": strategy_id,
                    "symbol": candle.symbol,
                    "reason": sig.reason,
                    "order": sig.order,
                    "bar": candle,
                    "last_quote": quote,
                    "position": pos,
                }),
                Some(trace_id),
            )
            .await;

            if !self
                .signal_allowed_by_ai_gate(
                    trace_id,
                    &strategy_id,
                    &strategy_params,
                    &sig,
                    &candle,
                    quote.as_ref(),
                )
                .await
            {
                continue;
            }
            let _ = self
                .place_order_inner(sig.order, Some(trace_id), Some(sig.reason))
                .await;
        }
    }

    async fn signal_allowed_by_ai_gate(
        &self,
        trace_id: Uuid,
        strategy_id: &str,
        strategy_params: &serde_json::Value,
        signal: &trader_shared::Signal,
        candle: &trader_shared::Candle,
        quote: Option<&Quote>,
    ) -> bool {
        // Apply model API confirmation only to the short-term strategy when explicitly enabled.
        if strategy_id != "short_term_momentum_bot" {
            return true;
        }
        let ai_confirm = strategy_params
            .get("ai_confirm")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !ai_confirm {
            return true;
        }

        let min_ai_conf = strategy_params
            .get("min_ai_confidence")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.65)
            .clamp(0.0, 1.0);
        let horizon_sec = strategy_params
            .get("horizon_sec")
            .and_then(|v| v.as_u64())
            .unwrap_or(60) as u32;

        let spread_bps = quote
            .and_then(|q| {
                if q.last <= 0.0 {
                    return None;
                }
                Some(((q.ask - q.bid).max(0.0) / q.last) * 10_000.0)
            })
            .unwrap_or(0.0);

        let req = AiSignalRequest {
            symbol: candle.symbol.clone(),
            strategy_id: strategy_id.to_string(),
            proposed_side: signal.order.side,
            reason: signal.reason.clone(),
            last_price: candle.close,
            spread_bps,
            horizon_sec,
        };

        let resp = match self.infer_ai_signal(req.clone()).await {
            Ok(r) => r,
            Err(err) => {
                self.audit_info(
                    "signal_ai_gate_error",
                    serde_json::json!({"strategy_id": strategy_id, "request": req, "error": err.to_string()}),
                    Some(trace_id),
                )
                .await;
                return false;
            }
        };

        let expected = order_side_to_ai_action(signal.order.side);
        let action_match = resp.action == expected;
        let confidence_ok = resp.confidence >= min_ai_conf;
        let allowed = action_match && confidence_ok;

        self.audit_info(
            "signal_ai_gate",
            serde_json::json!({
                "strategy_id": strategy_id,
                "allowed": allowed,
                "min_ai_confidence": min_ai_conf,
                "action_match": action_match,
                "confidence_ok": confidence_ok,
                "request": req,
                "response": resp,
            }),
            Some(trace_id),
        )
        .await;

        allowed
    }

    async fn place_order_inner(
        &self,
        req: OrderRequest,
        trace_id: Option<Uuid>,
        strategy_reason: Option<String>,
    ) -> anyhow::Result<Order> {
        let now = Utc::now();
        let prof = self.inner.active_profile.read().clone();

        let mut req = req;
        req.symbol = normalize_symbol(&req.symbol);

        let last_quote = { self.inner.quotes.read().get(&req.symbol).cloned() }
            .ok_or_else(|| anyhow::anyhow!("no quote for symbol {}", req.symbol))?;

        let position = {
            let port = self.inner.portfolio.lock();
            port.positions.get(&req.symbol).cloned()
        };

        let halted = !matches!(*self.inner.status.read(), EngineStatus::Running);
        let kill = self.inner.kill_switch.load(Ordering::Relaxed);
        let risk_res = {
            let mut risk = self.inner.risk.lock();
            risk.validate_order(OrderValidationCtx {
                now,
                mode: prof.mode,
                live_trading_unlocked: prof.live_trading_unlocked,
                kill_switch_engaged: kill,
                halted,
                time_controls: &prof.time_controls,
                req: &req,
                last_quote: Some(&last_quote),
                position: position.as_ref(),
            })
        };

        if let Err(RiskReject { reason }) = risk_res {
            self.inner.metrics.inc_order_rejected();
            self.audit_info(
                "order_rejected",
                serde_json::json!({"req": req, "reason": reason, "trace_id": trace_id.map(|t| t.to_string()), "strategy_reason": strategy_reason}),
                trace_id,
            )
            .await;
            return Err(anyhow::anyhow!("risk rejected: {reason}"));
        }

        self.inner.metrics.inc_order_placed();
        self.audit_info(
            "order_placed",
            serde_json::json!({"req": req, "trace_id": trace_id.map(|t| t.to_string()), "strategy_reason": strategy_reason}),
            trace_id,
        )
        .await;

        if matches!(prof.mode, trader_shared::ProfileMode::Live) {
            return self.place_live_order(now, req, trace_id).await;
        }

        let (order, fill) = {
            let mut paper = self.inner.paper.lock();
            paper.place_order(now, req, &last_quote)
        };

        let _ = self
            .inner
            .event_tx
            .send(EngineEvent::OrderUpdated(order.clone()));

        if let Some(fill) = fill {
            self.inner.metrics.inc_fill();
            self.apply_fill(&fill).await;
            let _ = self.inner.event_tx.send(EngineEvent::Fill(fill.clone()));
            self.audit_info(
                "order_filled",
                serde_json::json!({"order": order, "fill": fill}),
                trace_id,
            )
            .await;
        }

        Ok(order)
    }

    async fn apply_fill(&self, fill: &trader_shared::Fill) {
        let limits = self.inner.risk.lock().limits().clone();

        let (updated_pos, halt_reason) = {
            let mut port = self.inner.portfolio.lock();

            // Reset daily baseline on UTC day boundary.
            let day = fill.ts.date_naive();
            if day != port.day {
                let quotes = self.inner.quotes.read();
                let equity = port.cash + compute_positions_value_map(&port.positions, &quotes);
                port.day = day;
                port.day_start_equity = equity;
            }

            let mut pos = port
                .positions
                .get(&fill.symbol)
                .cloned()
                .unwrap_or(Position {
                    symbol: fill.symbol.clone(),
                    qty: 0,
                    avg_cost: 0.0,
                    realized_pnl: 0.0,
                    updated_at: fill.ts,
                });

            match fill.side {
                trader_shared::OrderSide::Buy => {
                    let cost = (fill.qty as f64) * fill.price + fill.fee;
                    port.cash -= cost;
                    let old_qty = pos.qty;
                    let new_qty = old_qty + fill.qty as i64;
                    let new_cost_basis =
                        pos.avg_cost * (old_qty as f64) + (fill.qty as f64) * fill.price + fill.fee;
                    pos.qty = new_qty;
                    pos.avg_cost = if new_qty > 0 {
                        new_cost_basis / (new_qty as f64)
                    } else {
                        0.0
                    };
                }
                trader_shared::OrderSide::Sell => {
                    let sell_qty = fill.qty.min(pos.qty.max(0) as u32);
                    let proceeds = (sell_qty as f64) * fill.price - fill.fee;
                    port.cash += proceeds;
                    let pnl = (fill.price - pos.avg_cost) * (sell_qty as f64) - fill.fee;
                    port.realized_pnl += pnl;
                    pos.realized_pnl += pnl;
                    pos.qty -= sell_qty as i64;
                    if pos.qty == 0 {
                        pos.avg_cost = 0.0;
                    }
                }
            }

            pos.updated_at = fill.ts;
            port.positions.insert(fill.symbol.clone(), pos.clone());

            let quotes = self.inner.quotes.read();
            let equity = port.cash + compute_positions_value_map(&port.positions, &quotes);
            let halt_reason = if equity < (port.day_start_equity - limits.max_daily_loss_usd) {
                Some(format!(
                    "Daily loss limit triggered (equity={equity:.2}, day_start={:.2}, max_loss={:.2})",
                    port.day_start_equity, limits.max_daily_loss_usd
                ))
            } else {
                None
            };

            (pos, halt_reason)
        };

        let _ = self
            .inner
            .event_tx
            .send(EngineEvent::PositionUpdated(updated_pos));

        if let Some(reason) = halt_reason {
            let _ = self.engage_kill_switch(reason).await;
        }
    }

    async fn audit_info(
        &self,
        event_type: &str,
        payload: serde_json::Value,
        trace_id: Option<Uuid>,
    ) {
        let _ = self
            .inner
            .audit
            .append(Utc::now(), event_type, &payload, trace_id)
            .await;
    }
}

async fn write_crash_marker(paths: &AppPaths) -> anyhow::Result<()> {
    std::fs::create_dir_all(&paths.data_dir)?;
    let v = serde_json::json!({"started_at": Utc::now().to_rfc3339()});
    tokio::fs::write(&paths.crash_marker_path, serde_json::to_vec_pretty(&v)?).await?;
    Ok(())
}

pub async fn clear_crash_marker(paths: &AppPaths) -> anyhow::Result<()> {
    if paths.crash_marker_path.exists() {
        tokio::fs::remove_file(&paths.crash_marker_path).await.ok();
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    hex::encode(digest)
}

fn pearson_corr(x: &[f64], y: &[f64]) -> f64 {
    if x.len() < 2 || x.len() != y.len() {
        return 0.0;
    }
    let n = x.len() as f64;
    let mx = x.iter().sum::<f64>() / n;
    let my = y.iter().sum::<f64>() / n;
    let mut num = 0.0;
    let mut vx = 0.0;
    let mut vy = 0.0;
    for i in 0..x.len() {
        let dx = x[i] - mx;
        let dy = y[i] - my;
        num += dx * dy;
        vx += dx * dx;
        vy += dy * dy;
    }
    if vx <= 0.0 || vy <= 0.0 {
        return 0.0;
    }
    num / (vx.sqrt() * vy.sqrt())
}

fn evaluate_model_on_candles(
    model: &RegisteredModel,
    candles: &[trader_shared::Candle],
) -> anyhow::Result<trader_shared::ModelEvalMetrics> {
    use trader_shared::{Confusion2x2, ModelEvalMetrics};

    if candles.len() < 3 {
        return Ok(ModelEvalMetrics {
            samples: 0,
            ic: 0.0,
            accuracy: 0.0,
            confusion: Confusion2x2 {
                tp: 0,
                fp: 0,
                tn: 0,
                fn_: 0,
            },
        });
    }

    let mut tp: u32 = 0;
    let mut fp: u32 = 0;
    let mut tn: u32 = 0;
    let mut fn_: u32 = 0;
    let mut scores: Vec<f64> = Vec::new();
    let mut next_rets: Vec<f64> = Vec::new();

    match model.kind {
        ModelKind::Builtin => {
            let mut m = trader_models::create_model(&model.base_id)
                .with_context(|| format!("unknown builtin model: {}", model.base_id))?;
            m.set_params(model.params.clone())
                .context("invalid params for registered model")?;

            for i in 1..(candles.len() - 1) {
                let window = &candles[..=i];
                let Some(score) = m.compute(window)? else {
                    continue;
                };

                let p0 = candles[i].close.max(0.0000001);
                let p1 = candles[i + 1].close;
                let next_ret = (p1 / p0) - 1.0;

                let pred_up = score > 0.0;
                let actual_up = next_ret > 0.0;

                match (pred_up, actual_up) {
                    (true, true) => tp += 1,
                    (true, false) => fp += 1,
                    (false, false) => tn += 1,
                    (false, true) => fn_ += 1,
                }

                scores.push(score);
                next_rets.push(next_ret);
            }
        }
        ModelKind::Onnx => {
            use tract_onnx::prelude::*;

            let Some(path) = model.artifact_path.as_ref() else {
                anyhow::bail!("onnx model is missing artifact_path");
            };

            // Minimal, documented contract:
            // - single float input shaped [1, 3]
            // - output is a scalar score (we take the first element)
            // Features: [ret1, sma_delta, zscore]
            let runnable = tract_onnx::onnx()
                .model_for_path(path)
                .with_context(|| format!("load onnx model from {path}"))?
                .into_optimized()
                .context("optimize onnx model")?
                .into_runnable()
                .context("make onnx model runnable")?;

            let sma_period = model
                .params
                .get("sma_period")
                .and_then(|v| v.as_u64())
                .unwrap_or(20) as usize;
            let z_period = model
                .params
                .get("zscore_period")
                .and_then(|v| v.as_u64())
                .unwrap_or(50) as usize;

            let mut closes: Vec<f64> = Vec::with_capacity(candles.len());
            for i in 0..candles.len() {
                closes.push(candles[i].close);
                if i == 0 || i >= candles.len() - 1 {
                    continue;
                }

                let prev_close = closes[i - 1].max(0.0000001);
                let close = closes[i];
                let ret1 = (close / prev_close - 1.0) as f32;
                let sma = sma_last(&closes[..=i], sma_period).unwrap_or(close);
                let sma_delta = if sma.abs() < 1e-12 {
                    0.0
                } else {
                    (close / sma - 1.0) as f32
                };
                let z = zscore_last(&closes[..=i], z_period).unwrap_or(0.0) as f32;

                let input: Tensor = tract_ndarray::arr2(&[[ret1, sma_delta, z]]).into_tensor();
                let outputs = runnable
                    .run(tvec![input.into()])
                    .context("onnx inference")?;
                let out0 = outputs.first().context("onnx output missing")?;
                let score_f32 = out0
                    .to_array_view::<f32>()
                    .context("onnx output type")?
                    .iter()
                    .next()
                    .cloned()
                    .unwrap_or(0.0);
                let score = score_f32 as f64;

                let p0 = candles[i].close.max(0.0000001);
                let p1 = candles[i + 1].close;
                let next_ret = (p1 / p0) - 1.0;

                let pred_up = score > 0.0;
                let actual_up = next_ret > 0.0;

                match (pred_up, actual_up) {
                    (true, true) => tp += 1,
                    (true, false) => fp += 1,
                    (false, false) => tn += 1,
                    (false, true) => fn_ += 1,
                }

                scores.push(score);
                next_rets.push(next_ret);
            }
        }
    }

    let samples = scores.len() as u32;
    let ic = pearson_corr(&scores, &next_rets);
    let acc = if samples == 0 {
        0.0
    } else {
        (tp + tn) as f64 / (samples as f64)
    };

    Ok(ModelEvalMetrics {
        samples,
        ic,
        accuracy: acc,
        confusion: Confusion2x2 { tp, fp, tn, fn_ },
    })
}

fn sma_last(closes: &[f64], period: usize) -> Option<f64> {
    if period == 0 || closes.len() < period {
        return None;
    }
    let slice = &closes[closes.len() - period..];
    Some(slice.iter().sum::<f64>() / period as f64)
}

fn zscore_last(closes: &[f64], period: usize) -> Option<f64> {
    if period < 2 || closes.len() < period {
        return None;
    }
    let slice = &closes[closes.len() - period..];
    let mean = slice.iter().sum::<f64>() / period as f64;
    let var = slice
        .iter()
        .map(|x| {
            let d = x - mean;
            d * d
        })
        .sum::<f64>()
        / period as f64;
    let std = var.sqrt();
    if std <= 0.0 {
        return Some(0.0);
    }
    let last = *slice.last().unwrap_or(&mean);
    Some((last - mean) / std)
}

fn render_model_eval_html(report: &ModelEvalReport, model: &RegisteredModel) -> String {
    let json = serde_json::to_string(report).unwrap_or_else(|_| "{}".to_string());
    format!(
        r#"<!doctype html>
<html>
<head>
<meta charset="utf-8"/>
<title>Model Evaluation Report</title>
<style>
  body {{ font-family: ui-sans-serif, system-ui, -apple-system; padding: 24px; max-width: 980px; margin: 0 auto; }}
  h1 {{ margin: 0 0 8px 0; }}
  .grid {{ display: grid; grid-template-columns: repeat(3, 1fr); gap: 12px; margin: 16px 0; }}
  .card {{ border: 1px solid #ddd; border-radius: 8px; padding: 12px; }}
  pre {{ white-space: pre-wrap; word-break: break-word; background: #f7f7f7; padding: 12px; border-radius: 8px; }}
  .muted {{ color: #555; font-size: 12px; }}
</style>
</head>
<body>
<h1>Model Evaluation Report</h1>
<div class="muted">
  <div><strong>Model:</strong> {name} ({kind})</div>
  <div><strong>Base:</strong> {base_id}</div>
  <div><strong>Version:</strong> {version}</div>
  <div><strong>Checksum:</strong> <code>{checksum}</code></div>
</div>
<div class="grid">
  <div class="card"><div>Samples</div><strong>{samples}</strong></div>
  <div class="card"><div>IC</div><strong>{ic:.4}</strong></div>
  <div class="card"><div>Accuracy</div><strong>{acc:.4}</strong></div>
  <div class="card"><div>TP</div><strong>{tp}</strong></div>
  <div class="card"><div>FP</div><strong>{fp}</strong></div>
  <div class="card"><div>TN</div><strong>{tn}</strong></div>
  <div class="card"><div>FN</div><strong>{fn_}</strong></div>
</div>
<h2>Raw JSON</h2>
<pre id="json"></pre>
<script>
  const report = {json};
  document.getElementById('json').textContent = JSON.stringify(report, null, 2);
</script>
</body>
</html>"#,
        name = escape_html(&model.name),
        kind = match model.kind {
            ModelKind::Builtin => "builtin",
            ModelKind::Onnx => "onnx",
        },
        base_id = escape_html(&model.base_id),
        version = escape_html(&model.version),
        checksum = escape_html(&model.checksum),
        samples = report.metrics.samples,
        ic = report.metrics.ic,
        acc = report.metrics.accuracy,
        tp = report.metrics.confusion.tp,
        fp = report.metrics.confusion.fp,
        tn = report.metrics.confusion.tn,
        fn_ = report.metrics.confusion.fn_,
        json = json,
    )
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn compute_positions_value(positions: &[Position], quotes: &BTreeMap<String, Quote>) -> f64 {
    positions
        .iter()
        .map(|p| {
            let last = quotes.get(&p.symbol).map(|q| q.last).unwrap_or(p.avg_cost);
            (p.qty.max(0) as f64) * last
        })
        .sum()
}

fn compute_positions_value_map(
    positions: &BTreeMap<String, Position>,
    quotes: &BTreeMap<String, Quote>,
) -> f64 {
    positions
        .values()
        .map(|p| {
            let last = quotes.get(&p.symbol).map(|q| q.last).unwrap_or(p.avg_cost);
            (p.qty.max(0) as f64) * last
        })
        .sum()
}

fn compute_unrealized_pnl(positions: &[Position], quotes: &BTreeMap<String, Quote>) -> f64 {
    positions
        .iter()
        .map(|p| {
            let last = quotes.get(&p.symbol).map(|q| q.last).unwrap_or(p.avg_cost);
            (last - p.avg_cost) * (p.qty.max(0) as f64)
        })
        .sum()
}

struct BarBuilder {
    interval_sec: u32,
    current: BTreeMap<String, trader_shared::Candle>,
}

impl BarBuilder {
    fn new(interval_sec: u32) -> Self {
        Self {
            interval_sec,
            current: BTreeMap::new(),
        }
    }

    fn on_quote(&mut self, q: &Quote) -> Option<trader_shared::Candle> {
        let bucket_ts = floor_time(q.ts, self.interval_sec);
        match self.current.get_mut(&q.symbol) {
            None => {
                self.current.insert(
                    q.symbol.clone(),
                    trader_shared::Candle {
                        symbol: q.symbol.clone(),
                        ts: bucket_ts,
                        interval_sec: self.interval_sec,
                        open: q.last,
                        high: q.last,
                        low: q.last,
                        close: q.last,
                        volume: q.volume,
                    },
                );
                None
            }
            Some(c) => {
                if c.ts == bucket_ts {
                    c.high = c.high.max(q.last);
                    c.low = c.low.min(q.last);
                    c.close = q.last;
                    c.volume = q.volume;
                    None
                } else {
                    // Emit previous and start new.
                    let prev = c.clone();
                    *c = trader_shared::Candle {
                        symbol: q.symbol.clone(),
                        ts: bucket_ts,
                        interval_sec: self.interval_sec,
                        open: q.last,
                        high: q.last,
                        low: q.last,
                        close: q.last,
                        volume: q.volume,
                    };
                    Some(prev)
                }
            }
        }
    }
}

fn floor_time(ts: DateTime<Utc>, interval_sec: u32) -> DateTime<Utc> {
    let secs = ts.timestamp();
    let bucket = secs - (secs % interval_sec as i64);
    DateTime::<Utc>::from_timestamp(bucket, 0).unwrap_or(ts)
}

fn validate_ai_provider_config(provider: &AiProviderConfig) -> anyhow::Result<()> {
    if provider.id.trim().is_empty() {
        anyhow::bail!("provider id is empty");
    }
    if provider.base_url.trim().is_empty() {
        anyhow::bail!("provider base_url is empty");
    }
    if provider.model.trim().is_empty() {
        anyhow::bail!("provider model is empty");
    }
    if provider.timeout_ms < 500 || provider.timeout_ms > 120_000 {
        anyhow::bail!("provider timeout_ms out of range [500, 120000]");
    }
    if provider.max_tokens == 0 || provider.max_tokens > 32_768 {
        anyhow::bail!("provider max_tokens out of range");
    }
    if !(0.0..=2.0).contains(&provider.temperature) {
        anyhow::bail!("provider temperature out of range [0, 2]");
    }
    if !matches!(provider.kind, trader_shared::AiProviderKind::Ollama)
        && provider.api_key_secret.trim().is_empty()
    {
        anyhow::bail!("api_key_secret is required for non-ollama providers");
    }
    Ok(())
}

fn order_side_to_ai_action(side: trader_shared::OrderSide) -> AiTradeAction {
    match side {
        trader_shared::OrderSide::Buy => AiTradeAction::Buy,
        trader_shared::OrderSide::Sell => AiTradeAction::Sell,
    }
}

fn sanitize_symbol(symbol: &str) -> String {
    symbol
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn normalize_symbol(symbol: &str) -> String {
    let s = symbol.trim();
    if s.is_empty() {
        return "".to_string();
    }
    let s = s.replace(' ', "").replace('_', "-");
    let s = s.to_uppercase();
    if let Some((prefix, code)) = s.split_once('.') {
        let p = prefix.trim();
        let c = code.trim();
        if p.is_empty() {
            return format!("US.{c}");
        }
        return format!("{p}.{c}");
    }
    format!("US.{s}")
}

fn trd_market_from_symbol(symbol: &str) -> i32 {
    let s = symbol.trim();
    let prefix = s
        .split_once('.')
        .map(|(p, _)| p.trim())
        .filter(|p| !p.is_empty())
        .unwrap_or("US")
        .to_uppercase();

    match prefix.as_str() {
        "HK" => trader_futu_connector::pb::trd_common::TrdMarket::Hk as i32,
        "SH" | "SZ" => trader_futu_connector::pb::trd_common::TrdMarket::Cn as i32,
        "US" => trader_futu_connector::pb::trd_common::TrdMarket::Us as i32,
        _ => trader_futu_connector::pb::trd_common::TrdMarket::Us as i32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_symbol_adds_default_market_prefix() {
        assert_eq!(normalize_symbol("aapl"), "US.AAPL");
        assert_eq!(normalize_symbol(" US.tsla "), "US.TSLA");
        assert_eq!(normalize_symbol("hk.00700"), "HK.00700");
        assert_eq!(normalize_symbol(""), "");
    }

    #[test]
    fn trd_market_mapping_is_expected() {
        assert_eq!(
            trd_market_from_symbol("US.AAPL"),
            trader_futu_connector::pb::trd_common::TrdMarket::Us as i32
        );
        assert_eq!(
            trd_market_from_symbol("HK.00700"),
            trader_futu_connector::pb::trd_common::TrdMarket::Hk as i32
        );
        assert_eq!(
            trd_market_from_symbol("SH.600519"),
            trader_futu_connector::pb::trd_common::TrdMarket::Cn as i32
        );
        assert_eq!(
            trd_market_from_symbol("UNKNOWN"),
            trader_futu_connector::pb::trd_common::TrdMarket::Us as i32
        );
    }
}
