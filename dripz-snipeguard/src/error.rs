//! Error type shared by every snipe-guard primitive.

use thiserror::Error;

/// Errors emitted by the commit-reveal, max-buy, and rolling-window modules.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SnipeGuardError {
    /// A commit was already registered for the given wallet in the current
    /// slot. Re-using the same `(wallet, slot)` tuple is forbidden so that a
    /// front-runner cannot grind nonces inside the commit window.
    #[error("duplicate commit for wallet in same slot")]
    DuplicateCommit,

    /// The commit referenced by a reveal was not found. Either the commit
    /// expired or it was never registered.
    #[error("commit not found for wallet")]
    CommitNotFound,

    /// The reveal hash does not match the prior commit hash.
    #[error("reveal hash mismatch")]
    HashMismatch,

    /// The reveal arrived before the minimum delay or after the maximum delay.
    #[error("reveal outside the allowed slot window")]
    OutOfWindow,

    /// The requested buy is larger than the per-tx cap.
    #[error("buy size {requested} exceeds the cap {cap}")]
    BuyTooLarge {
        /// Amount the buyer requested.
        requested: u64,
        /// The currently active cap.
        cap: u64,
    },

    /// Arithmetic overflow while computing a cap.
    #[error("arithmetic overflow in snipe guard math")]
    Overflow,
}
