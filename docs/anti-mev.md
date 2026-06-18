# Anti-MEV

Dripz uses Jito bundles with the DontFront tip pattern so that buys land atomically with their accompanying anti-snipe and anti-sandwich instructions.

## Bundle layout

Every Dripz buy is wrapped in a three-instruction bundle:

```
bundle = [
    dontfront_ix,       // burns a small SOL tip to a DontFront PDA
    swap_ix,            // calls the dripz-lbp program
    tip_ix              // pays the Jito searcher
]
```

The DontFront instruction puts a small SOL tip into a PDA that is read by the same `swap_ix`. If a searcher tries to split or reorder the bundle, the swap reverts because the PDA check fails. This is the same defense used by the Jito-DontFront reference design.

## Tip computation

The SDK computes the searcher tip from a rolling estimate of the per-CU price observed by the Jito block engine over the last 1 second:

```
tip_lamports = clamp(
    swap_amount_quote * tip_share_bps / 10_000,
    min_tip_lamports,
    max_tip_lamports,
)
```

`tip_share_bps` defaults to `2` (0.02% of the swap). `min_tip` and `max_tip` are operator-configured.

## Step-curve sweep

When the next slot crosses a Step curve boundary, the on-chain program inserts a "sweep" transaction inside the same bundle. The sweep buys at the pre-transition weight and sells at the post-transition weight so that the realised price at the boundary cannot be sniped by another bundle in the same block.

## Backtest harness

`dripz-cli-demo backtest` runs every curve through a synthetic demand profile and reports the first-5%-window volume share. We do not yet ship the Jito-vs-no-Jito benchmark in this repository because it requires a live mainnet RPC. The production indexer publishes those numbers daily on https://dripz.fi.

## References

- [Jito DontFront design](https://docs.jito.network/)
- [Solana Foundation MEV report](https://solana.com/news/jito)
- [Streamflow vesting integration](https://streamflow.finance/)
