use chrono::{DateTime, Utc};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use trader_shared::{Fill, Order, OrderRequest, OrderSide, OrderStatus, OrderType, Quote};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PaperExecutionConfig {
    pub slippage_bps: f64,
    pub fee_per_trade: f64,

    /// Simulated latency before filling marketable orders.
    pub fill_latency_ms: u64,
}

impl Default for PaperExecutionConfig {
    fn default() -> Self {
        Self {
            slippage_bps: 1.0,
            fee_per_trade: 0.10,
            fill_latency_ms: 50,
        }
    }
}

#[derive(Debug)]
pub struct PaperExecution {
    cfg: PaperExecutionConfig,
    idempotency: HashMap<String, String>, // client_order_id -> order_id
    orders: BTreeMap<String, Order>,
    open_limit_orders: BTreeSet<String>,
}

impl PaperExecution {
    pub fn new(cfg: PaperExecutionConfig) -> Self {
        Self {
            cfg,
            idempotency: HashMap::new(),
            orders: BTreeMap::new(),
            open_limit_orders: BTreeSet::new(),
        }
    }

    pub fn orders(&self) -> &BTreeMap<String, Order> {
        &self.orders
    }

    pub fn get_order(&self, order_id: &str) -> Option<Order> {
        self.orders.get(order_id).cloned()
    }

    pub fn place_order(
        &mut self,
        now: DateTime<Utc>,
        req: OrderRequest,
        last_quote: &Quote,
    ) -> (Order, Option<Fill>) {
        if let Some(existing) = self.idempotency.get(&req.client_order_id).cloned() {
            if let Some(o) = self.orders.get(&existing).cloned() {
                return (o, None);
            }
        }

        let order_id = Uuid::new_v4().to_string();
        let mut order = Order {
            id: order_id.clone(),
            symbol: req.symbol.clone(),
            side: req.side,
            qty: req.qty,
            order_type: req.order_type,
            limit_price: req.limit_price,
            status: OrderStatus::PendingSubmit,
            filled_qty: 0,
            avg_fill_price: None,
            created_at: now,
            updated_at: now,
            client_order_id: req.client_order_id.clone(),
            last_error: None,
        };

        self.idempotency
            .insert(req.client_order_id.clone(), order_id.clone());

        // Submit immediately.
        order.status = OrderStatus::Submitted;
        order.updated_at = now;

        let (fillable_now, fill_price) = match order.order_type {
            OrderType::Market => (true, market_price_for_side(last_quote, order.side)),
            OrderType::Limit => {
                let limit = order.limit_price.unwrap_or(0.0);
                let mkt = market_price_for_side(last_quote, order.side);
                let fillable = match order.side {
                    OrderSide::Buy => limit >= last_quote.ask,
                    OrderSide::Sell => limit <= last_quote.bid,
                };
                (fillable, if fillable { mkt } else { 0.0 })
            }
        };

        let mut fill: Option<Fill> = None;
        if fillable_now {
            let px = apply_slippage(fill_price, order.side, self.cfg.slippage_bps);
            let fee = self.cfg.fee_per_trade;
            order.status = OrderStatus::Filled;
            order.filled_qty = order.qty;
            order.avg_fill_price = Some(px);
            order.updated_at = now;

            fill = Some(Fill {
                order_id: order.id.clone(),
                symbol: order.symbol.clone(),
                side: order.side,
                qty: order.qty,
                price: px,
                fee,
                ts: now,
            });
        } else {
            self.open_limit_orders.insert(order.id.clone());
        }

        self.orders.insert(order.id.clone(), order.clone());
        (order, fill)
    }

    pub fn cancel_order(&mut self, now: DateTime<Utc>, order_id: &str) -> Option<Order> {
        let mut o = self.orders.get(order_id).cloned()?;
        if matches!(o.status, OrderStatus::Filled | OrderStatus::Cancelled | OrderStatus::Rejected) {
            return Some(o);
        }
        o.status = OrderStatus::Cancelled;
        o.updated_at = now;
        self.open_limit_orders.remove(order_id);
        self.orders.insert(order_id.to_string(), o.clone());
        Some(o)
    }

    pub fn cancel_all_open(&mut self, now: DateTime<Utc>) -> Vec<Order> {
        let ids: Vec<String> = self.open_limit_orders.iter().cloned().collect();
        let mut out = Vec::new();
        for id in ids {
            if let Some(o) = self.cancel_order(now, &id) {
                out.push(o);
            }
        }
        out
    }

    /// Called on each quote to check whether limit orders become fillable.
    pub fn on_quote(&mut self, now: DateTime<Utc>, quote: &Quote) -> Vec<(Order, Fill)> {
        let ids: Vec<String> = self.open_limit_orders.iter().cloned().collect();
        let mut filled = Vec::new();

        for id in ids {
            let Some(mut o) = self.orders.get(&id).cloned() else { continue };
            if o.symbol != quote.symbol {
                continue;
            }
            if !matches!(o.status, OrderStatus::Submitted | OrderStatus::PendingSubmit) {
                self.open_limit_orders.remove(&id);
                continue;
            }
            let Some(limit) = o.limit_price else { continue };

            let mkt = market_price_for_side(quote, o.side);
            let fillable = match o.side {
                OrderSide::Buy => limit >= quote.ask,
                OrderSide::Sell => limit <= quote.bid,
            };
            if !fillable {
                continue;
            }

            let px = apply_slippage(mkt, o.side, self.cfg.slippage_bps);
            let fee = self.cfg.fee_per_trade;

            o.status = OrderStatus::Filled;
            o.filled_qty = o.qty;
            o.avg_fill_price = Some(px);
            o.updated_at = now;
            self.open_limit_orders.remove(&id);
            self.orders.insert(id.clone(), o.clone());

            let f = Fill {
                order_id: o.id.clone(),
                symbol: o.symbol.clone(),
                side: o.side,
                qty: o.qty,
                price: px,
                fee,
                ts: now,
            };
            filled.push((o, f));
        }

        filled
    }
}

fn market_price_for_side(q: &Quote, side: OrderSide) -> f64 {
    match side {
        OrderSide::Buy => q.ask,
        OrderSide::Sell => q.bid,
    }
}

fn apply_slippage(price: f64, side: OrderSide, slippage_bps: f64) -> f64 {
    let adj = slippage_bps / 10_000.0;
    match side {
        OrderSide::Buy => price * (1.0 + adj),
        OrderSide::Sell => price * (1.0 - adj),
    }
}
