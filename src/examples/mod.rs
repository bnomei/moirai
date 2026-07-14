//! Progressive, runnable lessons for learning Moirai through its public API.
//!
//! Start with [`tier_a`] and follow each lesson's **Next** link. The tiers build
//! from direct world construction to scheduled behavior, queries, and host-side
//! verification. Every lesson is a normal stable-Rust doctest; none requires a
//! tutorial-only runtime API.
//!
//! - [`tier_a`]: world and application foundations.
//! - [`tier_b`]: scheduled behavior and data flow.
//! - [`tier_c`]: prepared queries and controlled side effects.
//! - [`tier_d`]: host integration, constrained data, and deterministic replay.

pub mod tier_a;
pub mod tier_b;
pub mod tier_c;
pub mod tier_d;
