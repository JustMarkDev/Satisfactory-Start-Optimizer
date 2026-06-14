import { describe, expect, test } from "bun:test";

import {
  DEFAULT_PHASE_IDS,
  hasOptimizationObjective,
  nonZeroWeights,
  parsePurityMultiplier,
  RESOURCES,
} from "./mapContracts.js";

describe("map contracts", () => {
  test("resource IDs are unique", () => {
    const ids = RESOURCES.map((resource) => resource.id);

    expect(new Set(ids).size).toBe(ids.length);
  });

  test("default UI phases match API preset IDs", () => {
    expect(DEFAULT_PHASE_IDS).toEqual([
      "phase1",
      "phase2",
      "phase3",
      "phase4",
      "phase5",
      "collectibles",
    ]);
  });

  test("parsePurityMultiplier maps engine purity tokens", () => {
    expect(parsePurityMultiplier("RP_Inpure")).toBe(0.5);
    expect(parsePurityMultiplier("RP_Impure")).toBe(0.5);
    expect(parsePurityMultiplier("RP_Normal")).toBe(1.0);
    expect(parsePurityMultiplier("RP_Pure")).toBe(2.0);
  });

  test("parsePurityMultiplier maps lowercase purity tokens", () => {
    expect(parsePurityMultiplier("impure")).toBe(0.5);
    expect(parsePurityMultiplier("normal")).toBe(1.0);
    expect(parsePurityMultiplier("pure")).toBe(2.0);
  });

  test("empty optimization objectives are detectable before API calls", () => {
    expect(nonZeroWeights({ iron: 0, copper: 1 })).toEqual({ copper: 1 });
    expect(hasOptimizationObjective({ iron: 0, copper: 0 })).toBe(false);
    expect(hasOptimizationObjective({ iron: 0, copper: 1 })).toBe(true);
  });
});
