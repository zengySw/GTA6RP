# ADR-0003: Local secret storage

- Status: Proposed, pending implementation review
- Date: 2026-11-21

## Context

The desktop application may later need to persist refresh credentials. Plain configuration files and SQLite are not acceptable locations for reusable secrets.

## Decision

On Windows, use Windows Credential Manager or DPAPI through a maintained, reviewed Rust implementation. Select the concrete crate only when authentication and local persistence are implemented. Store only non-secret preferences and operation metadata in SQLite.

## Alternatives

- Plaintext files or SQLite are easy to back up but expose credentials to any reader of those files.
- A custom cryptographic wrapper would create key-management risk without improving the platform boundary.

## Consequences and risks

- Secret persistence is not implemented by the Phase 0 launcher.
- The chosen implementation must be checked for maintenance, platform behavior, error handling, and safe migration before credentials are stored.
- Passwords, refresh tokens, private signing keys, and backend secrets must never enter logs or the client binary.

