// Resource Registry and Stylings
export const RESOURCES = [
  // Core Resources
  { id: "iron", name: "Iron Ore", color: "#4682B4", category: "core" },
  { id: "copper", name: "Copper Ore", color: "#B87333", category: "core" },
  { id: "limestone", name: "Limestone", color: "#E5E5C0", category: "core" },
  { id: "coal", name: "Coal", color: "#555555", category: "core" },
  { id: "water", name: "Water (Static)", color: "#00BFFF", category: "core" },
  { id: "oil", name: "Crude Oil", color: "#2E2E2E", category: "core" },
  { id: "sulfur", name: "Sulfur", color: "#DAA520", category: "core" },
  { id: "quartz", name: "Raw Quartz", color: "#FFD1DC", category: "core" },
  { id: "caterium", name: "Caterium Ore", color: "#FFD700", category: "core" },
  { id: "bauxite", name: "Bauxite", color: "#CD5C5C", category: "core" },
  { id: "uranium", name: "Uranium", color: "#7FFF00", category: "core" },
  { id: "sam", name: "SAM Ore", color: "#9370DB", category: "core" },
  { id: "nitrogenwell", name: "Nitrogen Gas Well", color: "#48D1CC", category: "core" },
  { id: "waterwell", name: "Water Well", color: "#1E90FF", category: "core" },
  { id: "geyser", name: "Geyser", color: "#A9A9A9", category: "core" },

  // Collectibles / Research
  { id: "blueslug", name: "Blue Slug", color: "#00FFFF", category: "collectible" },
  { id: "yellowslug", name: "Yellow Slug", color: "#FFFF00", category: "collectible" },
  { id: "purpleslug", name: "Purple Slug", color: "#FF00FF", category: "collectible" },
  { id: "mercer", name: "Mercer Sphere", color: "#FF7F50", category: "collectible" },
  { id: "somersloop", name: "Somersloop", color: "#D1C4E9", category: "collectible" },
  { id: "harddrive", name: "Hard Drive", color: "#CD7F32", category: "collectible" },
  { id: "paleberry", name: "Paleberry", color: "#DB7093", category: "collectible" },
  { id: "berylnut", name: "Berylnut", color: "#D2B48C", category: "collectible" },
  { id: "baconagaric", name: "Bacon Agaric", color: "#BC8F8F", category: "collectible" },

  // Threats
  { id: "sporeflower", name: "Spore Flower", color: "#8B008B", category: "threat" },
  { id: "gaspillar", name: "Gas Pillar", color: "#2E8B57", category: "threat" },
];

export const DEFAULT_PHASE_IDS = ["phase1", "phase2", "phase3", "phase4", "phase5", "collectibles"];

export function parsePurityMultiplier(purity) {
  if (purity.includes("pure") && !purity.includes("impure")) {
    return 2.0;
  }
  if (purity.includes("impure") || purity.includes("inpure")) {
    return 0.5;
  }
  return 1.0;
}

export function nonZeroWeights(weights) {
  return Object.fromEntries(Object.entries(weights).filter(([_, value]) => value !== 0));
}

export function hasOptimizationObjective(weights) {
  return Object.keys(nonZeroWeights(weights)).length > 0;
}

