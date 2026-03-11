//! Convenience pool-state helpers used by simulators, back-tests, and the
//! reference CLI. The on-chain Anchor program holds the same fields inside
//! its `Pool` PDA but with different serialization concerns.

use crate::error::EngineError;
use crate::math::{compute_buy_out, compute_sell_out, spot_price_micro};
use dripz_curves::{AnyCurve, CurveKind};

const MAX_SWAP_FEE_BPS: u16 = 1_000; // 10%
const WEIGHT_PRECISION_MICRO: u64 = 1_000_000;

/// Static pool configuration.
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Symbol of the asset being launched.
    pub token_symbol: String,
    /// Symbol of the asset accepted as quote.
    pub quote_symbol: String,
    /// Swap fee in basis points (1 bp = 0.01%).
    pub swap_fee_bps: u16,
    /// The curve that drives the weight schedule.
    pub curve: AnyCurve,
}

impl PoolConfig {
    /// Constructs a pool config, validating the fee.
    pub fn new(
        token_symbol: impl Into<String>,
        quote_symbol: impl Into<String>,
        swap_fee_bps: u16,
        curve: AnyCurve,
    ) -> Result<Self, EngineError> {
        if swap_fee_bps > MAX_SWAP_FEE_BPS {
            return Err(EngineError::SwapFeeTooHigh {
                got_bps: swap_fee_bps,
                max_bps: MAX_SWAP_FEE_BPS,
            });
        }
        Ok(Self {
            token_symbol: token_symbol.into(),
            quote_symbol: quote_symbol.into(),
            swap_fee_bps,
            curve,
        })
    }

    /// Reports the curve discriminant.
    pub fn curve_kind(&self) -> CurveKind {
        self.curve.kind()
    }
}

/// Mutable pool state -- balances and current weights.
#[derive(Debug, Clone)]
pub struct PoolState {
    /// Current quote-side balance (lamports).
    pub balance_quote_lamports: u64,
    /// Current token-side balance (lamports).
    pub balance_token_lamports: u64,
    /// Token weight in micro-units (1e6 = 100%).
    pub weight_token_micro: u64,
    /// Quote weight in micro-units. Always `1e6 - weight_token_micro`.
    pub weight_quote_micro: u64,
    /// Seconds elapsed since the LBP opened.
    pub elapsed_secs: u64,
}

impl PoolState {
    /// Re-evaluates the curve at the current `elapsed_secs`. Call this before
    /// every quote so the weights stay in sync with the configured schedule.
    pub fn refresh_weights(&mut self, config: &PoolConfig) -> Result<(), EngineError> {
        let weight_token = config.curve.weight_token_micro(self.elapsed_secs)?;
        self.weight_token_micro = weight_token;
        self.weight_quote_micro = WEIGHT_PRECISION_MICRO.saturating_sub(weight_token);
        Ok(())
    }

    /// Returns the spot price (quote per token) in micro-units.
    pub fn spot_price_micro(&self) -> Result<u128, EngineError> {
        spot_price_micro(
            self.balance_quote_lamports,
            self.balance_token_lamports,
            self.weight_quote_micro,
            self.weight_token_micro,
        )
    }
}

/// Output of a buy/sell quote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuoteResult {
    /// Token amount the trader receives after fees.
    pub amount_out: u64,
    /// Fee paid in input units.
    pub fee_paid: u64,
    /// Spot price before the trade.
    pub spot_price_before_micro: u128,
    /// Spot price after the trade.
    pub spot_price_after_micro: u128,
}

