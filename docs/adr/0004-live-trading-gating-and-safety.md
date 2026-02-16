# ADR-0004: Live Trading Gating + Kill Switch

## Status

Accepted (2026-02-15)

## Context

Live trading introduces substantial risk. The default must be paper trading, and enabling live trading must require explicit operator actions and safety controls.

## Decision

- Default to **paper** mode.
- Gate live trading behind an explicit unlock workflow:
  - read risk disclaimer (UI)
  - type a confirmation phrase exactly
  - configure non-default risk limits
  - configure a global kill-switch hotkey
  - perform a connectivity dry-run
- Implement a kill switch that:
  - halts engine
  - stops strategies
  - cancels open orders
  - blocks new order placement

## Consequences

- “Accidental live trading” is much harder.
- When uncertainty exists, the engine halts or rejects orders rather than proceeding.
- Live routing to OpenD is allowed only after unlock workflow passes.
- Safety gates remain server-side (engine enforced), not UI-only.
