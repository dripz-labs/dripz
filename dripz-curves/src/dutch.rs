//! Dutch auction curve.
//!
//! Weight is held constant but the *price* schedule decays linearly:
//! `P(t) = P_max - (P_max - P_min) * (t / T)`.
//!
//! This is the curve used by Liquid Token Launch and many NFT-style launches
//! where the issuer wants a guaranteed clearing band rather than a weight
//! curve. The pool engine still uses the Balancer spot-price formula; buys
//! are accepted if and only if the resulting realised price is within the
//! current Dutch price band.

use crate::common::{CurveKind, WEIGHT_PRECISION_MICRO};
use crate::error::CurveError;
use crate::Curve;

/// Closed-form Dutch auction curve.
#[derive(Debug, Clone, Copy)]
pub struct DutchCurve {
    constant_weight_token_micro: u64,
    duration_secs: u64,
    price_max_micro: u64,
    price_min_micro: u64,
}

impl DutchCurve {
    /// Constructs a Dutch curve. `price_max_micro` and `price_min_micro` are
    /// quote-per-token prices scaled by 1e6.
    pub fn new(
        constant_weight_token_micro: u64,
        duration_secs: u64,
        price_max_micro: u64,
        price_min_micro: u64,
    ) -> Result<Self, CurveError> {
        if constant_weight_token_micro == 0 || constant_weight_token_micro > WEIGHT_PRECISION_MICRO
        {
            return Err(CurveError::WeightOutOfRange {
                got: constant_weight_token_micro,
                max: WEIGHT_PRECISION_MICRO,
            });
        }
        if duration_secs == 0 {
            return Err(CurveError::DurationTooShort {
                got: duration_secs,
                min: 1,
            });
        }
        if price_max_micro <= price_min_micro {
            return Err(CurveError::InvalidDutchConfig(
                "price_max must be strictly greater than price_min",
            ));
        }
        Ok(Self {
            constant_weight_token_micro,
            duration_secs,
            price_max_micro,
            price_min_micro,
        })
    }

    /// Returns the current price in micro-units.
    pub fn price_micro_at(&self, elapsed_secs: u64) -> u64 {
        let t = elapsed_secs.min(self.duration_secs);
        let span = self.price_max_micro - self.price_min_micro;
        let scaled = (span as u128 * t as u128) / self.duration_secs as u128;
        self.price_max_micro - scaled as u64
    }

    /// Returns the configured maximum price (start of auction).
    pub fn price_max(&self) -> u64 {
        self.price_max_micro
    }

    /// Returns the configured minimum (reserve) price.
    pub fn price_min(&self) -> u64 {
        self.price_min_micro
    }

    /// Returns the LBP duration.
    pub fn duration(&self) -> u64 {
        self.duration_secs
    }

    /// Samples the price curve.
    pub fn sample_price(&self, points: usize) -> Vec<(u64, u64)> {
        let points = points.max(2);
        let mut out = Vec::with_capacity(points);
        for i in 0..points {
            let t = (i as u64 * self.duration_secs) / (points as u64 - 1);
            out.push((t, self.price_micro_at(t)));
        }
        out
    }
}

impl Curve for DutchCurve {
    fn weight_token_micro(&self, _elapsed_secs: u64) -> Result<u64, CurveError> {
        Ok(self.constant_weight_token_micro)
    }

    fn kind(&self) -> CurveKind {
        CurveKind::Dutch
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weight_is_constant() {
        let curve = DutchCurve::new(500_000, 1_000, 1_000_000, 100_000).unwrap();
        for t in 0..=1_000 {
            assert_eq!(curve.weight_token_micro(t).unwrap(), 500_000);
        }
    }

    #[test]
    fn price_endpoints_match() {
        let curve = DutchCurve::new(500_000, 1_000, 2_000_000, 500_000).unwrap();
        assert_eq!(curve.price_micro_at(0), 2_000_000);
        assert_eq!(curve.price_micro_at(1_000), 500_000);
    }

    #[test]
    fn price_is_monotonically_decreasing() {
        let curve = DutchCurve::new(500_000, 1_000, 2_000_000, 500_000).unwrap();
        let mut prev = u64::MAX;
        for t in (0..=1_000).step_by(20) {
            let p = curve.price_micro_at(t);
            assert!(p <= prev);
            prev = p;
        }
    }

    #[test]
    fn rejects_invalid_price_range() {
        assert!(DutchCurve::new(500_000, 1_000, 500_000, 500_000).is_err());
        assert!(DutchCurve::new(500_000, 1_000, 100_000, 200_000).is_err());
    }
}
