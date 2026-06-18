//! Per-wallet rolling-window purchase tracker.
//!
//! Used off-chain by the SDK and the indexer to refuse bundle
//! construction once a wallet has spent more than its allocation inside the
//! configured time window. The on-chain program does not track this
//! directly; the Jito bundle builder is the one that drops bundles that
//! would breach the cap.

use std::collections::HashMap;
use std::collections::VecDeque;

/// A recorded purchase entry inside the rolling window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WalletPurchase {
    /// Lamport amount spent on this buy.
    pub amount_quote_lamports: u64,
    /// Slot or unix timestamp at which the buy landed.
    pub at_slot_or_ts: u64,
}

/// Tracks total spend per wallet inside a rolling window.
#[derive(Debug, Default, Clone)]
pub struct RollingWindow {
    per_wallet: HashMap<[u8; 32], VecDeque<WalletPurchase>>,
    window_size: u64,
    per_wallet_cap: u64,
}

impl RollingWindow {
    /// Constructs a tracker with the given window size (in slots or seconds)
    /// and per-wallet spend cap.
    pub fn new(window_size: u64, per_wallet_cap: u64) -> Self {
        Self {
            per_wallet: HashMap::new(),
            window_size,
            per_wallet_cap,
        }
    }

    /// Returns the running total inside the window for a wallet.
    pub fn running_total(&self, wallet: &[u8; 32]) -> u64 {
        self.per_wallet
            .get(wallet)
            .map(|queue| queue.iter().map(|p| p.amount_quote_lamports).sum())
            .unwrap_or(0)
    }

    /// Determines whether the requested additional spend would exceed the cap.
    pub fn would_exceed(&self, wallet: &[u8; 32], requested: u64) -> bool {
        self.running_total(wallet).saturating_add(requested) > self.per_wallet_cap
    }

    /// Records a purchase and ages out any entries older than the window.
    pub fn record_purchase(&mut self, wallet: [u8; 32], amount_quote_lamports: u64, now: u64) {
        let entry = self.per_wallet.entry(wallet).or_insert_with(VecDeque::new);
        entry.push_back(WalletPurchase {
            amount_quote_lamports,
            at_slot_or_ts: now,
        });
        let cutoff = now.saturating_sub(self.window_size);
        while let Some(front) = entry.front() {
            if front.at_slot_or_ts < cutoff {
                entry.pop_front();
            } else {
                break;
            }
        }
    }

    /// Returns the configured per-wallet cap.
    pub fn cap(&self) -> u64 {
        self.per_wallet_cap
    }

    /// Returns the size of the rolling window.
    pub fn window_size(&self) -> u64 {
        self.window_size
    }

    /// Number of distinct wallets currently being tracked.
    pub fn tracked_wallet_count(&self) -> usize {
        self.per_wallet.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_total_aggregates_purchases() {
        let mut w = RollingWindow::new(60, 1_000);
        let wallet = [9u8; 32];
        w.record_purchase(wallet, 300, 10);
        w.record_purchase(wallet, 200, 20);
        assert_eq!(w.running_total(&wallet), 500);
    }

    #[test]
    fn old_purchases_are_aged_out() {
        let mut w = RollingWindow::new(30, 1_000);
        let wallet = [9u8; 32];
        w.record_purchase(wallet, 400, 10);
        w.record_purchase(wallet, 100, 100); // 90 slots later
        assert_eq!(w.running_total(&wallet), 100);
    }

    #[test]
    fn would_exceed_signals_cap_breach() {
        let mut w = RollingWindow::new(60, 500);
        let wallet = [9u8; 32];
        w.record_purchase(wallet, 400, 10);
        assert!(w.would_exceed(&wallet, 200));
        assert!(!w.would_exceed(&wallet, 99));
    }

    #[test]
    fn unknown_wallet_starts_at_zero() {
        let w = RollingWindow::new(60, 500);
        assert_eq!(w.running_total(&[0u8; 32]), 0);
    }
}