// Parse Complete Map Data from Raw JSON format
export function parseNodes(data) {
  const nodes = [];
  const waterwellIndices = [];

  if (data && data.options) {
    data.options.forEach((category) => {
      if (category.options) {
        category.options.forEach((subcat) => {
          if (subcat.options) {
            subcat.options.forEach((item) => {
              let resType = "";
              const id = item.layerId;
              if (id.startsWith("limestone")) resType = "limestone";
              else if (id.startsWith("iron")) resType = "iron";
              else if (id.startsWith("copper")) resType = "copper";
              else if (id.startsWith("caterium")) resType = "caterium";
              else if (id.startsWith("coal")) resType = "coal";
              else if (id.startsWith("oilWell") || id.startsWith("oil")) resType = "oil";
              else if (id.startsWith("sulfur")) resType = "sulfur";
              else if (id.startsWith("bauxite")) resType = "bauxite";
              else if (id.startsWith("quartz")) resType = "quartz";
              else if (id.startsWith("uranium")) resType = "uranium";
              else if (id.startsWith("sam")) resType = "sam";
              else if (id.startsWith("nitrogen")) resType = "nitrogenwell";
              else if (id.startsWith("water")) resType = "waterwell";
              else if (id.startsWith("geyser")) resType = "geyser";
              else if (id === "greenSlugs") resType = "blueslug";
              else if (id === "yellowSlugs") resType = "yellowslug";
              else if (id === "purpleSlugs") resType = "purpleslug";
              else if (id === "mercerSpheres") resType = "mercer";
              else if (id === "somersloops") resType = "somersloop";
              else if (id === "hardDrives") resType = "harddrive";
              else if (id === "paleBerry") resType = "paleberry";
              else if (id === "berylNut") resType = "berylnut";
              else if (id === "baconAgaric") resType = "baconagaric";
              else if (id === "sporeFlowers") resType = "sporeflower";
              else if (id === "pillars") resType = "gaspillar";

              if (!resType) return;

              if (item.markers) {
                const addMarker = (marker) => {
                  let rawType = marker.type || resType;
                  let mType = resType;

                  switch (rawType) {
                    case "Desc_Stone_C":
                      mType = "limestone";
                      break;
                    case "Desc_OreIron_C":
                      mType = "iron";
                      break;
                    case "Desc_OreCopper_C":
                      mType = "copper";
                      break;
                    case "Desc_OreGold_C":
                      mType = "caterium";
                      break;
                    case "Desc_Coal_C":
                      mType = "coal";
                      break;
                    case "Desc_LiquidOil_C":
                      mType = "oil";
                      break;
                    case "Desc_Sulfur_C":
                      mType = "sulfur";
                      break;
                    case "Desc_OreBauxite_C":
                      mType = "bauxite";
                      break;
                    case "Desc_RawQuartz_C":
                      mType = "quartz";
                      break;
                    case "Desc_OreUranium_C":
                      mType = "uranium";
                      break;
                    case "Desc_SAM_C":
                      mType = "sam";
                      break;
                    case "Desc_NitrogenGas_C":
                      mType = "nitrogenwell";
                      break;
                    case "Desc_Water_C":
                      mType = "waterwell";
                      break;
                    default:
                      if (marker.pathName && marker.pathName.includes("BP_ResourceNodeGeyser")) {
                        mType = "geyser";
                      } else {
                        mType = resType;
                      }
                      break;
                  }

                  if (mType.startsWith("water")) mType = "waterwell";
                  if (mType.startsWith("nitrogen")) mType = "nitrogenwell";

                  let purityStr = marker.purity || item.purity || "normal";
                  let purityMultiplier = parsePurityMultiplier(purityStr);

                  let obstructed = !!marker.obstructed;
                  if (!obstructed && mType === "caterium") {
                    const isStartingCaterium =
                      Math.abs(marker.x - -220000.0) < 50000.0 &&
                      Math.abs(marker.y - -150000.0) < 50000.0;
                    if (!isStartingCaterium) obstructed = true;
                  }

                  const node = {
                    resource_type: mType,
                    x: parseFloat(marker.x),
                    y: parseFloat(marker.y),
                    z: parseFloat(marker.z || 0),
                    purityMultiplier,
                    purity: purityStr,
                    obstructed,
                  };

                  nodes.push(node);

                  if (mType === "waterwell") {
                    waterwellIndices.push(nodes.length - 1);
                  }
                };

                if (Array.isArray(item.markers)) {
                  item.markers.forEach(addMarker);
                } else if (typeof item.markers === "object" && item.markers !== null) {
                  Object.keys(item.markers).forEach((k) => {
                    addMarker(item.markers[k]);
                  });
                }
              }
            });
          }
        });
      }
    });
  }

  nodes.waterwellIndices = waterwellIndices;
  return nodes;
}
