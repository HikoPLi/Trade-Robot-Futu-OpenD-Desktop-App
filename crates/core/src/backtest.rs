use anyhow::Context;
use chrono::{DateTime, Utc};
use std::path::Path;
use trader_shared::{
    BacktestParams, BacktestReport, BacktestTrade, Candle, OrderSide, OrderType, Position,
    StrategyContext,
};
use uuid::Uuid;

pub fn load_candles_csv(path: &Path, symbol: &str) -> anyhow::Result<Vec<Candle>> {
    let mut rdr =
        csv::Reader::from_path(path).with_context(|| format!("open candles csv: {path:?}"))?;
    #[derive(serde::Deserialize)]
    struct Row {
        ts: String,
        open: f64,
        high: f64,
        low: f64,
        close: f64,
        volume: f64,
        interval_sec: Option<u32>,
    }

    let mut out = Vec::new();
    for rec in rdr.deserialize::<Row>() {
        let r = rec.context("read csv row")?;
        let ts = DateTime::parse_from_rfc3339(&r.ts)
            .context("parse ts")?
            .with_timezone(&Utc);
        out.push(Candle {
            symbol: symbol.to_string(),
            ts,
            interval_sec: r.interval_sec.unwrap_or(60),
            open: r.open,
            high: r.high,
            low: r.low,
            close: r.close,
            volume: r.volume,
        });
    }
    Ok(out)
}

pub fn run_backtest(params: BacktestParams) -> anyhow::Result<BacktestReport> {
    let started_at = Utc::now();
    let candles = load_candles_csv(Path::new(&params.candles_csv_path), &params.symbol)?;

    let mut strategy = trader_strategies::create_strategy(&params.strategy_id)?;
    strategy
        .set_params(params.strategy_params.clone())
        .context("set strategy params")?;

    let mut cash = params.starting_cash;
    let mut position = Position {
        symbol: params.symbol.clone(),
        qty: 0,
        avg_cost: 0.0,
        realized_pnl: 0.0,
        updated_at: started_at,
    };

    let mut trades: Vec<BacktestTrade> = Vec::new();
    let mut equity_curve: Vec<(DateTime<Utc>, f64)> = Vec::new();
    let mut turnover_notional = 0.0f64;

    let mut wins = 0u32;
    let mut sells = 0u32;

    for c in candles.iter() {
        let trace_id = Uuid::new_v4();
        let ctx = StrategyContext {
            trace_id,
            now: c.ts,
            last_quote: None,
            position: Some(position.clone()),
        };

        let signals = strategy.on_bar(&ctx, c);
        for sig in signals {
            let req = sig.order;
            if req.symbol != params.symbol {
                continue;
            }
            if !matches!(req.order_type, OrderType::Market) {
                // MVP backtest only supports market orders.
                continue;
            }

            let fill_price = apply_slippage(c.close, req.side, params.slippage_bps);
            let fee = params.fee_per_trade;

            // Reject if insufficient cash.
            let executed_qty = match req.side {
                OrderSide::Buy => {
                    let cost = (req.qty as f64) * fill_price + fee;
                    if cost > cash {
                        continue;
                    }
                    let executed_qty = req.qty;
                    cash -= cost;
                    let old_qty = position.qty;
                    let new_qty = old_qty + req.qty as i64;
                    let new_cost_basis =
                        position.avg_cost * (old_qty as f64) + (req.qty as f64) * fill_price + fee;
                    position.qty = new_qty;
                    position.avg_cost = if new_qty > 0 {
                        new_cost_basis / (new_qty as f64)
                    } else {
                        0.0
                    };
                    executed_qty
                }
                OrderSide::Sell => {
                    // Long-only; sell to reduce.
                    let sell_qty = req.qty.min(position.qty.max(0) as u32);
                    if sell_qty == 0 {
                        continue;
                    }
                    let executed_qty = sell_qty;
                    let proceeds = (sell_qty as f64) * fill_price - fee;
                    cash += proceeds;
                    sells += 1;
                    let pnl = (fill_price - position.avg_cost) * (sell_qty as f64) - fee;
                    if pnl > 0.0 {
                        wins += 1;
                    }
                    position.realized_pnl += pnl;
                    position.qty -= sell_qty as i64;
                    if position.qty == 0 {
                        position.avg_cost = 0.0;
                    }
                    executed_qty
                }
            };
            let filled_notional = (executed_qty as f64) * fill_price;
            turnover_notional += filled_notional;

            trades.push(BacktestTrade {
                ts: c.ts,
                symbol: params.symbol.clone(),
                side: req.side,
                qty: req.qty,
                price: fill_price,
                fee,
                reason: sig.reason,
            });
        }

        let equity = cash + (position.qty.max(0) as f64) * c.close;
        equity_curve.push((c.ts, equity));
    }

    let ending_cash = cash;
    let ending_position_qty = position.qty;
    let ending_equity = equity_curve
        .last()
        .map(|(_, e)| *e)
        .unwrap_or(params.starting_cash);

    let total_return_pct = if params.starting_cash <= 0.0 {
        0.0
    } else {
        (ending_equity - params.starting_cash) / params.starting_cash * 100.0
    };

    let max_drawdown_pct = compute_max_drawdown_pct(&equity_curve);
    let sharpe_ratio = compute_sharpe_ratio(
        &equity_curve,
        candles.first().map(|c| c.interval_sec).unwrap_or(60),
    );
    let avg_equity = if equity_curve.is_empty() {
        params.starting_cash.max(0.0)
    } else {
        equity_curve.iter().map(|(_, v)| *v).sum::<f64>() / (equity_curve.len() as f64)
    };
    let turnover = if avg_equity > 0.0 {
        turnover_notional / avg_equity
    } else {
        0.0
    };

    let hit_rate = if sells == 0 {
        0.0
    } else {
        wins as f64 / sells as f64
    };

    let finished_at = Utc::now();

    Ok(BacktestReport {
        params,
        started_at,
        finished_at,
        ending_cash,
        ending_position_qty,
        ending_equity,
        total_return_pct,
        max_drawdown_pct,
        sharpe_ratio,
        turnover,
        hit_rate,
        trade_count: trades.len() as u32,
        equity_curve,
        trades,
    })
}

