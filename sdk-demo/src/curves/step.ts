import Decimal from "decimal.js";
import type { CurveConfig } from "../types";

const DEFAULT_STEPS = [
  { tFraction: 0.0, weightToken: 0.99 },
  { tFraction: 0.25, weightToken: 0.8 },
  { tFraction: 0.5, weightToken: 0.65 },
  { tFraction: 0.75, weightToken: 0.5 },
];

export function stepWeight(config: CurveConfig, elapsedSecs: number): Decimal {
  if (config.kind !== "step") {
    throw new Error(`stepWeight expects kind=step, got ${config.kind}`);
  }
  const steps =
    config.steps && config.steps.length > 0 ? config.steps : DEFAULT_STEPS;
  const ordered = [...steps].sort((a, b) => a.tFraction - b.tFraction);
  const t = Math.max(0, Math.min(elapsedSecs, config.durationSecs));
  const tFraction = t / config.durationSecs;

  let current = ordered[0].weightToken;
  for (const step of ordered) {
    if (tFraction >= step.tFraction) {
      current = step.weightToken;
    } else {
      break;
    }
  }
  return new Decimal(current);
}
