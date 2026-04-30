import { describe, expect, it } from "vitest";
import Decimal from "decimal.js";
import { linearWeight } from "./curves/linear";
import { exponentialWeight } from "./curves/exponential";
import { stepWeight } from "./curves/step";
import { dutchPrice, dutchWeight } from "./curves/dutch";
import { fairWeight } from "./curves/fair";
import type { CurveConfig } from "./types";

describe("linearWeight", () => {
  it("matches endpoints", () => {
    const config: CurveConfig = {
      kind: "linear",
      startWeightToken: 0.99,
      endWeightToken: 0.5,
      durationSecs: 1000,
    };
    expect(linearWeight(config, 0).toNumber()).toBeCloseTo(0.99, 6);
    expect(linearWeight(config, 1000).toNumber()).toBeCloseTo(0.5, 6);
  });

  it("decreases monotonically", () => {
    const config: CurveConfig = {
      kind: "linear",
      startWeightToken: 0.9,
      endWeightToken: 0.5,
      durationSecs: 1000,
    };
    let prev = 1;
    for (let t = 0; t <= 1000; t += 50) {
      const w = linearWeight(config, t).toNumber();
      expect(w).toBeLessThanOrEqual(prev + 1e-12);
      prev = w;
    }
  });
});

describe("exponentialWeight", () => {
  it("returns start at t=0 and approaches end at t=T", () => {
    const config: CurveConfig = {
      kind: "exponential",
      startWeightToken: 0.99,
      endWeightToken: 0.5,
      durationSecs: 1000,
      exponentialK: 3,
    };
    expect(exponentialWeight(config, 0).toNumber()).toBeCloseTo(0.99, 6);
    expect(exponentialWeight(config, 1000).toNumber()).toBeLessThan(0.55);
  });

  it("larger k decays faster", () => {
    const mild: CurveConfig = {
      kind: "exponential",
      startWeightToken: 0.9,
      endWeightToken: 0.5,
      durationSecs: 1000,
      exponentialK: 1,
    };
    const steep: CurveConfig = { ...mild, exponentialK: 5 };
    const tThird = 333;
    expect(exponentialWeight(steep, tThird).toNumber()).toBeLessThan(
      exponentialWeight(mild, tThird).toNumber()
    );
  });
});

describe("stepWeight", () => {
  it("snaps to default 4 rungs", () => {
    const config: CurveConfig = {
      kind: "step",
      startWeightToken: 0.99,
      endWeightToken: 0.5,
      durationSecs: 1000,
    };
    expect(stepWeight(config, 0).toNumber()).toBe(0.99);
    expect(stepWeight(config, 999).toNumber()).toBe(0.5);
  });
});

describe("dutch", () => {
  it("price decays linearly between max and min", () => {
    const config: CurveConfig = {
      kind: "dutch",
      startWeightToken: 0.5,
      endWeightToken: 0.5,
      durationSecs: 1000,
      dutchPriceMax: 100,
      dutchPriceMin: 10,
    };
    expect(dutchPrice(config, 0).toNumber()).toBe(100);
    expect(dutchPrice(config, 1000).toNumber()).toBeCloseTo(10, 6);
    expect(dutchWeight(config).toNumber()).toBe(0.5);
  });
});

describe("fairWeight", () => {
  it("equals linear when there is no buy pressure", () => {
    const config: CurveConfig = {
      kind: "fair",
      startWeightToken: 0.9,
      endWeightToken: 0.5,
      durationSecs: 1000,
      fairAlpha: 0.3,
    };
    const linearConfig: CurveConfig = { ...config, kind: "linear" };
    const t = 500;
    const fair = fairWeight(config, t, new Decimal(0)).toNumber();
    const linear = linearWeight(linearConfig, t).toNumber();
    expect(fair).toBeCloseTo(linear, 6);
  });

  it("strong buy pressure keeps weight elevated", () => {
    const config: CurveConfig = {
      kind: "fair",
      startWeightToken: 0.9,
      endWeightToken: 0.5,
      durationSecs: 1000,
      fairAlpha: 0.5,
    };
    const t = 500;
    const noDemand = fairWeight(config, t, new Decimal(0)).toNumber();
    const heavyDemand = fairWeight(config, t, new Decimal(0.8)).toNumber();
    expect(heavyDemand).toBeGreaterThan(noDemand);
  });
});
