use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Default)]
pub struct Metrics {
    quote_updates: AtomicU64,
    candles_built: AtomicU64,
    orders_placed: AtomicU64,
    orders_rejected: AtomicU64,
    fills: AtomicU64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricsSnapshot {
    pub quote_updates: u64,
    pub candles_built: u64,
    pub orders_placed: u64,
    pub orders_rejected: u64,
    pub fills: u64,
}

impl Metrics {
    pub fn inc_quote(&self) {
        self.quote_updates.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_candle(&self) {
        self.candles_built.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_order_placed(&self) {
        self.orders_placed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_order_rejected(&self) {
        self.orders_rejected.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_fill(&self) {
        self.fills.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            quote_updates: self.quote_updates.load(Ordering::Relaxed),
            candles_built: self.candles_built.load(Ordering::Relaxed),
            orders_placed: self.orders_placed.load(Ordering::Relaxed),
            orders_rejected: self.orders_rejected.load(Ordering::Relaxed),
            fills: self.fills.load(Ordering::Relaxed),
        }
    }
}
