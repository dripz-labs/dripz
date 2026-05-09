# Curve Specification

Every Dripz LBP is parameterised by one of five curve families. The weight is expressed in micro-units so that the on-chain Anchor program and the off-chain `dripz-curves` evaluator agree on the same integer answer.

## 1. Linear Decay

$$W_{token}(t) = W_0 - (W_0 - W_T) \cdot \frac{t}{T}$$

| Property | Value |
| -------- | ----- |
| Crate symbol | `LinearCurve::new(start, end, duration_secs)` |
| Default start | 99% |
| Default end | 50% |
| Snipe profile | Constant slope; baseline reference |

Linear is the canonical Balancer LBP shape. It is the easiest curve to reason about and ships as the default in the Curve Designer.

## 2. Exponential Decay

$$W_{token}(t) = W_T + (W_0 - W_T) \cdot e^{-k \cdot t / T}$$

`k` controls steepness. Larger `k` accelerates the early drop, which raises the cost of sniping inside the first few minutes of the window. The on-chain program approximates `exp(-x)` with a six-term Taylor expansion plus halving so the input always lands in `[-1, 0]`.

## 3. Step

$$W_{token}(t) = \begin{cases} W_0 & 0 \le t/T < 0.25 \\ W_1 & 0.25 \le t/T < 0.5 \\ W_2 & 0.5 \le t/T < 0.75 \\ W_T & 0.75 \le t/T \le 1 \end{cases}$$

The 4-rung step curve is common for DAO launches (99% / 80% / 65% / 50%). The Anchor program performs a "sweep" inside a Jito bundle at every transition boundary so MEV cannot front-run the weight change.

## 4. Dutch Auction

$$P(t) = P_{max} - (P_{max} - P_{min}) \cdot \frac{t}{T}$$

Weight stays constant; the *quoted price* falls linearly from `P_max` to `P_min`. Buys are accepted as long as the realised swap price stays inside the current band. This is the curve modelled on the Liquid Token Launch design.

## 5. Fair Discovery

$$W_{token}(t, d) = \mathrm{base}(t) + \alpha \cdot d$$

`d` is the realised buy-pressure signal (fraction of tokens already sold). `alpha` is a smoothing constant scaled by 1e6. When demand is strong the weight stays elevated, keeping the price higher for longer. When demand is weak the curve falls back to the linear baseline. This is the Copper LBP fair-discovery pattern.

## Snipe resistance comparison

The `dripz backtest` command runs a synthetic Pump.fun-style demand profile -- 70% of volume in the first 5% of the window -- against any two curves. Sample output:

```bash
$ dripz backtest --baseline linear --candidate exponential \
    --start-weight-micro 990000 --end-weight-micro 500000 --duration-secs 604800
```

| Curve | Final spot price (micro) | First-5% volume share (bps) | Average slippage (bps) |
| ----- | ----------------------- | --------------------------- | ---------------------- |
| Linear | 14 | 7000 | 980 |
| Exponential (k=4) | 11 | 7000 | 1190 |

The exponential curve doubles the price impact of front-loaded buys, which is exactly the desired behaviour.
