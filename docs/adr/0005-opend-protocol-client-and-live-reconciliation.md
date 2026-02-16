# ADR-0005: OpenD Protocol Client + Live Reconciliation

## Status

Accepted (2026-02-16)

## Context

The system must support real OpenD connectivity for:

- quote subscriptions
- account discovery
- order placement/cancel
- order/position/funds reconciliation

while preserving paper-first safety behavior and deterministic fallback behavior.

## Decision

- Implement a native Rust OpenD protocol client in `packages/connectors/futu`:
  - protobuf request/response encoding (`prost`)
  - OpenD framing header packing/unpacking + SHA1 verification
  - push + request/response handling over TCP
- Add connector APIs for:
  - `connect` / `close` / `test_connection`
  - `qot_subscribe_basic`, `qot_get_basic_qot`, `qot_get_kl`
  - `trd_get_acc_list`, `trd_unlock_trade`
  - `trd_place_order`, `trd_cancel_order`
  - `trd_get_orders`, `trd_get_positions`, `trd_get_funds`
- In core engine:
  - live profile disables mock feed and enables OpenD
  - live order path uses idempotency by `client_order_id`
  - periodic reconciliation loop updates orders/positions/funds
  - quote staleness and daily-loss breakers auto-trigger kill switch

## Security Decision

- Do not expose misleading TLS toggle for OpenD protocol path.
- For remote usage, require SSH/VPN tunnel guidance in docs/runbook.

## Consequences

### Positive

- Real OpenD order/account lifecycle support is available.
- Live safety posture remains strict and centralized in engine logic.
- Connector can be integration-tested with a mock OpenD TCP server.

### Tradeoffs

- No native TLS at protocol layer in this implementation path.
- Live order placement intentionally avoids aggressive auto-retry to reduce duplicate-order risk.
