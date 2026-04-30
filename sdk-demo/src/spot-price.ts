import Decimal from "decimal.js";
import type { CurveConfig, PoolState, SwapResult } from "./types";
import { linearWeight } from "./curves/linear";
import { exponentialWeight } from "./curves/exponential";
import { stepWeight } from "./curves/step";
import { dutchWeight } from "./curves/dutch";
import { fairWeight } from "./curves/fair";

export function weightAt(
  config: CurveConfig,
  elapsedSecs: number,
  realizedBuyPressure: Decimal = new Decimal(0)
): Decimal {
  switch (config.kind) {
    case "linear":
      return linearWeight(config, elapsedSecs);
    case "exponential":
      return exponentialWeight(config, elapsedSecs);
    case "step":
      return stepWeight(config, elapsedSecs);
    case "dutch":
      return dutchWeight(config);
    case "fair":
      return fairWeight(config, elapsedSecs, realizedBuyPressure);
    default: {
      const exhaustive: never = config.kind;
      throw new Error(`unknown curve kind ${exhaustive}`);
    }
  }
}

export function spotPrice(state: PoolState): Decimal {
  const numerator = state.balanceQuote.div(state.weightQuote);
  const denominator = state.balanceToken.div(state.weightToken);
  return numerator.div(denominator);
}

export function computeBuyOut(
  state: PoolState,
  amountInQuote: Decimal
): SwapResult {
  const feeMultiplier = new Decimal(1).minus(
    new Decimal(state.swapFeeBps).div(10_000)
  );
  const amountInNet = amountInQuote.mul(feeMultiplier);
  const feePaid = amountInQuote.minus(amountInNet);

  const wRatio = state.weightQuote.div(state.weightToken);
  const base = state.balanceQuote.div(state.balanceQuote.plus(amountInNet));
  const power = base.pow(wRatio);
  const amountOut = state.balanceToken.mul(new Decimal(1).minus(power));

  const spotBefore = spotPrice(state);
  const newState: PoolState = {
    ...state,
    balanceQuote: state.balanceQuote.plus(amountInNet),
    balanceToken: state.balanceToken.minus(amountOut),
  };
  const spotAfter = spotPrice(newState);
  const priceImpactBps = Math.round(
    spotAfter.minus(spotBefore).div(spotBefore).mul(10_000).toNumber()
  );

  return {
    amountOut,
    spotPriceBefore: spotBefore,
    spotPriceAfter: spotAfter,
    feePaid,
    priceImpactBps,
  };
}

export function computeSellOut(
  state: PoolState,
  amountInToken: Decimal
): SwapResult {
  const wRatio = state.weightToken.div(state.weightQuote);
  const base = state.balanceToken.div(state.balanceToken.plus(amountInToken));
  const power = base.pow(wRatio);
  const grossOut = state.balanceQuote.mul(new Decimal(1).minus(power));
  const feeMultiplier = new Decimal(1).minus(
    new Decimal(state.swapFeeBps).div(10_000)
  );
  const amountOut = grossOut.mul(feeMultiplier);
  const feePaid = grossOut.minus(amountOut);

  const spotBefore = spotPrice(state);
  const newState: PoolState = {
    ...state,
    balanceToken: state.balanceToken.plus(amountInToken),
    balanceQuote: state.balanceQuote.minus(grossOut),
  };
  const spotAfter = spotPrice(newState);
  const priceImpactBps = Math.round(
    spotBefore.minus(spotAfter).div(spotBefore).mul(10_000).toNumber()
  );

  return {
    amountOut,
    spotPriceBefore: spotBefore,
    spotPriceAfter: spotAfter,
    feePaid,
    priceImpactBps,
  };
}