pub fn render_report_html(report: &BacktestReport) -> String {
    // Intentionally standalone HTML; no external JS.
    let json = serde_json::to_string(report).unwrap_or_else(|_| "{}".to_string());
    format!(
        r#"<!doctype html>
<html>
<head>
<meta charset="utf-8"/>
<title>Backtest Report</title>
<style>
  body {{ font-family: ui-sans-serif, system-ui, -apple-system; padding: 24px; max-width: 980px; margin: 0 auto; }}
  h1 {{ margin: 0 0 8px 0; }}
  .grid {{ display: grid; grid-template-columns: repeat(3, 1fr); gap: 12px; margin: 16px 0; }}
  .card {{ border: 1px solid #ddd; border-radius: 8px; padding: 12px; }}
  pre {{ white-space: pre-wrap; word-break: break-word; background: #f7f7f7; padding: 12px; border-radius: 8px; }}
</style>
</head>
<body>
<h1>Backtest Report</h1>
<div class="grid">
  <div class="card"><div>Total Return</div><strong>{total_return:.2}%</strong></div>
  <div class="card"><div>Max Drawdown</div><strong>{mdd:.2}%</strong></div>
  <div class="card"><div>Sharpe Ratio</div><strong>{sharpe:.3}</strong></div>
  <div class="card"><div>Turnover</div><strong>{turnover:.3}x</strong></div>
  <div class="card"><div>Hit Rate</div><strong>{hit:.2}</strong></div>
  <div class="card"><div>Trades</div><strong>{trades}</strong></div>
  <div class="card"><div>Ending Equity</div><strong>{eq:.2}</strong></div>
  <div class="card"><div>Ending Cash</div><strong>{cash:.2}</strong></div>
</div>
<h2>Raw JSON</h2>
<pre id="json"></pre>
<script>
  const report = {json};
  document.getElementById('json').textContent = JSON.stringify(report, null, 2);
</script>
</body>
</html>"#,
        total_return = report.total_return_pct,
        mdd = report.max_drawdown_pct,
        sharpe = report.sharpe_ratio,
        turnover = report.turnover,
        hit = report.hit_rate,
        trades = report.trade_count,
        eq = report.ending_equity,
        cash = report.ending_cash,
        json = json
    )
}

fn apply_slippage(price: f64, side: OrderSide, slippage_bps: f64) -> f64 {
    let adj = slippage_bps / 10_000.0;
    match side {
        OrderSide::Buy => price * (1.0 + adj),
        OrderSide::Sell => price * (1.0 - adj),
    }
}

fn compute_max_drawdown_pct(equity: &[(DateTime<Utc>, f64)]) -> f64 {
    let mut peak = f64::MIN;
    let mut max_dd = 0.0;
    for (_, v) in equity {
        if *v > peak {
            peak = *v;
        }
        if peak > 0.0 {
            let dd = (peak - *v) / peak;
            if dd > max_dd {
                max_dd = dd;
            }
        }
    }
    max_dd * 100.0
}

fn compute_sharpe_ratio(equity: &[(DateTime<Utc>, f64)], interval_sec: u32) -> f64 {
    if equity.len() < 3 {
        return 0.0;
    }
    let mut rets = Vec::with_capacity(equity.len().saturating_sub(1));
    for i in 1..equity.len() {
        let prev = equity[i - 1].1;
        let cur = equity[i].1;
        if prev > 0.0 {
            rets.push(cur / prev - 1.0);
        }
    }
    if rets.len() < 2 {
        return 0.0;
    }

    let mean = rets.iter().sum::<f64>() / (rets.len() as f64);
    let var = rets
        .iter()
        .map(|r| {
            let d = *r - mean;
            d * d
        })
        .sum::<f64>()
        / ((rets.len() - 1) as f64);
    let std = var.sqrt();
    if std <= f64::EPSILON {
        return 0.0;
    }

    let bars_per_year = (365.0 * 24.0 * 3600.0) / (interval_sec.max(1) as f64);
    (mean / std) * bars_per_year.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use uuid::Uuid;

    #[test]
    fn max_drawdown_basic() {
        let t0 = Utc::now();
        let eq = vec![
            (t0, 100.0),
            (t0, 110.0),
            (t0, 90.0),
            (t0, 120.0),
            (t0, 80.0),
        ];
        let dd = compute_max_drawdown_pct(&eq);
        assert!(dd > 0.0);
    }

    #[test]
    fn backtest_is_deterministic_for_same_input() {
        let file = std::env::temp_dir().join(format!("trader_backtest_{}.csv", Uuid::new_v4()));
        let mut f = std::fs::File::create(&file).unwrap();
        writeln!(f, "ts,open,high,low,close,volume,interval_sec").unwrap();
        let rows = [
            ("2026-01-01T00:00:00Z", 10.0),
            ("2026-01-01T00:01:00Z", 9.0),
            ("2026-01-01T00:02:00Z", 8.0),
            ("2026-01-01T00:03:00Z", 9.0),
            ("2026-01-01T00:04:00Z", 10.0),
            ("2026-01-01T00:05:00Z", 11.0),
            ("2026-01-01T00:06:00Z", 12.0),
            ("2026-01-01T00:07:00Z", 11.0),
            ("2026-01-01T00:08:00Z", 10.0),
            ("2026-01-01T00:09:00Z", 9.0),
        ];
        for (ts, c) in rows {
            writeln!(f, "{ts},{c},{c},{c},{c},100,60").unwrap();
        }

        let params = BacktestParams {
            symbol: "US.AAPL".to_string(),
            strategy_id: "ma_crossover".to_string(),
            strategy_params: serde_json::json!({
                "symbol": "US.AAPL",
                "fast_period": 2,
                "slow_period": 3,
                "trade_qty": 1
            }),
            starting_cash: 10_000.0,
            fee_per_trade: 0.0,
            slippage_bps: 0.0,
            candles_csv_path: file.to_string_lossy().to_string(),
        };

        let r1 = run_backtest(params.clone()).unwrap();
        let r2 = run_backtest(params).unwrap();

        assert_eq!(r1.trade_count, r2.trade_count);
        assert_eq!(r1.total_return_pct, r2.total_return_pct);
        assert_eq!(r1.max_drawdown_pct, r2.max_drawdown_pct);
        assert_eq!(r1.sharpe_ratio, r2.sharpe_ratio);
        assert_eq!(r1.turnover, r2.turnover);
        assert_eq!(r1.hit_rate, r2.hit_rate);

        let _ = std::fs::remove_file(file);
    }
}
