# Development

## Prereqs

- Node.js + `pnpm` (repo pins `pnpm@9.9.0`)
- Rust toolchain (pinned in `rust-toolchain.toml`)

## One-Command Dev

```bash
pnpm install
pnpm dev
```

Notes:

- The engine runs in the Tauri Rust host process, not the UI thread.
- Paper trading works without OpenD using `MockMarketData`.
- UI supports `en`, `zh-CN`, `zh-TW` (language selector in `Settings`).

## One-Command Build

```bash
pnpm install
pnpm build
```

Bundles are produced under `target/release/bundle/` for the current OS.

## Test / Lint

```bash
pnpm typecheck
pnpm test
pnpm lint
```

Targeted test runs:

```bash
# OpenD connector mock integration tests
cargo test -p trader_futu_connector

# Core engine tests
cargo test -p trader_core
```

## Debugging

- Enable more Rust logs:
  - `RUST_LOG=info,trader_core=debug pnpm dev`
- In-app pages:
  - `Diagnostics` shows snapshot + buffered events
  - `Audit` shows persisted audit events and supports JSONL export

## Data Directory

The engine stores all local state under the platform-specific app data directory derived from:

- `directories::ProjectDirs::from(\"com\", \"lihiko\", \"TradeRobot\")`

Key files:

- `config.json` (non-secrets)
- `db/trade_robot.sqlite` (audit log)
- `logs/trade_robot.jsonl` (structured logs)
- `backtests/<uuid>/report.{json,html}`

Additional directories:

- `models/` (copied ONNX artifacts for registered models)
- `model_evals/<uuid>/report.{json,html}`

## Reproducible Builds

- Rust toolchain pinned in `rust-toolchain.toml`
- Rust dependency graph locked in `Cargo.lock`
- JS/TS dependency graph locked in `pnpm-lock.yaml`
- CI workflows:
  - `.github/workflows/ci.yml` for checks
  - `.github/workflows/bundle.yml` for cross-platform bundles
