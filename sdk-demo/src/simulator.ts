import Decimal from "decimal.js";
import type { CurveConfig, PoolState } from "./types";
import { weightAt, spotPrice } from "./spot-price";

export interface SimulationStep {
  elapsedSecs: number;
  weightToken: Decimal;
  weightQuote: Decimal;
  spotPrice: Decimal;
  balanceToken: Decimal;
  balanceQuote: Decimal;
}

export interface SimulationOpts {
  intervalSecs: number;
  realizedBuyPressureFn?: (elapsedSecs: number) => Decimal;
}

export function simulateCurve(
  config: CurveConfig,
  initialState: Omit<PoolState, "weightToken" | "weightQuote">,
  opts: SimulationOpts
): SimulationStep[] {
  const steps: SimulationStep[] = [];
  const points = Math.max(
    2,
    Math.floor(config.durationSecs / opts.intervalSecs) + 1
  );

  for (let i = 0; i < points; i++) {
    const elapsedSecs = Math.min(i * opts.intervalSecs, config.durationSecs);
    const pressure = opts.realizedBuyPressureFn
      ? opts.realizedBuyPressureFn(elapsedSecs)
      : new Decimal(0);
    const wToken = weightAt(config, elapsedSecs, pressure);
    const wQuote = new Decimal(1).minus(wToken);

    const state: PoolState = {
      ...initialState,
      weightToken: wToken,
      weightQuote: wQuote,
    };
    steps.push({
      elapsedSecs,
      weightToken: wToken,
      weightQuote: wQuote,
      spotPrice: spotPrice(state),
      balanceToken: state.balanceToken,
      balanceQuote: state.balanceQuote,
    });
  }
  return steps;
}
