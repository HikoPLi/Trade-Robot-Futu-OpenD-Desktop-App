# Threat Model (STRIDE)

Scope: desktop app (`apps/desktop`) + Tauri host + Rust trading engine (`crates/core`) + OpenD connector (`packages/connectors/futu`).

## Assets

- A1: account/trading permissions and credentials (trade password/token)
- A2: order intent, strategy definitions, model artifacts and parameters
- A3: risk limits, time controls, kill-switch configuration
- A4: audit trail integrity and event traceability
- A5: local historical data and evaluation/backtest outputs

## Trust Boundaries

1. UI webview <-> Tauri IPC host
2. Host <-> local filesystem (config/db/logs/exports)
3. Host <-> OS keychain/credential vault
4. Host <-> OpenD over TCP (local or tunnelled remote)
5. Strategy sandbox (Rhai script boundary)

## STRIDE Analysis

## Spoofing

- Threat: app connects to malicious OpenD endpoint.
- Current mitigations:
  - explicit host/port config
  - live unlock requires global-state/trade unlock validation
  - safer defaults (paper mode)
- Remaining risk:
  - OpenD protocol path does not enforce TLS directly.
- Recommended ops control:
  - use SSH/VPN tunnel for remote OpenD access.

## Tampering

- Threat: local config/db/log manipulation to hide actions or weaken limits.
- Current mitigations:
  - key risk gates enforced in engine, not UI only
  - audit log hash chain (`prev_hash -> hash`)
  - secrets not stored in config/db
- Gaps:
  - no encrypted-at-rest DB mode yet.

## Repudiation

- Threat: inability to prove signal/order lineage.
- Current mitigations:
  - trace IDs across signals/orders/audit events
  - append-only audit rows with timestamps and hash chain
  - strategy signal explainability events captured

## Information Disclosure

- Threat: credential leakage via logs/files.
- Current mitigations:
  - secrets stored in OS keychain only
  - audit records key names, never secret values
  - structured logging avoids secret payloads by design
- Gaps:
  - no external crash reporter integration in default build.

## Denial of Service

- Threat: quote flood / lag / connectivity degradation causing unsafe behavior.
- Current mitigations:
  - bounded channels and lag handling
  - quote staleness breaker can auto-halt
  - kill switch available via UI + global hotkey
  - safe mode blocks trading after crash

## Elevation of Privilege

- Threat: custom strategy script escaping sandbox or running in live mode.
- Current mitigations:
  - Rhai sandbox with strict execution limits
  - no file/network APIs exposed to script
  - custom script strategy blocked in live profile

## Supply Chain

- Controls:
  - `Cargo.lock`, `pnpm-lock.yaml`
  - CI checks (typecheck, tests, lint)
- Recommendations:
  - continuous dependency audit (`cargo audit`, `pnpm audit`)
  - SBOM generation and release artifact signing policy.

## High-Impact Safety Controls (Implemented)

- Live trading default locked behind explicit workflow
- Kill switch (UI + global hotkey)
- Pre-trade validations (allowlist, limits, bands, rates, sessions)
- Daily-loss / quote-staleness auto-halt in live mode
- Safe-mode startup after unclean crash
