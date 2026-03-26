//! `dripz-snipeguard` -- launch-window anti-snipe primitives for Dripz LBPs.
//!
//! Three independent layers compose into the on-chain `enforce_buy` check:
//!
//! 1. **Per-tx max-buy cap.** During the first `protected_slots` after the
//!    LBP opens, every buy is capped at a fraction of the remaining token
//!    vault. Default: 50 bps (0.5%) for the first ~2 minutes.
//! 2. **Commit-reveal.** Buyers submit a hash of `(amount, nonce)` in block T,
//!    then reveal `(amount, nonce)` in block T+1. The on-chain program only
//!    accepts the buy if the reveal matches the prior commit -- sniper
//!    bots cannot front-run the reveal because the original commit hides the
//!    intended amount.
//! 3. **Wallet rolling window.** The off-chain SDK / Telegram bot tracks a
//!    rolling per-wallet purchase volume and refuses to sign further bundles
//!    once a wallet exceeds its share.
//!
//! Everything is integer arithmetic so the same module compiles on-chain and
//! off-chain.

#![deny(missing_docs)]

pub mod commit_reveal;
pub mod error;
pub mod max_buy;
pub mod rolling_window;

pub use commit_reveal::{Commit, CommitReveal, Reveal};
pub use error::SnipeGuardError;
pub use max_buy::{MaxBuyConfig, MaxBuyDecision};
pub use rolling_window::{RollingWindow, WalletPurchase};

/// Composite decision returned by `enforce_buy`. Each layer can pass or veto
/// independently; the on-chain program reverts on the first veto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnforceBuyDecision {
    /// Final decision -- true if and only if every layer accepts.
    pub accepted: bool,
    /// Reason returned by the first failing layer, if any.
    pub reject_reason: Option<&'static str>,
    /// Per-tx cap (token amount) that applied at decision time.
    pub effective_cap_tokens: u128,
}

/// Combines per-tx max-buy with the rolling window check. The on-chain
/// program calls this from inside its `buy` instruction, the SDK calls it
/// while constructing the Jito bundle.
pub fn enforce_buy(
    config: &MaxBuyConfig,
    rolling: &RollingWindow,
    vault_balance: u64,
    requested_amount: u64,
    wallet: &[u8; 32],
    current_slot: u64,
) -> EnforceBuyDecision {
    let max_decision = max_buy::evaluate(config, vault_balance, requested_amount, current_slot);
    if !max_decision.accepted {
        return EnforceBuyDecision {
            accepted: false,
            reject_reason: Some("per-tx cap exceeded"),
            effective_cap_tokens: max_decision.cap_tokens,
        };
    }
    if rolling.would_exceed(wallet, requested_amount) {
        return EnforceBuyDecision {
            accepted: false,
            reject_reason: Some("wallet rolling cap exceeded"),
            effective_cap_tokens: max_decision.cap_tokens,
        };
    }
    EnforceBuyDecision {
        accepted: true,
        reject_reason: None,
        effective_cap_tokens: max_decision.cap_tokens,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enforce_buy_rejects_oversized_tx() {
        let cfg = MaxBuyConfig {
            launch_slot: 0,
            protected_slots: 300,
            max_share_bps: 50,
        };
        let mut rolling = RollingWindow::new(86_400, 1_000_000_000);
        let wallet = [1u8; 32];
        let decision = enforce_buy(&cfg, &mut rolling, 1_000_000, 999_999, &wallet, 10);
        assert!(!decision.accepted);
        assert_eq!(decision.reject_reason, Some("per-tx cap exceeded"));
    }

    #[test]
    fn enforce_buy_allows_within_caps() {
        let cfg = MaxBuyConfig {
            launch_slot: 0,
            protected_slots: 300,
            max_share_bps: 50,
        };
        let rolling = RollingWindow::new(86_400, 1_000_000_000);
        let wallet = [1u8; 32];
        let decision = enforce_buy(&cfg, &rolling, 10_000_000_000, 5_000_000, &wallet, 10);
        assert!(decision.accepted);
        assert!(decision.effective_cap_tokens >= 5_000_000);
    }

    #[test]
    fn enforce_buy_rejects_wallet_window_breach() {
        let cfg = MaxBuyConfig {
            launch_slot: 0,
            protected_slots: 0,
            max_share_bps: 10_000,
        };
        let mut rolling = RollingWindow::new(60, 100);
        let wallet = [2u8; 32];
        // Saturate the rolling window so any further buy fails.
        rolling.record_purchase(wallet, 100, 1);
        let decision = enforce_buy(&cfg, &rolling, 1_000_000, 1, &wallet, 5);
        assert!(!decision.accepted);
        assert_eq!(decision.reject_reason, Some("wallet rolling cap exceeded"));
    }
}
