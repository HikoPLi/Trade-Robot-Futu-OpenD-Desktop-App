use chrono::{DateTime, Datelike, Duration, Timelike, Utc};
use std::collections::{HashMap, VecDeque};
use trader_shared::{OrderRequest, OrderSide, OrderType, Position, ProfileMode, Quote, RiskLimits, TimeControls};

#[derive(Debug, thiserror::Error)]
#[error("risk rejected: {reason}")]
pub struct RiskReject {
    pub reason: String,
}

#[derive(Debug)]
pub struct RiskEngine {
    limits: RiskLimits,
    order_times: VecDeque<DateTime<Utc>>,
    last_order_by_symbol: HashMap<String, DateTime<Utc>>,
}

pub struct OrderValidationCtx<'a> {
    pub now: DateTime<Utc>,
    pub mode: ProfileMode,
    pub live_trading_unlocked: bool,
    pub kill_switch_engaged: bool,
    pub halted: bool,
    pub time_controls: &'a TimeControls,
    pub req: &'a OrderRequest,
    pub last_quote: Option<&'a Quote>,
    pub position: Option<&'a Position>,
}

impl RiskEngine {
    pub fn new(limits: RiskLimits) -> Self {
        Self {
            limits,
            order_times: VecDeque::new(),
            last_order_by_symbol: HashMap::new(),
        }
    }

    pub fn update_limits(&mut self, limits: RiskLimits) {
        self.limits = limits;
    }

    pub fn limits(&self) -> &RiskLimits {
        &self.limits
    }

    pub fn validate_order(
        &mut self,
        ctx: OrderValidationCtx<'_>,
    ) -> Result<(), RiskReject> {
        let OrderValidationCtx {
            now,
            mode,
            live_trading_unlocked,
            kill_switch_engaged,
            halted,
            time_controls,
            req,
            last_quote,
            position,
        } = ctx;

        if kill_switch_engaged || halted {
            return Err(RiskReject {
                reason: "engine halted/kill-switch engaged".to_string(),
            });
        }

        if matches!(mode, ProfileMode::Live) && !live_trading_unlocked {
            return Err(RiskReject {
                reason: "live trading is locked (complete Enable Live Trading workflow)".to_string(),
            });
        }

        if !self.limits.symbol_allowlist.is_empty()
            && !self
                .limits
                .symbol_allowlist
                .iter()
                .any(|s| s.eq_ignore_ascii_case(&req.symbol))
        {
            return Err(RiskReject {
                reason: format!("symbol {} not in allowlist", req.symbol),
            });
        }

        if !self.limits.allowed_markets.is_empty() {
            let mkt = symbol_market_prefix(&req.symbol);
            if !self
                .limits
                .allowed_markets
                .iter()
                .any(|m| m.eq_ignore_ascii_case(&mkt))
            {
                return Err(RiskReject {
                    reason: format!("market {} not allowed for symbol {}", mkt, req.symbol),
                });
            }
        }

        if time_controls.enabled {
            let in_session = time_controls.sessions_utc.is_empty()
                || time_controls
                    .sessions_utc
                    .iter()
                    .any(|w| window_contains_utc(w, now));
            if !in_session {
                return Err(RiskReject {
                    reason: "outside allowed trading sessions (UTC)".to_string(),
                });
            }
            let in_blackout = time_controls
                .blackout_utc
                .iter()
                .any(|w| window_contains_utc(w, now));
            if in_blackout {
                return Err(RiskReject {
                    reason: "inside blackout window (UTC)".to_string(),
                });
            }

            if time_controls.cooldown_sec > 0 {
                if let Some(last) = self.last_order_by_symbol.get(&req.symbol) {
                    let dt = now - *last;
                    if dt < Duration::seconds(time_controls.cooldown_sec as i64) {
                        return Err(RiskReject {
                            reason: format!(
                                "cooldown active for {} ({}s)",
                                req.symbol, time_controls.cooldown_sec
                            ),
                        });
                    }
                }
            }
        }

        if req.qty == 0 {
            return Err(RiskReject {
                reason: "qty must be > 0".to_string(),
            });
        }
        if req.qty > self.limits.max_order_qty {
            return Err(RiskReject {
                reason: format!("order qty {} exceeds max_order_qty {}", req.qty, self.limits.max_order_qty),
            });
        }

        // Default to no shorting for safety.
        let pos_qty = position.map(|p| p.qty).unwrap_or(0);
        let signed_delta: i64 = match req.side {
            OrderSide::Buy => req.qty as i64,
            OrderSide::Sell => -(req.qty as i64),
        };
        let new_qty = pos_qty + signed_delta;
        if new_qty < 0 {
            return Err(RiskReject {
                reason: "short selling not allowed in MVP (would go negative)".to_string(),
            });
        }
        if (new_qty as u32) > self.limits.max_position_qty {
            return Err(RiskReject {
                reason: format!("position limit exceeded: new_qty={} > max_position_qty={}", new_qty, self.limits.max_position_qty),
            });
        }

        if matches!(req.order_type, OrderType::Limit) {
            let Some(limit_price) = req.limit_price else {
                return Err(RiskReject { reason: "limit order requires limit_price".to_string() });
            };
            if limit_price <= 0.0 {
                return Err(RiskReject { reason: "limit_price must be > 0".to_string() });
            }
            let Some(q) = last_quote else {
                return Err(RiskReject { reason: "no market price available for price sanity check".to_string() });
            };
            let band = q.last * self.limits.price_band_pct;
            let lo = (q.last - band).max(0.01);
            let hi = q.last + band;
            if !(lo..=hi).contains(&limit_price) {
                return Err(RiskReject {
                    reason: format!("limit_price out of band: {limit_price:.4} not in [{lo:.4}, {hi:.4}]"),
                });
            }
        }

        // Rate-limit: sliding window of 60s.
        while let Some(ts) = self.order_times.front().cloned() {
            if now - ts > Duration::seconds(60) {
                self.order_times.pop_front();
            } else {
                break;
            }
        }
        if self.order_times.len() as u32 >= self.limits.max_orders_per_minute {
            return Err(RiskReject {
                reason: format!("rate limit: max_orders_per_minute {} exceeded", self.limits.max_orders_per_minute),
            });
        }

        // Pass: record time.
        self.order_times.push_back(now);
        if time_controls.enabled && time_controls.cooldown_sec > 0 {
            self.last_order_by_symbol.insert(req.symbol.clone(), now);
        }
        Ok(())
    }
}

