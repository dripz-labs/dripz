//! Commit-reveal helper used during the snipe-protected window.
//!
//! Buyers hash their `(amount, nonce, wallet)` triple and submit the digest
//! in slot T. In slot `T + reveal_delay`, they reveal the plaintext; the
//! on-chain program recomputes the hash and rejects the buy if it does not
//! match. Because the digest hides both the amount and the nonce, MEV bots
//! cannot decode and front-run the trade during the commit interval.

use crate::error::SnipeGuardError;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// A commit registered for a wallet during the protected window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    /// 32-byte SHA-256 digest of `wallet || amount || nonce`.
    pub digest: [u8; 32],
    /// Solana slot in which the commit landed.
    pub slot: u64,
}

/// The payload a buyer reveals after the commit delay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reveal {
    /// The wallet that owns the commit.
    pub wallet: [u8; 32],
    /// The intended buy amount in quote lamports.
    pub amount: u64,
    /// Anti-replay nonce (any 16 random bytes).
    pub nonce: [u8; 16],
}

/// Minimum and maximum slot delay between commit and reveal. The on-chain
/// program enforces these bounds when accepting a reveal.
#[derive(Debug, Clone, Copy)]
pub struct CommitWindow {
    /// Minimum number of slots that must pass before a reveal is accepted.
    pub min_delay_slots: u64,
    /// Maximum number of slots after the commit during which a reveal is
    /// still accepted.
    pub max_delay_slots: u64,
}

impl Default for CommitWindow {
    fn default() -> Self {
        Self {
            min_delay_slots: 1,
            max_delay_slots: 20,
        }
    }
}

/// In-memory commit registry. The on-chain version stores commits inside a
/// dedicated PDA per LBP; this type is what the off-chain simulator uses.
#[derive(Debug, Default, Clone)]
pub struct CommitReveal {
    commits: HashMap<[u8; 32], Commit>,
    window: CommitWindow,
}

impl CommitReveal {
    /// Constructs a new registry with the given window bounds.
    pub fn new(window: CommitWindow) -> Self {
        Self {
            commits: HashMap::new(),
            window,
        }
    }

    /// Computes the canonical digest a buyer must submit.
    pub fn digest(wallet: &[u8; 32], amount: u64, nonce: &[u8; 16]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(wallet);
        hasher.update(amount.to_le_bytes());
        hasher.update(nonce);
        let result = hasher.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(&result);
        out
    }

    /// Registers a commit. Returns an error if a commit is already pending
    /// for this wallet in the current slot.
    pub fn commit(
        &mut self,
        wallet: [u8; 32],
        digest: [u8; 32],
        slot: u64,
    ) -> Result<(), SnipeGuardError> {
        if let Some(existing) = self.commits.get(&wallet) {
            if existing.slot == slot {
                return Err(SnipeGuardError::DuplicateCommit);
            }
        }
        self.commits.insert(wallet, Commit { digest, slot });
        Ok(())
    }

    /// Verifies a reveal and consumes the prior commit on success.
    pub fn reveal(&mut self, reveal: &Reveal, current_slot: u64) -> Result<(), SnipeGuardError> {
        let stored = self
            .commits
            .get(&reveal.wallet)
            .cloned()
            .ok_or(SnipeGuardError::CommitNotFound)?;
        let elapsed = current_slot.saturating_sub(stored.slot);
        if elapsed < self.window.min_delay_slots || elapsed > self.window.max_delay_slots {
            return Err(SnipeGuardError::OutOfWindow);
        }
        let recomputed = Self::digest(&reveal.wallet, reveal.amount, &reveal.nonce);
        if recomputed != stored.digest {
            return Err(SnipeGuardError::HashMismatch);
        }
        self.commits.remove(&reveal.wallet);
        Ok(())
    }

    /// Returns the configured window bounds.
    pub fn window(&self) -> CommitWindow {
        self.window
    }

    /// Returns the number of commits currently pending.
    pub fn pending_commit_count(&self) -> usize {
        self.commits.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> ([u8; 32], u64, [u8; 16]) {
        ([7u8; 32], 1_500_000_000, [3u8; 16])
    }

    #[test]
    fn commit_then_reveal_succeeds() {
        let (wallet, amount, nonce) = fixture();
        let digest = CommitReveal::digest(&wallet, amount, &nonce);
        let mut registry = CommitReveal::new(CommitWindow::default());
        registry.commit(wallet, digest, 100).unwrap();
        let reveal = Reveal {
            wallet,
            amount,
            nonce,
        };
        registry.reveal(&reveal, 102).unwrap();
        assert_eq!(registry.pending_commit_count(), 0);
    }

    #[test]
    fn reveal_with_wrong_amount_is_rejected() {
        let (wallet, amount, nonce) = fixture();
        let digest = CommitReveal::digest(&wallet, amount, &nonce);
        let mut registry = CommitReveal::new(CommitWindow::default());
        registry.commit(wallet, digest, 100).unwrap();
        let reveal = Reveal {
            wallet,
            amount: amount + 1,
            nonce,
        };
        let err = registry.reveal(&reveal, 102).unwrap_err();
        assert_eq!(err, SnipeGuardError::HashMismatch);
    }

    #[test]
    fn reveal_outside_window_is_rejected_early() {
        let (wallet, amount, nonce) = fixture();
        let digest = CommitReveal::digest(&wallet, amount, &nonce);
        let mut registry = CommitReveal::new(CommitWindow {
            min_delay_slots: 2,
            max_delay_slots: 5,
        });
        registry.commit(wallet, digest, 100).unwrap();
        let reveal = Reveal {
            wallet,
            amount,
            nonce,
        };
        let err = registry.reveal(&reveal, 101).unwrap_err();
        assert_eq!(err, SnipeGuardError::OutOfWindow);
    }

    #[test]
    fn duplicate_commit_in_same_slot_is_rejected() {
        let mut registry = CommitReveal::new(CommitWindow::default());
        registry.commit([1u8; 32], [0u8; 32], 100).unwrap();
        let err = registry.commit([1u8; 32], [9u8; 32], 100).unwrap_err();
        assert_eq!(err, SnipeGuardError::DuplicateCommit);
    }
}
