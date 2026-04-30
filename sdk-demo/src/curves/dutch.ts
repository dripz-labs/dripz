import Decimal from "decimal.js";
import type { CurveConfig } from "../types";

export function dutchPrice(config: CurveConfig, elapsedSecs: number): Decimal {
  if (config.kind !== "dutch") {
    throw new Error(`dutchPrice expects kind=dutch, got ${config.kind}`);
  }
  if (
    config.dutchPriceMax === undefined ||
    config.dutchPriceMin === undefined
  ) {
    throw new Error("dutch curve requires dutchPriceMax and dutchPriceMin");
  }
  const t = Math.max(0, Math.min(elapsedSecs, config.durationSecs));
  const tFraction = new Decimal(t).div(config.durationSecs);
  const pMax = new Decimal(config.dutchPriceMax);
  const pMin = new Decimal(config.dutchPriceMin);
  return pMax.minus(pMax.minus(pMin).mul(tFraction));
}

export function dutchWeight(config: CurveConfig): Decimal {
  return new Decimal(config.startWeightToken);
}
