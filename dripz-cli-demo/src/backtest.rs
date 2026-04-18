//! Headless backtest harness used by `dripz backtest` and the verifier tests.
//!
//! The synthetic demand profile mimics the Pump.fun snipe pattern: 70% of
//! quote volume in the first 5% of the window, the remainder spread evenly
//! across the rest of the LBP. We then report the realised slippage,
//! sniped share, and the final price under each curve.

use anyhow::Result;
use dripz_curves::CurveParams;
use dripz_engine::{PoolConfig, PoolState};
use serde::Serialize;

/// Summary metrics returned for one curve.
#[derive(Debug, Clone, Serialize)]
pub struct BacktestSummary {
    /// Display name of the curve.
    pub curve: String,
    /// Total realised quote volume.
    pub total_quote_raised: u64,
    /// Realised final spot price (quote per token), in micro-units.
    pub final_spot_price_micro: u128,
    /// Average per-buy slippage, in basis points.
    pub average_slippage_bps: u64,
    /// Fraction of total volume bought inside the first 5% of the window,
    /// in basis points. Lower == better snipe resistance.
    pub first_5pct_volume_share_bps: u64,
    /// Number of buys executed.
    pub buy_count: usize,
}

const TOTAL_RAISE_LAMPORTS: u64 = 1_000_000_000_000; // 1_000 SOL-equivalent quote
const TOKEN_VAULT_LAMPORTS: u64 = 100_000_000_000_000;

/// Runs the backtest and returns the summary.
pub fn run(params: &CurveParams) -> Result<BacktestSummary> {
    params
        .validate()
        .map_err(|e| anyhow::anyhow!("curve params invalid: {e}"))?;
    let curve = dripz_curves::AnyCurve::from_params(params)?;
    let config = PoolConfig::new("DRIPZ", "USDC", 30, curve.clone())?;
    let mut state = PoolState {
        balance_token_lamports: TOKEN_VAULT_LAMPORTS,
        balance_quote_lamports: TOTAL_RAISE_LAMPORTS / 100,
        weight_token_micro: params.start_weight_token_micro,
        weight_quote_micro: 1_000_000 - params.start_weight_token_micro,
        elapsed_secs: 0,
    };
    state.refresh_weights(&config)?;

    let mut samples = Vec::new();
    let heavy_window_secs = params.duration_secs * 5 / 100;
    let heavy_volume = (TOTAL_RAISE_LAMPORTS as u128 * 70 / 100) as u64;
    let light_volume = TOTAL_RAISE_LAMPORTS - heavy_volume;

    let heavy_chunks = 30u64;
    let mut first_5pct_volume = 0u64;
    for i in 0..heavy_chunks {
        let elapsed = (heavy_window_secs * i) / heavy_chunks;
        let buy_amount = heavy_volume / heavy_chunks;
        state.elapsed_secs = elapsed;
        state.refresh_weights(&config)?;
        let quote = state.quote_buy(&config, buy_amount)?;
        let fee_net = buy_amount.saturating_sub(quote.fee_paid);
        state.apply_buy(fee_net, quote.amount_out);
        first_5pct_volume = first_5pct_volume.saturating_add(buy_amount);
        samples.push(quote);
    }

    let light_chunks = 60u64;
    for i in 0..light_chunks {
        let elapsed =
            heavy_window_secs + ((params.duration_secs - heavy_window_secs) * i) / light_chunks;
        let buy_amount = light_volume / light_chunks;
        state.elapsed_secs = elapsed.min(params.duration_secs);
        state.refresh_weights(&config)?;
        let quote = state.quote_buy(&config, buy_amount)?;
        let fee_net = buy_amount.saturating_sub(quote.fee_paid);
        state.apply_buy(fee_net, quote.amount_out);
        samples.push(quote);
    }

    let final_spot = state.spot_price_micro()?;
    let buy_count = samples.len();
    let slippage_sum: u128 = samples
        .iter()
        .map(|s| {
            let before = s.spot_price_before_micro.max(1);
            let after = s.spot_price_after_micro.max(1);
            let abs_change = if after > before {
                after - before
            } else {
                before - after
            };
            (abs_change * 10_000) / before
        })
        .sum();
    let average_slippage_bps = if buy_count == 0 {
        0
    } else {
        (slippage_sum / buy_count as u128) as u64
    };
    let total_raised = TOTAL_RAISE_LAMPORTS;
    let first_5pct_share_bps = ((first_5pct_volume as u128 * 10_000) / total_raised as u128) as u64;

    Ok(BacktestSummary {
        curve: format!("{:?}", curve.kind()),
        total_quote_raised: total_raised,
        final_spot_price_micro: final_spot,
        average_slippage_bps,
        first_5pct_volume_share_bps: first_5pct_share_bps,
        buy_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_and_exponential_runs_complete_with_full_volume() {
        let linear = run(&CurveParams::linear(990_000, 500_000, 604_800)).unwrap();
        let exponential = run(&CurveParams::exponential(
            990_000, 500_000, 604_800, 4_000_000,
        ))
        .unwrap();
        assert!(linear.buy_count > 0);
        assert!(exponential.buy_count > 0);
        // The synthetic Pump.fun-style profile front-loads exactly 70% of
        // volume into the first 5% of the window, so both runs land near
        // the same share.
        assert!(linear.first_5pct_volume_share_bps >= 6_500);
        assert!(exponential.first_5pct_volume_share_bps >= 6_500);
    }

    #[test]
    fn summary_buy_count_matches_synthetic_profile() {
        let summary = run(&CurveParams::linear(990_000, 500_000, 604_800)).unwrap();
        assert_eq!(summary.buy_count, 30 + 60);
    }
}
