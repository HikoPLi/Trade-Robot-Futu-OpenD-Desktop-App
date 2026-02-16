# Trade Robot (Futu OpenD) Desktop App

Production-grade, cross-platform desktop trading application (paper-first) for Futu NiuNiu via Futu OpenAPI (OpenD).

## Safety Defaults

- Default mode is **Paper Trading**.
- Live trading is **locked** behind an explicit workflow (risk disclaimer + confirmation phrase + kill switch).
- When uncertain, the engine chooses the safer behavior: do nothing, cancel, or halt.

## Dev

Prereqs:
- Node + pnpm
- Rust toolchain (see `rust-toolchain.toml`)

Commands:

```bash
pnpm install
pnpm dev
```

## Build

```bash
pnpm install
pnpm build
```

## Repo Layout

- `apps/desktop`: Tauri v2 desktop app (React/TS UI + Rust backend)
- `crates/shared`: Rust domain types + contracts
- `crates/core`: Rust trading engine (event bus, risk, execution, backtest, audit)
- `packages/connectors/futu`: Futu/OpenD connector + mock connector
- `packages/strategies`: built-in strategy plugins + sandboxed scripting
- `packages/models`: built-in model plugins (indicators/signals)
- `packages/shared`: TypeScript shared types/schemas
- `docs`: architecture, threat model, runbooks

## Docs

- `docs/ARCHITECTURE.md`
- `docs/THREAT_MODEL.md`
- `docs/RUNBOOK.md`
- `docs/DEVELOPMENT.md`
- `docs/API.md`
- `docs/adr/README.md`

## Known Limitations (MVP)

- Replay mode from audit logs is not implemented yet (audit export exists; full deterministic replay UI is roadmap).
- TLS is not supported by OpenD protocol; remote deployments should use SSH/VPN tunnels.
- Signed auto-update flow is not enabled by default in this repo (packaging + signature verification docs are provided).
- Encrypted-at-rest SQLite is not implemented (audit is tamper-evident via hash chain, secrets remain in OS keychain).

## Roadmap

1. Add full replay mode (reproduce live session from audit + event snapshots).
2. Expand execution reliability tests (network fault injection, reconnect soak tests, duplicate-order defense tests).
3. Add richer model tooling (dataset registry, walk-forward matrix, Python sidecar supervision contract).
4. Add signed release + update distribution hardening (CI signing, notarization, verification policy).
5. Add optional encrypted-at-rest DB mode (SQLCipher profile).
# Trade-Robot-Futu-OpenD-Desktop-App
