# Runbook

This runbook is for safe operation of the desktop trading workstation.

## Safety Rules

- Default to **paper** profile unless you intentionally switch to `live`.
- If anything is uncertain: **Kill Switch first**, investigation second.
- Never bypass broker/API rules or local regulations.

## Startup Checklist

1. Start app (`pnpm dev` or packaged binary).
2. Open `Dashboard`:
   - engine status should be `running` (unless safe mode)
   - kill switch should be `off`
3. Open `Market` and verify watchlist quotes update.
4. Open `Diagnostics` and confirm events are streaming.

## Paper Mode Validation

1. Place a small paper order in `Trading`.
2. Confirm order/fill/position updates.
3. Confirm audit rows are written in `Audit`.
4. Start a strategy and verify explainability signal events.

## Live Mode Validation (Before Unlock)

1. Switch profile to `live`.
2. Set OpenD host/port and run connection test.
3. Set OpenD trade env to `real`.
4. Configure non-default risk limits.
5. Ensure global kill-switch hotkey is configured.
6. Store `futu.trade_password` in OS keychain.
7. Verify unlock remains **locked** until workflow is completed.

## Enable Live Trading Workflow

The unlock button only succeeds when all conditions pass:

- confirmation phrase exactly matches:
  - `I UNDERSTAND LIVE TRADING RISK`
- non-default risk limits configured
- kill-switch hotkey configured
- active profile is `live`
- OpenD trade env is `real`
- keychain secret `futu.trade_password` exists
- OpenD connectivity/global-state check passes
- trade unlock call succeeds (`trd_unlock_trade`)

## Kill Switch

### How to trigger

- UI button: top bar `Kill Switch`
- Global hotkey: configured in `Settings`

### Immediate effects

- engine status -> halted
- all strategy instances stopped
- new orders blocked
- open paper orders cancelled
- open live orders cancel-requested to OpenD (best effort)

## Safe Mode (Crash Recovery)

If previous shutdown was unclean:

- app starts in safe mode (`status = halted`)
- live connection is blocked

Recovery:

1. Inspect logs + audit data.
2. Fix root cause.
3. Cleanly restart app.

## Incident Response

1. Trigger kill switch.
2. Export audit JSONL from `Audit` page.
3. Collect logs from app data dir:
   - `logs/trade_robot.jsonl`
4. Preserve config snapshot and report files if backtest/model eval was involved.
5. Reproduce in paper mode before re-enabling live.

## Data Locations

Platform-specific app data root:

- via `directories::ProjectDirs::from("com", "lihiko", "TradeRobot")`

Key paths:

- `config.json` (non-secret config)
- `db/trade_robot.sqlite` (audit + registries)
- `logs/trade_robot.jsonl` (structured logs)
- `backtests/<uuid>/report.{json,html}`
- `model_evals/<uuid>/report.{json,html}`

## Operational Notes

- OpenD protocol does not provide native TLS mode in this app path; use SSH/VPN for remote deployments.
- Quote staleness and daily-loss breakers can auto-halt live trading.
- Live execution intentionally avoids automatic place-order retry to reduce duplicate-order risk.
