import { describe, expect, test } from "bun:test";

import {
  DEFAULT_PHASE_IDS,
  hasOptimizationObjective,
  nonZeroWeights,
  parseNodes,
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

  test("parseNodes maps resource markers into optimizer nodes", () => {
    const nodes = parseNodes({
      options: [
        {
          options: [
            {
              options: [
                {
                  layerId: "ironNodes",
                  markers: [
                    {
                      type: "Desc_OreIron_C",
                      x: "123.5",
                      y: "-456.25",
                      z: "78",
                      purity: "pure",
                      obstructed: true,
                    },
                  ],
                },
              ],
            },
          ],
        },
      ],
    });

    expect(nodes).toHaveLength(1);
    expect(nodes[0]).toEqual({
      resource_type: "iron",
      x: 123.5,
      y: -456.25,
      z: 78,
      purityMultiplier: 2,
      purity: "pure",
      obstructed: true,
    });
    expect(nodes.waterwellIndices).toEqual([]);
  });

  test("empty optimization objectives are detectable before API calls", () => {
    expect(nonZeroWeights({ iron: 0, copper: 1 })).toEqual({ copper: 1 });
    expect(hasOptimizationObjective({ iron: 0, copper: 0 })).toBe(false);
    expect(hasOptimizationObjective({ iron: 0, copper: 1 })).toBe(true);
  });
});
