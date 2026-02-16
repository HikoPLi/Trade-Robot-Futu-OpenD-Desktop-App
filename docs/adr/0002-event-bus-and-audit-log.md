# ADR-0002: Event-Driven Engine + SQLite Audit Hash Chain

## Status

Accepted (2026-02-15)

## Context

We need:

- a clean separation between market data, strategies, risk, execution, and portfolio
- observability and post-mortem capability for trading decisions
- a replay-able record of what happened (including strategy reasons)

## Decision

- Use an internal typed event stream (`EngineEvent`) for engine -> UI updates.
- Persist an **append-only** audit log in SQLite with a **hash chain**:
  - each row stores `prev_hash` and `hash = SHA256(prev_hash || ts || type || trace_id || payload_json)`

## Consequences

- Detectable tampering: any modification breaks the chain.
- Clear operational visibility: audit events + trace IDs correlate strategy signals to orders and fills.
- Replay mode can be built by consuming exported audit JSONL (roadmap).

