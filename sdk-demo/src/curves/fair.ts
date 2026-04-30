import Decimal from "decimal.js";
import type { CurveConfig } from "../types";
import { linearWeight } from "./linear";

const DEFAULT_ALPHA = 0.3;

export function fairWeight(
  config: CurveConfig,
  elapsedSecs: number,
  realizedBuyPressure: Decimal
): Decimal {
  if (config.kind !== "fair") {
    throw new Error(`fairWeight expects kind=fair, got ${config.kind}`);
  }
  const alpha = new Decimal(config.fairAlpha ?? DEFAULT_ALPHA);
  const baseConfig = { ...config, kind: "linear" as const };
  const base = linearWeight(baseConfig, elapsedSecs);
  const pressureBoost = alpha.mul(realizedBuyPressure);
  const w0 = new Decimal(config.startWeightToken);
  const wT = new Decimal(config.endWeightToken);
  const adjusted = base.plus(pressureBoost);
  return Decimal.max(wT, Decimal.min(w0, adjusted));
}
