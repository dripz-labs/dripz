import Decimal from "decimal.js";

export type CurveKind =
  | "linear"
  | "exponential"
  | "step"
  | "dutch"
  | "fair";

export interface CurveConfig {
  kind: CurveKind;
  startWeightToken: number;
  endWeightToken: number;
  durationSecs: number;
  exponentialK?: number;
  steps?: Array<{ tFraction: number; weightToken: number }>;
  dutchPriceMax?: number;
  dutchPriceMin?: number;
  fairAlpha?: number;
}

export interface PoolState {
  balanceToken: Decimal;
  balanceQuote: Decimal;
  weightToken: Decimal;
  weightQuote: Decimal;
  startTimestampSecs: number;
  swapFeeBps: number;
}

export interface SwapResult {
  amountOut: Decimal;
  spotPriceBefore: Decimal;
  spotPriceAfter: Decimal;
  feePaid: Decimal;
  priceImpactBps: number;
}

export const WEIGHT_PRECISION = 1_000_000;

export function bpsFromDecimal(value: Decimal): number {
  return Math.round(value.mul(10_000).toNumber());
}
