//! Kani harnesses over the real `pool-core` (shared-pool strategy, Kani layer).
//!
//! `cargo test --release` runs every harness body in plain mode (random, **not
//! a proof**). `cargo kani` runs the same bodies symbolically (`proofs.rs`):
//! that is the bounded proof.
//!
//! Verified: shared-pool-controls v1.1.0. Pin: difflabai/shared-pool-controls
//! `57b3f02d18e85149db3c1035431368c4e07a4d8a` (the merge commit of PR #4; its
//! tree is the PR head `6fcbd56`), `crates/core/src/lib.rs` sha256
//! `c3bb825b91f7c277dee8651beafd45955f7cf8935c858f928b6a1b62d607ba48`.

#![cfg_attr(kani, feature(allocator_api))]

#[macro_use]
pub mod src;
pub mod checks;
pub mod gen;
pub mod spec;
/// The TLA+ transcription for REF-1..3 (`tla/SharedPool.tla`).
pub mod tla;

#[cfg(kani)]
pub mod model;
#[cfg(kani)]
mod proofs;

#[cfg(all(test, not(kani)))]
mod plain;
#[cfg(all(test, not(kani)))]
mod conform;
