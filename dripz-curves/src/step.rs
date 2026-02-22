//! Step (piecewise-constant) curve.
//!
//! The weight stays constant inside each window and snaps to the next level at
//! the boundary. Common configuration for DAO launches: 99% / 80% / 65% / 50%
//! over four equal slices. To minimise MEV at the transition block, the
//! Anchor program sweeps the pool inside a Jito bundle whenever it detects a
//! step boundary in the next slot.

use crate::common::{CurveKind, WEIGHT_PRECISION_MICRO};
use crate::error::CurveError;
use crate::Curve;

/// A single rung in the step curve.
#[derive(Debug, Clone, Copy)]
pub struct StepLevel {
    /// Fractional time `t/T` (in micro-units; 0 == start, 1_000_000 == end).
    pub t_fraction_micro: u64,
    /// Token weight in micro-units at this rung.
    pub weight_token_micro: u64,
}

/// Step curve.
#[derive(Debug, Clone)]
pub struct StepCurve {
    levels: Vec<StepLevel>,
    duration_secs: u64,
}

impl StepCurve {
    /// Builds a step curve from explicit levels (ordered by `t_fraction_micro`).
    pub fn new(levels: Vec<StepLevel>, duration_secs: u64) -> Result<Self, CurveError> {
        if levels.is_empty() {
            return Err(CurveError::InvalidStepConfig("levels must not be empty"));
        }
        if duration_secs == 0 {
            return Err(CurveError::DurationTooShort {
                got: duration_secs,
                min: 1,
            });
        }
        let mut ordered = levels;
        ordered.sort_by_key(|l| l.t_fraction_micro);
        for level in &ordered {
            if level.weight_token_micro > WEIGHT_PRECISION_MICRO {
                return Err(CurveError::WeightOutOfRange {
                    got: level.weight_token_micro,
                    max: WEIGHT_PRECISION_MICRO,
                });
            }
            if level.t_fraction_micro > WEIGHT_PRECISION_MICRO {
                return Err(CurveError::InvalidStepConfig(
                    "step t_fraction_micro must be <= 1_000_000",
                ));
            }
        }
        if ordered[0].t_fraction_micro != 0 {
            return Err(CurveError::InvalidStepConfig(
                "first level must start at t=0",
            ));
        }
        Ok(Self {
            levels: ordered,
            duration_secs,
        })
    }

    /// Builds the canonical 4-tier step curve.
    pub fn with_default_levels(
        start: u64,
        end: u64,
        duration_secs: u64,
    ) -> Result<Self, CurveError> {
        if start <= end {
            return Err(CurveError::NonDecreasingWeights);
        }
        let span = start - end;
        let tier_two = start - (span * 3) / 8;
        let tier_three = start - (span * 5) / 8;
        let levels = vec![
            StepLevel {
                t_fraction_micro: 0,
                weight_token_micro: start,
            },
            StepLevel {
                t_fraction_micro: 250_000,
                weight_token_micro: tier_two,
            },
            StepLevel {
                t_fraction_micro: 500_000,
                weight_token_micro: tier_three,
            },
            StepLevel {
                t_fraction_micro: 750_000,
                weight_token_micro: end,
            },
        ];
        Self::new(levels, duration_secs)
    }

    /// Returns the configured levels.
    pub fn levels(&self) -> &[StepLevel] {
        &self.levels
    }

    /// Returns the number of step rungs.
    pub fn rung_count(&self) -> usize {
        self.levels.len()
    }

    /// Returns the configured LBP duration.
    pub fn duration(&self) -> u64 {
        self.duration_secs
    }
}

impl Curve for StepCurve {
    fn weight_token_micro(&self, elapsed_secs: u64) -> Result<u64, CurveError> {
        let t = elapsed_secs.min(self.duration_secs);
        let fraction = if self.duration_secs == 0 {
            0
        } else {
            (t as u128 * WEIGHT_PRECISION_MICRO as u128 / self.duration_secs as u128) as u64
        };
        let mut current = self.levels[0].weight_token_micro;
        for level in &self.levels {
            if fraction >= level.t_fraction_micro {
                current = level.weight_token_micro;
            } else {
                break;
            }
        }
        Ok(current)
    }

    fn kind(&self) -> CurveKind {
        CurveKind::Step
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_default() -> StepCurve {
        StepCurve::with_default_levels(990_000, 500_000, 1_000).unwrap()
    }

    #[test]
    fn default_curve_has_four_rungs() {
        let curve = build_default();
        assert_eq!(curve.rung_count(), 4);
    }

    #[test]
    fn weight_snaps_to_each_rung() {
        let curve = build_default();
        assert_eq!(curve.weight_token_micro(0).unwrap(), 990_000);
        assert!(curve.weight_token_micro(300).unwrap() < 990_000);
        assert!(curve.weight_token_micro(500).unwrap() < curve.weight_token_micro(300).unwrap());
        assert_eq!(curve.weight_token_micro(1_000).unwrap(), 500_000);
    }

    #[test]
    fn weight_never_increases() {
        let curve = build_default();
        let mut prev = u64::MAX;
        for t in 0..=1_000 {
            let w = curve.weight_token_micro(t).unwrap();
            assert!(
                w <= prev,
                "step curve increased at t={t}: prev={prev} new={w}"
            );
            prev = w;
        }
    }

    #[test]
    fn rejects_empty_level_set() {
        assert!(StepCurve::new(vec![], 1_000).is_err());
    }

    #[test]
    fn rejects_first_level_not_at_zero() {
        let levels = vec![StepLevel {
            t_fraction_micro: 100_000,
            weight_token_micro: 900_000,
        }];
        assert!(StepCurve::new(levels, 1_000).is_err());
    }
}
