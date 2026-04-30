import Decimal from "decimal.js";
import type { CurveConfig } from "../types";

const DEFAULT_K = 3.0;

export function exponentialWeight(
  config: CurveConfig,
  elapsedSecs: number
): Decimal {
  if (config.kind !== "exponential") {
    throw new Error(
      `exponentialWeight expects kind=exponential, got ${config.kind}`
    );
  }
  const k = config.exponentialK ?? DEFAULT_K;
  const t = Math.max(0, Math.min(elapsedSecs, config.durationSecs));
  const tFraction = t / config.durationSecs;
  const decay = Math.exp(-k * tFraction);
  const w0 = new Decimal(config.startWeightToken);
  const wT = new Decimal(config.endWeightToken);
  return wT.plus(w0.minus(wT).mul(decay));
}
