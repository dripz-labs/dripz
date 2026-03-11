//! Closed-form weighted pool math implemented in u128 fixed point.
//!
//! These functions are the Rust mirror of `pool-engine` and produce the same
//! numbers as the on-chain Anchor program. Floating point is forbidden so
//! that swaps quoted off-chain match swaps executed on-chain bit-for-bit.

use crate::error::EngineError;

const WEIGHT_PRECISION_MICRO: u128 = 1_000_000;
const FIXED_SCALE: u128 = 1 << 64;

/// Spot price in micro-units (quote per token, scaled by 1e6).
///
/// `price = (balance_in / weight_in) / (balance_out / weight_out)`
pub fn spot_price_micro(
    balance_in: u64,
    balance_out: u64,
    weight_in_micro: u64,
    weight_out_micro: u64,
) -> Result<u128, EngineError> {
    if balance_out == 0 || weight_out_micro == 0 {
        return Err(EngineError::DivisionByZero);
    }
    let bi = balance_in as u128;
    let bo = balance_out as u128;
    let wi = weight_in_micro as u128;
    let wo = weight_out_micro as u128;
    let numerator = bi.checked_mul(wo).ok_or(EngineError::Overflow)?;
    let denominator = bo.checked_mul(wi).ok_or(EngineError::Overflow)?;
    if denominator == 0 {
        return Err(EngineError::DivisionByZero);
    }
    let price = numerator
        .checked_mul(WEIGHT_PRECISION_MICRO)
        .ok_or(EngineError::Overflow)?
        / denominator;
    Ok(price)
}

/// Quotes the amount-out for a buy on the weighted pool. The math matches
/// Balancer V2 exactly: `out = balance_out * (1 - (Bi / (Bi + Ai))^(Wi/Wo))`.
pub fn compute_buy_out(
    balance_in: u64,
    balance_out: u64,
    weight_in_micro: u64,
    weight_out_micro: u64,
    amount_in: u64,
) -> Result<u64, EngineError> {
    if balance_in == 0 || balance_out == 0 {
        return Err(EngineError::DivisionByZero);
    }
    if weight_in_micro == 0 || weight_out_micro == 0 {
        return Err(EngineError::WeightOutOfRange {
            got: weight_in_micro.min(weight_out_micro),
            max: WEIGHT_PRECISION_MICRO as u64,
        });
    }

    let bi: u128 = balance_in as u128;
    let bo: u128 = balance_out as u128;
    let wi: u128 = weight_in_micro as u128;
    let wo: u128 = weight_out_micro as u128;
    let ai: u128 = amount_in as u128;

    let base = bi.checked_mul(FIXED_SCALE).ok_or(EngineError::Overflow)?
        / bi.checked_add(ai).ok_or(EngineError::Overflow)?;
    let exponent = wi.checked_mul(FIXED_SCALE).ok_or(EngineError::Overflow)? / wo;
    let factor = pow_fixed(base, exponent)?;
    let one = FIXED_SCALE;
    let factor_complement = one
        .checked_sub(factor.min(one))
        .ok_or(EngineError::Overflow)?;
    let out = bo
        .checked_mul(factor_complement)
        .ok_or(EngineError::Overflow)?
        / FIXED_SCALE;
    Ok(out as u64)
}

/// Quotes the amount-out for a sell. Symmetric with `compute_buy_out`; the
/// caller is responsible for tracking that the resulting pool weights still
/// match the curve.
pub fn compute_sell_out(
    balance_in: u64,
    balance_out: u64,
    weight_in_micro: u64,
    weight_out_micro: u64,
    amount_in: u64,
) -> Result<u64, EngineError> {
    compute_buy_out(
        balance_in,
        balance_out,
        weight_in_micro,
        weight_out_micro,
        amount_in,
    )
}

fn pow_fixed(base: u128, exponent: u128) -> Result<u128, EngineError> {
    if base == 0 {
        return Ok(0);
    }
    if exponent == 0 {
        return Ok(FIXED_SCALE);
    }
    let ln_base = ln_fixed(base)?;
    let value = i128_mul(ln_base, exponent as i128, FIXED_SCALE as i128)?;
    exp_fixed(value)
}

fn ln_fixed(x: u128) -> Result<i128, EngineError> {
    if x == 0 {
        return Err(EngineError::DivisionByZero);
    }
    let x = x as i128;
    let s = FIXED_SCALE as i128;
    let y = x.checked_sub(s).ok_or(EngineError::Overflow)?;
    let denom = x.checked_add(s).ok_or(EngineError::Overflow)?;
    // z = y / denom, scaled by s. y can be near +/- 2^64 and denom is near 2^65
    // so we cannot simply do `y * s`. Instead split into high/low halves.
    let z = mul_div_signed(y, s, denom)?;
    let z2 = mul_div_signed(z, z, s)?;
    let mut result = z;
    let mut term = z;
    for i in 1..6 {
        term = mul_div_signed(term, z2, s)?;
        result = result
            .checked_add(term / (2 * i as i128 + 1))
            .ok_or(EngineError::Overflow)?;
    }
    Ok(result.checked_mul(2).ok_or(EngineError::Overflow)?)
}

