# ADR-0001: Tauri v2 + Rust Engine + React UI

## Status

Accepted (2026-02-15)

## Context

We need a production-grade, cross-platform desktop trading app (Windows/macOS/Linux) with:

- responsive UI and background engine execution
- safety-first controls (paper-first, kill switch, audit logging)
- reproducible builds and CI packaging

## Decision

Use:

- **Tauri v2** for the desktop shell and IPC boundary
- **Rust** for the core trading engine and safety-critical logic
- **React + TypeScript** for the UI

## Consequences

Pros:

- small runtime vs Electron; good security posture with allowlisted commands/capabilities
- Rust is a strong fit for deterministic backtests, risk controls, and safe concurrency
- single mono-repo with typed contracts shared between Rust and TS

Cons:

- native build dependencies (especially Linux webkit/gtk) are required in CI
- dynamic plugin loading is harder than in a pure JS environment (handled via scripted strategies + compile-time registry in MVP)

