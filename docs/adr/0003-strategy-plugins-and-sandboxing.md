# ADR-0003: Strategy Plugins + Sandboxed Custom Scripting

## Status

Accepted (2026-02-15)

## Context

We need:

- pluggable strategies (short-term and long-term)
- a way to run custom strategies without compromising safety
- deterministic behavior for backtests and reproducibility

## Decision

- Built-in strategies are registered by ID and metadata (compile-time registry).
- Provide a **sandboxed script strategy** using Rhai:
  - no IO/network APIs exposed
  - strict execution limits (ops/call depth/expr depth/string size)
  - disabled in live profile

## Consequences

- MVP supports configurable strategy modules and a safe custom scripting path.
- Dynamic native plugin loading and plugin signing are deferred to later phases.

