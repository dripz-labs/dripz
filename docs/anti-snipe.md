# Anti-Snipe

The protected window after an LBP opens is when sniper bots cause the most damage to fair price discovery. Dripz layers three independent defences during this window. Each one is implemented inside `dripz-snipeguard`; the on-chain Anchor program calls them in order.

## Layer 1 -- Per-tx max-buy cap

```rust
use dripz_snipeguard::{evaluate, MaxBuyConfig};

let config = MaxBuyConfig {
    launch_slot: 1_000,
    protected_slots: 300,      // ~2 minutes on Solana
    max_share_bps: 50,         // 0.5% of vault per tx
};
let decision = evaluate(&config, vault_balance, requested_amount, current_slot);
assert!(decision.accepted);
```

Inside the first 300 slots, every buy is capped at 0.5% of the remaining token vault. After the window closes, the cap reverts to `u128::MAX` and the layer becomes a no-op.

## Layer 2 -- Commit-reveal

Buyers register a SHA-256 hash of `(wallet, amount, nonce)` in slot `T`, then reveal `(amount, nonce)` in slot `T + 1`. The on-chain program rejects the reveal if it does not match the prior commit. Because the digest hides both the amount and the nonce, MEV bots cannot decode and front-run the intended trade during the commit interval.

```rust
use dripz_snipeguard::{CommitReveal, CommitWindow, Reveal};

let mut registry = CommitReveal::new(CommitWindow::default());
let digest = CommitReveal::digest(&wallet, amount, &nonce);
registry.commit(wallet, digest, slot_t)?;

// One slot later:
registry.reveal(&Reveal { wallet, amount, nonce }, slot_t + 1)?;
```

The default window is `[1, 20]` slots. A reveal that arrives outside the window is rejected with `SnipeGuardError::OutOfWindow`.

## Layer 3 -- Wallet rolling window

Off-chain (SDK + Telegram bot), the rolling-window guard tracks each wallet's cumulative spend across a moving time window. Once a wallet hits its allocation the bundle builder simply drops further buy bundles for that wallet.

```rust
use dripz_snipeguard::RollingWindow;

let mut rolling = RollingWindow::new(86_400, 1_000_000_000); // 24h, 1k SOL cap
rolling.record_purchase(wallet, 250_000_000, now);
assert!(rolling.would_exceed(&wallet, 800_000_000));
```

## End-to-end check

`dripz_snipeguard::enforce_buy` composes all three layers and returns a single `EnforceBuyDecision`. The on-chain program calls it as the first line of its `buy` instruction; the off-chain SDK calls it from the bundle builder.

## Backtest results

Running the synthetic Pump.fun snipe profile (70% of volume in the first 5% of the window) against an LBP with all three layers enabled shows the cap rejecting roughly half of the heavy-window attempts before they ever reach the pool. See `dripz-cli-demo/src/backtest.rs` for the harness and `dripz-snipeguard/src/lib.rs` tests for the unit-level invariants.
