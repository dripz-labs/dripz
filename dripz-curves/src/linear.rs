//! Linear decay curve.
//!
//! `W_token(t) = W_0 - (W_0 - W_T) * (t / T)`.
//!
//! The simplest and most common LBP curve. Balancer's Liquidity Bootstrapping
//! Pool spec uses this as the canonical reference. Snipe resistance comes from
//! the constant downward slope of the token weight: buying early pays a
//! noticeably higher spot price than buying near the end of the window.

use crate::common::{linear_interp, CurveKind, WEIGHT_PRECISION_MICRO};
use crate::error::CurveError;
use crate::Curve;

/// Closed-form linear curve.
#[derive(Debug, Clone, Copy)]
pub struct LinearCurve {
    start_weight_token_micro: u64,
    end_weight_token_micro: u64,
    duration_secs: u64,
}

impl LinearCurve {
    /// Constructs a linear curve, returning an error if the parameters are
    /// outside the supported range.
    pub fn new(start: u64, end: u64, duration_secs: u64) -> Result<Self, CurveError> {
        if start > WEIGHT_PRECISION_MICRO || end > WEIGHT_PRECISION_MICRO {
            return Err(CurveError::WeightOutOfRange {
                got: start.max(end),
                max: WEIGHT_PRECISION_MICRO,
            });
        }
        if start <= end {
            return Err(CurveError::NonDecreasingWeights);
        }
        if duration_secs == 0 {
            return Err(CurveError::DurationTooShort {
                got: duration_secs,
                min: 1,
            });
        }
        Ok(Self {
            start_weight_token_micro: start,
            end_weight_token_micro: end,
            duration_secs,
        })
    }

    /// Returns the starting weight (token side).
    pub fn start(&self) -> u64 {
        self.start_weight_token_micro
    }

    /// Returns the ending weight (token side).
    pub fn end(&self) -> u64 {
        self.end_weight_token_micro
    }

    /// Returns the configured LBP duration in seconds.
    pub fn duration(&self) -> u64 {
        self.duration_secs
    }

    /// Returns the instantaneous slope in micro-units per second. Constant
    /// across the entire window; included for benchmarking / charting.
    pub fn slope_micro_per_sec(&self) -> i64 {
        let delta = self.start_weight_token_micro as i64 - self.end_weight_token_micro as i64;
        -(delta / self.duration_secs.max(1) as i64)
    }

    /// Convenience helper used by chart renderers.
    pub fn sample(&self, samples: usize) -> Vec<(u64, u64)> {
        let samples = samples.max(2);
        let mut out = Vec::with_capacity(samples);
        for i in 0..samples {
            let t = (i as u64 * self.duration_secs) / (samples as u64 - 1);
            let w = self
                .weight_token_micro(t)
                .unwrap_or(self.end_weight_token_micro);
            out.push((t, w));
        }
        out
    }
}

impl Curve for LinearCurve {
    fn weight_token_micro(&self, elapsed_secs: u64) -> Result<u64, CurveError> {
        Ok(linear_interp(
            self.start_weight_token_micro,
            self.end_weight_token_micro,
            elapsed_secs,
            self.duration_secs,
        ))
    }

    fn kind(&self) -> CurveKind {
        CurveKind::Linear
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoints_match_configuration() {
        let curve = LinearCurve::new(990_000, 500_000, 1_000).unwrap();
        assert_eq!(curve.weight_token_micro(0).unwrap(), 990_000);
        assert_eq!(curve.weight_token_micro(1_000).unwrap(), 500_000);
    }

    #[test]
    fn weight_is_monotonically_decreasing() {
        let curve = LinearCurve::new(900_000, 500_000, 1_000).unwrap();
        let mut prev = u64::MAX;
        for t in (0..=1_000).step_by(50) {
            let w = curve.weight_token_micro(t).unwrap();
            assert!(w <= prev, "weight increased at t={t}: prev={prev} new={w}");
            prev = w;
        }
    }

    #[test]
    fn rejects_invalid_configuration() {
        assert!(LinearCurve::new(500_000, 600_000, 1_000).is_err());
        assert!(LinearCurve::new(1_500_000, 500_000, 1_000).is_err());
        assert!(LinearCurve::new(900_000, 500_000, 0).is_err());
    }

    #[test]
    fn sample_produces_expected_size() {
        let curve = LinearCurve::new(990_000, 500_000, 1_000).unwrap();
        let samples = curve.sample(11);
        assert_eq!(samples.len(), 11);
        assert_eq!(samples.first().unwrap().1, 990_000);
        assert_eq!(samples.last().unwrap().1, 500_000);
    }

    #[test]
    fn slope_is_negative_for_decaying_curve() {
        let curve = LinearCurve::new(900_000, 500_000, 4_000).unwrap();
        assert!(curve.slope_micro_per_sec() < 0);
    }
}
