//! Pool simulator. Walks a `PoolState` through equally-spaced time steps with
//! a constant per-step demand and records the spot price + weight at every
//! sample point.

use anyhow::Result;
use dripz_engine::{EngineError, PoolConfig, PoolState};
use serde::Serialize;

/// One sample point emitted by `run`.
#[derive(Debug, Clone, Serialize)]
pub struct SimulationSample {
    /// Elapsed seconds since LBP open.
    pub elapsed_secs: u64,
    /// Token weight in micro-units at this sample.
    pub weight_token_micro: u64,
    /// Realised spot price (quote per token) in micro-units.
    pub spot_price_micro: u128,
    /// Remaining token balance in lamports.
    pub balance_token_lamports: u64,
    /// Remaining quote balance in lamports.
    pub balance_quote_lamports: u64,
}

/// Runs the simulator. `points` controls how many samples are produced.
pub fn run(
    config: &PoolConfig,
    state: &mut PoolState,
    duration_secs: u64,
    points: usize,
) -> Result<Vec<SimulationSample>, EngineError> {
    let points = points.max(2);
    let mut samples = Vec::with_capacity(points);
    let step_secs = duration_secs / (points as u64 - 1);
    let demand_per_step = (state.balance_quote_lamports as u128
        + state.balance_token_lamports as u128)
        / (4 * points as u128);
    let demand_per_step = demand_per_step as u64;
    for i in 0..points {
        let elapsed = (i as u64) * step_secs;
        state.elapsed_secs = elapsed.min(duration_secs);
        state.refresh_weights(config)?;
        let _ = state.spot_price_micro()?; // ensures non-degenerate state
        samples.push(SimulationSample {
            elapsed_secs: state.elapsed_secs,
            weight_token_micro: state.weight_token_micro,
            spot_price_micro: state.spot_price_micro()?,
            balance_token_lamports: state.balance_token_lamports,
            balance_quote_lamports: state.balance_quote_lamports,
        });
        if i + 1 < points && demand_per_step > 0 {
            let quote = state.quote_buy(config, demand_per_step)?;
            let fee_net = demand_per_step.saturating_sub(quote.fee_paid);
            state.apply_buy(fee_net, quote.amount_out);
        }
    }
    Ok(samples)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dripz_curves::{AnyCurve, CurveParams};

    #[test]
    fn run_produces_requested_sample_count() {
        let curve = AnyCurve::from_params(&CurveParams::linear(990_000, 500_000, 1_000)).unwrap();
        let config = PoolConfig::new("DRIPZ", "USDC", 30, curve).unwrap();
        let mut state = PoolState {
            balance_token_lamports: 5_000_000_000,
            balance_quote_lamports: 500_000_000,
            weight_token_micro: 990_000,
            weight_quote_micro: 10_000,
            elapsed_secs: 0,
        };
        state.refresh_weights(&config).unwrap();
        let samples = run(&config, &mut state, 1_000, 21).unwrap();
        assert_eq!(samples.len(), 21);
        assert!(
            samples.first().unwrap().weight_token_micro
                > samples.last().unwrap().weight_token_micro
        );
    }
}
