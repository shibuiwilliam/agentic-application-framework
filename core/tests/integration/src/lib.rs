//! Cross-crate integration tests for AAF v0.1.
//!
//! Each test in `tests/` exercises a slice of the architecture that
//! crosses crate boundaries. The crate exposes shared helpers in
//! `lib.rs` so individual integration test files stay focused on
//! assertions.

#![forbid(unsafe_code)]