fn exp_fixed(value: i128) -> Result<u128, EngineError> {
    let s = FIXED_SCALE as i128;
    let mut term: i128 = s;
    let mut sum: i128 = s;
    for n in 1..=10 {
        term = mul_div_signed(
            term,
            value,
            s.checked_mul(n as i128).ok_or(EngineError::Overflow)?,
        )?;
        sum = sum.checked_add(term).ok_or(EngineError::Overflow)?;
        if sum < 0 {
            sum = 0;
            break;
        }
    }
    Ok(sum as u128)
}

fn i128_mul(a: i128, b: i128, scale: i128) -> Result<i128, EngineError> {
    mul_div_signed(a, b, scale)
}

/// Computes `(a * b) / divisor` using a 256-bit intermediate so the
/// multiplication cannot overflow even when `a` and `b` are both near `2^64`.
fn mul_div_signed(a: i128, b: i128, divisor: i128) -> Result<i128, EngineError> {
    if divisor == 0 {
        return Err(EngineError::DivisionByZero);
    }
    let sign = (a.signum() * b.signum()) * divisor.signum();
    let abs_a = a.unsigned_abs();
    let abs_b = b.unsigned_abs();
    let abs_div = divisor.unsigned_abs();
    let product = mul_u128_to_u256(abs_a, abs_b);
    let quotient = div_u256_by_u128(product, abs_div)?;
    if quotient > i128::MAX as u128 {
        return Err(EngineError::Overflow);
    }
    let signed = quotient as i128;
    Ok(if sign < 0 { -signed } else { signed })
}

/// Multiplies two u128 values and returns the 256-bit product as `(high, low)`.
fn mul_u128_to_u256(a: u128, b: u128) -> (u128, u128) {
    let a_lo: u128 = a & u64::MAX as u128;
    let a_hi: u128 = a >> 64;
    let b_lo: u128 = b & u64::MAX as u128;
    let b_hi: u128 = b >> 64;
    let ll = a_lo * b_lo;
    let lh = a_lo * b_hi;
    let hl = a_hi * b_lo;
    let hh = a_hi * b_hi;
    let mid = (ll >> 64) + (lh & u64::MAX as u128) + (hl & u64::MAX as u128);
    let low = (ll & u64::MAX as u128) | (mid << 64);
    let high = hh + (lh >> 64) + (hl >> 64) + (mid >> 64);
    (high, low)
}

/// Divides a 256-bit unsigned value (`high << 128 | low`) by a u128 divisor.
fn div_u256_by_u128(value: (u128, u128), divisor: u128) -> Result<u128, EngineError> {
    let (high, low) = value;
    if divisor == 0 {
        return Err(EngineError::DivisionByZero);
    }
    if high == 0 {
        return Ok(low / divisor);
    }
    // Long division bit-by-bit so we never need a true 256-bit type.
    let mut remainder: u128 = 0;
    let mut quotient: u128 = 0;
    for bit in (0..256).rev() {
        remainder = remainder.checked_shl(1).ok_or(EngineError::Overflow)?;
        let value_bit = if bit >= 128 {
            (high >> (bit - 128)) & 1
        } else {
            (low >> bit) & 1
        };
        remainder |= value_bit;
        if remainder >= divisor {
            remainder -= divisor;
            if bit < 128 {
                quotient |= 1u128 << bit;
            } else {
                return Err(EngineError::Overflow);
            }
        }
    }
    Ok(quotient)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balanced_pool_returns_unit_price() {
        let price = spot_price_micro(1_000_000, 1_000_000, 500_000, 500_000).unwrap();
        assert!((999_500..1_000_500).contains(&(price as u64)));
    }

    #[test]
    fn buy_amount_out_within_pool_balance() {
        let out = compute_buy_out(10_000_000, 1_000_000, 100_000, 900_000, 1_000_000).unwrap();
        assert!(out > 0 && out < 1_000_000);
    }

    #[test]
    fn larger_buy_yields_more_tokens() {
        let small = compute_buy_out(10_000_000, 1_000_000, 100_000, 900_000, 100_000).unwrap();
        let big = compute_buy_out(10_000_000, 1_000_000, 100_000, 900_000, 500_000).unwrap();
        assert!(big > small);
    }

    #[test]
    fn zero_balance_in_returns_error() {
        assert!(compute_buy_out(0, 1_000, 100_000, 900_000, 10).is_err());
    }

    #[test]
    fn zero_weight_returns_error() {
        assert!(compute_buy_out(1_000, 1_000, 0, 900_000, 10).is_err());
    }

    #[test]
    fn spot_price_inversely_responds_to_weight() {
        // price = (B_in / W_in) / (B_out / W_out) -- larger output weight
        // with smaller input weight pushes the quoted price up.
        let p_when_out_weight_low =
            spot_price_micro(1_000_000, 1_000_000, 900_000, 100_000).unwrap();
        let p_when_out_weight_high =
            spot_price_micro(1_000_000, 1_000_000, 100_000, 900_000).unwrap();
        assert!(p_when_out_weight_high > p_when_out_weight_low);
    }
}
