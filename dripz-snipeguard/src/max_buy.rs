//! Per-tx maximum buy enforcement during the snipe-protected window.

/// Configuration for the per-tx cap.
#[derive(Debug, Clone, Copy)]
pub struct MaxBuyConfig {
    /// Slot in which the LBP opened. Used to compute the elapsed window.
    pub launch_slot: u64,
    /// Number of slots after `launch_slot` during which the cap applies.
    pub protected_slots: u64,
    /// Cap, expressed as basis points (1 bp = 0.01%) of the vault balance.
    pub max_share_bps: u16,
}

/// Output of a single cap evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaxBuyDecision {
    /// Whether the buy is admitted.
    pub accepted: bool,
    /// Effective per-tx cap in token units (vault-denominated).
    pub cap_tokens: u128,
    /// Slots remaining in the protected window. 0 once the window has closed.
    pub slots_remaining: u64,
}

/// Evaluates a single buy against the cap.
pub fn evaluate(
    config: &MaxBuyConfig,
    vault_balance: u64,
    requested_amount: u64,
    current_slot: u64,
) -> MaxBuyDecision {
    let elapsed = current_slot.saturating_sub(config.launch_slot);
    if elapsed >= config.protected_slots {
        return MaxBuyDecision {
            accepted: true,
            cap_tokens: u128::MAX,
            slots_remaining: 0,
        };
    }
    let slots_remaining = config.protected_slots - elapsed;
    let cap = (vault_balance as u128 * config.max_share_bps as u128) / 10_000u128;
    MaxBuyDecision {
        accepted: (requested_amount as u128) <= cap,
        cap_tokens: cap,
        slots_remaining,
    }
}

/// Convenience helper: returns the effective cap in tokens without making a
/// decision. Used by Telegram bot embeds and the SDK's "preview" call.
pub fn current_cap_tokens(config: &MaxBuyConfig, vault_balance: u64, current_slot: u64) -> u128 {
    let elapsed = current_slot.saturating_sub(config.launch_slot);
    if elapsed >= config.protected_slots {
        return u128::MAX;
    }
    (vault_balance as u128 * config.max_share_bps as u128) / 10_000u128
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> MaxBuyConfig {
        MaxBuyConfig {
            launch_slot: 1_000,
            protected_slots: 300,
            max_share_bps: 50, // 0.5%
        }
    }

    #[test]
    fn buy_at_cap_is_admitted() {
        let result = evaluate(&cfg(), 10_000_000, 50_000, 1_100);
        assert!(result.accepted);
        assert_eq!(result.cap_tokens, 50_000);
    }

    #[test]
    fn buy_above_cap_is_rejected() {
        let result = evaluate(&cfg(), 10_000_000, 50_001, 1_100);
        assert!(!result.accepted);
    }

    #[test]
    fn cap_is_disabled_after_window() {
        let result = evaluate(&cfg(), 10_000_000, u64::MAX, 5_000);
        assert!(result.accepted);
        assert_eq!(result.cap_tokens, u128::MAX);
        assert_eq!(result.slots_remaining, 0);
    }

    #[test]
    fn current_cap_helper_matches_decision() {
        let cap = current_cap_tokens(&cfg(), 10_000_000, 1_050);
        assert_eq!(cap, 50_000);
    }
}