impl PoolState {
    /// Quotes a buy without mutating the state.
    pub fn quote_buy(
        &self,
        config: &PoolConfig,
        amount_in_quote_lamports: u64,
    ) -> Result<QuoteResult, EngineError> {
        let fee_micro = config.swap_fee_bps as u128;
        let amount_net =
            ((amount_in_quote_lamports as u128) * (10_000 - fee_micro) / 10_000) as u64;
        let fee_paid = amount_in_quote_lamports.saturating_sub(amount_net);
        let amount_out = compute_buy_out(
            self.balance_quote_lamports,
            self.balance_token_lamports,
            self.weight_quote_micro,
            self.weight_token_micro,
            amount_net,
        )?;
        let new_balance_quote = self.balance_quote_lamports.saturating_add(amount_net);
        let new_balance_token = self.balance_token_lamports.saturating_sub(amount_out);
        let spot_before = self.spot_price_micro()?;
        let spot_after = spot_price_micro(
            new_balance_quote,
            new_balance_token,
            self.weight_quote_micro,
            self.weight_token_micro,
        )?;
        Ok(QuoteResult {
            amount_out,
            fee_paid,
            spot_price_before_micro: spot_before,
            spot_price_after_micro: spot_after,
        })
    }

    /// Applies a previously-quoted buy to the state in-place.
    pub fn apply_buy(&mut self, fee_net_amount_in: u64, amount_out: u64) {
        self.balance_quote_lamports = self
            .balance_quote_lamports
            .saturating_add(fee_net_amount_in);
        self.balance_token_lamports = self.balance_token_lamports.saturating_sub(amount_out);
    }

    /// Quotes a sell symmetric with `quote_buy`.
    pub fn quote_sell(
        &self,
        config: &PoolConfig,
        amount_in_token_lamports: u64,
    ) -> Result<QuoteResult, EngineError> {
        let raw_out = compute_sell_out(
            self.balance_token_lamports,
            self.balance_quote_lamports,
            self.weight_token_micro,
            self.weight_quote_micro,
            amount_in_token_lamports,
        )?;
        let fee_micro = config.swap_fee_bps as u128;
        let amount_out = ((raw_out as u128) * (10_000 - fee_micro) / 10_000) as u64;
        let fee_paid = raw_out.saturating_sub(amount_out);
        let spot_before = self.spot_price_micro()?;
        let new_balance_token = self
            .balance_token_lamports
            .saturating_add(amount_in_token_lamports);
        let new_balance_quote = self.balance_quote_lamports.saturating_sub(raw_out);
        let spot_after = spot_price_micro(
            new_balance_quote,
            new_balance_token,
            self.weight_quote_micro,
            self.weight_token_micro,
        )?;
        Ok(QuoteResult {
            amount_out,
            fee_paid,
            spot_price_before_micro: spot_before,
            spot_price_after_micro: spot_after,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dripz_curves::{AnyCurve, CurveParams};

    fn fixture() -> (PoolConfig, PoolState) {
        let curve = AnyCurve::from_params(&CurveParams::linear(990_000, 500_000, 604_800)).unwrap();
        let config = PoolConfig::new("DRIPZ", "USDC", 30, curve).unwrap();
        let mut state = PoolState {
            balance_quote_lamports: 10_000_000_000,
            balance_token_lamports: 10_000_000_000,
            weight_token_micro: 990_000,
            weight_quote_micro: 10_000,
            elapsed_secs: 0,
        };
        state.refresh_weights(&config).unwrap();
        (config, state)
    }

    #[test]
    fn buy_quote_returns_positive_amount() {
        let (config, state) = fixture();
        let q = state.quote_buy(&config, 100_000_000).unwrap();
        assert!(q.amount_out > 0);
        assert!(q.fee_paid > 0);
    }

    #[test]
    fn refresh_weights_updates_with_curve() {
        let (config, mut state) = fixture();
        state.elapsed_secs = 302_400; // halfway through linear curve
        state.refresh_weights(&config).unwrap();
        assert!(state.weight_token_micro < 990_000);
        assert!(state.weight_token_micro > 500_000);
    }

    #[test]
    fn rejects_excessive_swap_fee() {
        let curve = AnyCurve::from_params(&CurveParams::linear(990_000, 500_000, 604_800)).unwrap();
        let err = PoolConfig::new("DRIPZ", "USDC", 2_000, curve).unwrap_err();
        assert!(matches!(err, EngineError::SwapFeeTooHigh { .. }));
    }
}
