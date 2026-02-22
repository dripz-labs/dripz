//! Exponential decay curve.
//!
//! `W_token(t) = W_T + (W_0 - W_T) * exp(-k * t / T)`.
//!
//! Used when the issuer wants the spot price to fall quickly during the first
//! third of the window in order to penalise snipers. The on-chain version
//! (used by the Anchor program) approximates `exp(-x)` with a six-term Taylor
//! series; this CPU implementation matches it byte-for-byte so the simulator
//! and the program agree on the same numbers.

use crate::common::{CurveKind, WEIGHT_PRECISION_MICRO};
use crate::error::CurveError;
use crate::Curve;

const TAYLOR_TERMS: usize = 6;
const FIXED_ONE: i128 = 1_000_000;

/// Closed-form exponential decay curve.
#[derive(Debug, Clone, Copy)]
pub struct ExponentialCurve {
    start_weight_token_micro: u64,
    end_weight_token_micro: u64,
    duration_secs: u64,
    k_micro: u64,
}

impl ExponentialCurve {
    /// Constructs an exponential decay curve.
    ///
    /// `k_micro` controls the steepness, scaled by 1e6. A value of `3_000_000`
    /// (`k=3.0`) means roughly 95% of the decay has happened by the time `t`
    /// reaches the end of the LBP window.
    pub fn new(start: u64, end: u64, duration_secs: u64, k_micro: u64) -> Result<Self, CurveError> {
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
        if k_micro == 0 || k_micro > 10_000_000 {
            return Err(CurveError::InvalidStepConfig(
                "k_micro must be in (0, 10_000_000]",
            ));
        }
        Ok(Self {
            start_weight_token_micro: start,
            end_weight_token_micro: end,
            duration_secs,
            k_micro,
        })
    }

    /// Six-term Taylor series for `exp(x)` in fixed-point Q-1.6.
    /// Returns the result scaled by 1e6.
    pub fn exp_fixed(x_micro: i128) -> Result<u64, CurveError> {
        let mut term: i128 = FIXED_ONE;
        let mut sum: i128 = FIXED_ONE;
        for n in 1..=TAYLOR_TERMS {
            let prod = term.checked_mul(x_micro).ok_or(CurveError::Overflow)?;
            term = prod / (FIXED_ONE * n as i128);
            sum = sum.checked_add(term).ok_or(CurveError::Overflow)?;
        }
        if sum < 0 {
            sum = 0;
        }
        Ok(sum as u64)
    }

    /// Returns the steepness scaled by 1e6.
    pub fn k(&self) -> u64 {
        self.k_micro
    }

    /// Samples the curve evenly across the duration.
    pub fn sample(&self, points: usize) -> Vec<(u64, u64)> {
        let points = points.max(2);
        let mut out = Vec::with_capacity(points);
        for i in 0..points {
            let t = (i as u64 * self.duration_secs) / (points as u64 - 1);
            let w = self
                .weight_token_micro(t)
                .unwrap_or(self.end_weight_token_micro);
            out.push((t, w));
        }
        out
    }
}

impl Curve for ExponentialCurve {
    fn weight_token_micro(&self, elapsed_secs: u64) -> Result<u64, CurveError> {
        let t = elapsed_secs.min(self.duration_secs);
        if t == 0 {
            return Ok(self.start_weight_token_micro);
        }
        // x = -k * t / T (in micro units)
        let k = self.k_micro as i128;
        let numerator = k.checked_mul(t as i128).ok_or(CurveError::Overflow)?;
        let x_micro = -(numerator / self.duration_secs as i128);
        // The 6-term Taylor series only converges well for |x| <= 2; beyond
        // that, halve and square so the input to the polynomial stays small.
        let decay = exp_negative_fixed(x_micro)?;
        let span = self
            .start_weight_token_micro
            .checked_sub(self.end_weight_token_micro)
            .ok_or(CurveError::Overflow)?;
        let scaled = (span as u128 * decay as u128) / FIXED_ONE as u128;
        let candidate = self.end_weight_token_micro.saturating_add(scaled as u64);
        // Clamp into [end, start] to guard against numerical drift.
        Ok(candidate.clamp(self.end_weight_token_micro, self.start_weight_token_micro))
    }

    fn kind(&self) -> CurveKind {
        CurveKind::Exponential
    }
}

/// Computes `exp(x_micro / 1e6)` for `x_micro <= 0`, scaled by 1e6.
///
/// Uses halving so the Taylor input stays in `[-1, 0]` where six terms give
/// better than 1e-6 accuracy.
fn exp_negative_fixed(x_micro: i128) -> Result<u64, CurveError> {
    if x_micro >= 0 {
        return Ok(FIXED_ONE as u64);
    }
    let mut x = x_micro;
    let mut halvings: u32 = 0;
    while x < -FIXED_ONE {
        x /= 2;
        halvings += 1;
        if halvings > 12 {
            // Hit the floor: exp is effectively zero in micro precision.
            return Ok(0);
        }
    }
    let mut result = ExponentialCurve::exp_fixed(x)?;
    for _ in 0..halvings {
        let r = result as u128;
        result = ((r * r) / FIXED_ONE as u128) as u64;
        if result == 0 {
            break;
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exp_zero_equals_one() {
        let result = ExponentialCurve::exp_fixed(0).unwrap();
        assert_eq!(result, FIXED_ONE as u64);
    }

    #[test]
    fn endpoints_match_expectation() {
        let curve = ExponentialCurve::new(990_000, 500_000, 7 * 86_400, 3_000_000).unwrap();
        assert_eq!(curve.weight_token_micro(0).unwrap(), 990_000);
        let end = curve.weight_token_micro(7 * 86_400).unwrap();
        assert!(
            end < 525_000,
            "expected end weight close to 500k, got {end}"
        );
        assert!(end >= 500_000);
    }

    #[test]
    fn faster_decay_with_larger_k() {
        let mild = ExponentialCurve::new(900_000, 500_000, 1_000, 1_000_000).unwrap();
        let steep = ExponentialCurve::new(900_000, 500_000, 1_000, 5_000_000).unwrap();
        let t_third = 333;
        assert!(
            steep.weight_token_micro(t_third).unwrap() < mild.weight_token_micro(t_third).unwrap()
        );
    }

    #[test]
    fn weight_is_monotonically_decreasing() {
        let curve = ExponentialCurve::new(900_000, 500_000, 1_000, 3_000_000).unwrap();
        let mut prev = u64::MAX;
        for t in (0..=1_000).step_by(25) {
            let w = curve.weight_token_micro(t).unwrap();
            assert!(
                w <= prev,
                "exp curve increased at t={t}: prev={prev} new={w}"
            );
            prev = w;
        }
    }

    #[test]
    fn rejects_invalid_k() {
        assert!(ExponentialCurve::new(900_000, 500_000, 1_000, 0).is_err());
        assert!(ExponentialCurve::new(900_000, 500_000, 1_000, 11_000_000).is_err());
    }
}
