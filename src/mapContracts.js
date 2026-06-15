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
  const normalizedPurity = String(purity).toLowerCase();

  if (normalizedPurity === "rp_pure" || normalizedPurity === "pure") {
    return 2.0;
  }
  if (
    normalizedPurity === "rp_impure" ||
    normalizedPurity === "rp_inpure" ||
    normalizedPurity === "impure" ||
    normalizedPurity === "inpure"
  ) {
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
