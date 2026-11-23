# ADR-0001: Desktop UI framework

- Status: Proposed, pending Phase 0 spike
- Date: 2026-11-21

## Context

The desktop client should keep its application logic in Rust and avoid a browser-based UI stack. The UI must support a media-rich dark launcher layout on Windows 10/11 x64.

## Decision

Use Slint for the Phase 0 prototype. Pin the `slint` and `slint-build` dependencies to the same version. Compile declarative `.slint` files at build time from `build.rs`.

## Alternatives

- Tauri + React: mature desktop distribution path, but puts UI and much application-facing code in a WebView/JavaScript stack.
- Other native Rust UI frameworks: keep Rust-first but require a separate visual and ecosystem evaluation.

## Consequences and risks

- The prototype stays Rust-first and has no embedded browser.
- Slint is not approved for production until Windows builds, visual quality, large-image handling, DPI (100/125/150/200%), resize, focus, startup, RAM, idle CPU, and license terms are evaluated and recorded.
- If a critical requirement fails, record a replacement decision in a new ADR rather than silently changing the framework.

## Validation record

Pending Windows x64 evaluation. The current repository environment has not produced Windows measurements or a license review.

