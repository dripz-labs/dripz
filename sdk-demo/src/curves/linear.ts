import Decimal from "decimal.js";
import type { CurveConfig } from "../types";

export function linearWeight(config: CurveConfig, elapsedSecs: number): Decimal {
  if (config.kind !== "linear") {
    throw new Error(`linearWeight expects kind=linear, got ${config.kind}`);
  }
  const t = Math.max(0, Math.min(elapsedSecs, config.durationSecs));
  const tFraction = new Decimal(t).div(config.durationSecs);
  const w0 = new Decimal(config.startWeightToken);
  const wT = new Decimal(config.endWeightToken);
  return w0.minus(w0.minus(wT).mul(tFraction));
}