fn symbol_market_prefix(symbol: &str) -> String {
    let s = symbol.trim();
    if s.is_empty() {
        return "US".to_string();
    }
    if let Some((p, _code)) = s.split_once('.') {
        let p = p.trim();
        if !p.is_empty() {
            return p.to_uppercase();
        }
    }
    "US".to_string()
}

fn parse_hhmm_to_minutes(s: &str) -> Option<u32> {
    let s = s.trim();
    let (h, m) = s.split_once(':')?;
    let h: u32 = h.parse().ok()?;
    let m: u32 = m.parse().ok()?;
    if h > 23 || m > 59 {
        return None;
    }
    Some(h * 60 + m)
}

fn window_contains_utc(w: &trader_shared::SessionWindowUtc, now: DateTime<Utc>) -> bool {
    let wd = now.weekday().number_from_monday() as u8;
    if !w.weekdays.is_empty() && !w.weekdays.contains(&wd) {
        return false;
    }
    let Some(start) = parse_hhmm_to_minutes(&w.start_hhmm) else { return false };
    let Some(end) = parse_hhmm_to_minutes(&w.end_hhmm) else { return false };
    let mins = now.hour() * 60 + now.minute();

    // If start==end: treat as empty window.
    if start == end {
        return false;
    }
    if start < end {
        (start..end).contains(&mins)
    } else {
        // Crosses midnight.
        mins >= start || mins < end
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use trader_shared::{OrderRequest, OrderSide, OrderType, ProfileMode, RiskLimits, SessionWindowUtc};

    proptest! {
        #[test]
        fn rejects_oversize_order(qty in 1u32..10_000u32) {
            let limits = RiskLimits { max_order_qty: 10, ..Default::default() };
            let mut re = RiskEngine::new(limits);

            let req = OrderRequest {
                symbol: "AAPL".to_string(),
                side: OrderSide::Buy,
                qty,
                order_type: OrderType::Market,
                limit_price: None,
                client_order_id: "x".to_string(),
            };

            let res = re.validate_order(OrderValidationCtx {
                now: Utc::now(),
                mode: ProfileMode::Paper,
                live_trading_unlocked: false,
                kill_switch_engaged: false,
                halted: false,
                time_controls: &TimeControls::default(),
                req: &req,
                last_quote: None,
                position: None,
            });
            if qty > 10 {
                prop_assert!(res.is_err());
            } else {
                prop_assert!(res.is_ok());
            }
        }
    }

    #[test]
    fn rejects_live_orders_when_not_unlocked() {
        let limits = RiskLimits::default();
        let mut re = RiskEngine::new(limits);
        let req = OrderRequest {
            symbol: "US.AAPL".to_string(),
            side: OrderSide::Buy,
            qty: 1,
            order_type: OrderType::Market,
            limit_price: None,
            client_order_id: "x".to_string(),
        };
        let res = re.validate_order(OrderValidationCtx {
            now: Utc::now(),
            mode: ProfileMode::Live,
            live_trading_unlocked: false,
            kill_switch_engaged: false,
            halted: false,
            time_controls: &TimeControls::default(),
            req: &req,
            last_quote: None,
            position: None,
        });
        assert!(res.is_err());
        assert!(res.unwrap_err().reason.contains("live trading is locked"));
    }

    #[test]
    fn enforces_allowed_markets() {
        let limits = RiskLimits {
            allowed_markets: vec!["HK".to_string()],
            ..RiskLimits::default()
        };
        let mut re = RiskEngine::new(limits);
        let req = OrderRequest {
            symbol: "US.AAPL".to_string(),
            side: OrderSide::Buy,
            qty: 1,
            order_type: OrderType::Market,
            limit_price: None,
            client_order_id: "x".to_string(),
        };
        let res = re.validate_order(OrderValidationCtx {
            now: Utc::now(),
            mode: ProfileMode::Paper,
            live_trading_unlocked: false,
            kill_switch_engaged: false,
            halted: false,
            time_controls: &TimeControls::default(),
            req: &req,
            last_quote: None,
            position: None,
        });
        assert!(res.is_err());
        assert!(res.unwrap_err().reason.contains("market US not allowed"));
    }

    #[test]
    fn enforces_session_window_and_cooldown() {
        let limits = RiskLimits::default();
        let mut re = RiskEngine::new(limits);
        let req = OrderRequest {
            symbol: "US.AAPL".to_string(),
            side: OrderSide::Buy,
            qty: 1,
            order_type: OrderType::Market,
            limit_price: None,
            client_order_id: "x".to_string(),
        };

        let controls = TimeControls {
            enabled: true,
            sessions_utc: vec![SessionWindowUtc {
                weekdays: vec![1], // Monday
                start_hhmm: "09:30".to_string(),
                end_hhmm: "16:00".to_string(),
            }],
            blackout_utc: vec![],
            cooldown_sec: 30,
        };

        // Monday 10:00 UTC: inside session.
        let inside = chrono::DateTime::parse_from_rfc3339("2026-02-16T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let ok = re.validate_order(OrderValidationCtx {
            now: inside,
            mode: ProfileMode::Paper,
            live_trading_unlocked: false,
            kill_switch_engaged: false,
            halted: false,
            time_controls: &controls,
            req: &req,
            last_quote: None,
            position: None,
        });
        assert!(ok.is_ok());

        // Within cooldown => reject.
        let inside_soon = chrono::DateTime::parse_from_rfc3339("2026-02-16T10:00:10Z")
            .unwrap()
            .with_timezone(&Utc);
        let reject_cooldown = re.validate_order(OrderValidationCtx {
            now: inside_soon,
            mode: ProfileMode::Paper,
            live_trading_unlocked: false,
            kill_switch_engaged: false,
            halted: false,
            time_controls: &controls,
            req: &req,
            last_quote: None,
            position: None,
        });
        assert!(reject_cooldown.is_err());
        assert!(reject_cooldown.unwrap_err().reason.contains("cooldown active"));

        // Outside session => reject.
        let outside = chrono::DateTime::parse_from_rfc3339("2026-02-16T08:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let reject_session = re.validate_order(OrderValidationCtx {
            now: outside,
            mode: ProfileMode::Paper,
            live_trading_unlocked: false,
            kill_switch_engaged: false,
            halted: false,
            time_controls: &controls,
            req: &req,
            last_quote: None,
            position: None,
        });
        assert!(reject_session.is_err());
        assert!(reject_session
            .unwrap_err()
            .reason
            .contains("outside allowed trading sessions"));
    }
}
