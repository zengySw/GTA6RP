# ADR-0005: Filesystem free space query

- Status: Accepted
- Date: 2026-11-23

## Context

The launcher needs to check available free disk space before installation or download
operations (Phase 4.3). Rust's standard library does not provide a cross-platform
API for querying filesystem free space. On Unix, this requires calling `statvfs()`
from libc, which is an `extern "C"` function and therefore requires `unsafe`. On
Windows, `GetDiskFreeSpaceExW` from the Windows API also requires `unsafe`.

## Decision

Use `libc::statvfs` on Unix and `GetDiskFreeSpaceExW` on Windows to query free
disk space. Both are wrapped in `unsafe` blocks that are carefully audited:

- Unix: `statvfs` is called with a valid `CString` path and a zero-initialized
  `statvfs` struct. Return value is checked for errors.
- Windows: `GetDiskFreeSpaceExW` is called with a null-terminated UTF-16 path
  and valid output pointers. Return `BOOL` is checked for success.

## Alternatives

- Add the `nix` crate as a dependency: `nix::sys::statvfs::statvfs` provides a
  safe wrapper, but adds a heavy dependency for a single function.
- Use `std::process::Command` to run `df` (Unix) or `fsutil` (Windows): safe but
  slow, fragile output parsing, and depends on external utilities being present.
- Add `std::fs::space` equivalent: not available in Rust standard library.

## Consequences and risks

- `unsafe` code is isolated to two small, well-bounded functions in
  `crates/installation/src/safe_root.rs`.
- Each `unsafe` block is documented with comments explaining why it is sound.
- The functions return `Result` and never panic on normal errors.
- If the FFI call fails, the error is propagated as `InstallError::Io`.

## Review needed

This ADR documents the reviewed need for `unsafe`. The unsafe blocks should be
reviewed alongside any changes to `crates/installation/src/safe_root.rs`.
