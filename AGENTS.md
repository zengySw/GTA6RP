# Project instructions

## Scope

- Follow `PLAN.md` as the product and architecture roadmap.
- The first client is a Windows 10/11 x64 Rust desktop application using Slint.
- Until an official PC release and a lawful integration path are validated, use synthetic fixtures only. Do not claim GTA VI support or include game binaries.
- Do not implement game modification, code injection, anti-cheat bypass, or DRM bypass.

## Architecture

- Keep UI callbacks free of filesystem, database, network, and hashing work.
- Keep domain and application logic independent of Slint and infrastructure.
- Use background tasks for long operations and bounded progress updates.
- Do not execute commands received from the backend or build shell command strings.
- Keep secrets out of source, configuration examples, logs, and client binaries.

## Rust quality

- Format with `cargo fmt --all` and keep Clippy warnings resolved.
- Avoid `unwrap()` and `expect()` on ordinary runtime paths.
- Keep unsafe code forbidden unless an ADR documents a reviewed need.
- Add focused tests alongside behavior as implementation proceeds; run the relevant checks before closing a phase.

