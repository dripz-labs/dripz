import { describe, expect, it } from "vitest";
import Decimal from "decimal.js";
import { simulateCurve } from "./simulator";
import type { CurveConfig, PoolState } from "./types";

describe("simulateCurve", () => {
  it("produces evenly spaced samples", () => {
    const config: CurveConfig = {
      kind: "linear",
      startWeightToken: 0.99,
      endWeightToken: 0.5,
      durationSecs: 1000,
    };
    const initial: Omit<PoolState, "weightToken" | "weightQuote"> = {
      balanceToken: new Decimal(1_000_000),
      balanceQuote: new Decimal(100_000),
      startTimestampSecs: 0,
      swapFeeBps: 30,
    };
    const steps = simulateCurve(config, initial, { intervalSecs: 100 });
    expect(steps.length).toBe(11);
    expect(steps[0].elapsedSecs).toBe(0);
    expect(steps[steps.length - 1].elapsedSecs).toBe(1000);
    expect(steps[0].weightToken.toNumber()).toBeCloseTo(0.99, 6);
    expect(steps[steps.length - 1].weightToken.toNumber()).toBeCloseTo(0.5, 6);
  });
});
