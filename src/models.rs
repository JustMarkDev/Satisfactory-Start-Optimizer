use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Purity {
    Impure,
    Normal,
    Pure,
}

impl Purity {
    pub fn multiplier(self) -> f64 {
        match self {
            Purity::Impure => 0.5,
            Purity::Normal => 1.0,
            Purity::Pure => 2.0,
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "RP_Inpure" | "RP_Impure" | "impure" => Purity::Impure,
            "RP_Normal" | "normal" => Purity::Normal,
            "RP_Pure" | "pure" => Purity::Pure,
            _ => Purity::Normal,
        }
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PurityOverride {
    Default,
    Impure,
    Normal,
    Pure,
}

impl PurityOverride {
    pub fn to_str(self) -> &'static str {
        match self {
            PurityOverride::Default => "Default (Database)",
            PurityOverride::Impure => "All Impure (0.5x)",
            PurityOverride::Normal => "All Normal (1.0x)",
            PurityOverride::Pure => "All Pure (2.0x)",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum GamePhase {
    Phase1, // Tiers 1-2 (Early Game)
    Phase2, // Tiers 3-4 (Steel & Coal Power)
    Phase3, // Tiers 5-6 (Oil & Quartz)
    Phase4, // Tiers 7-8 (Aluminum & Nuclear)
    Phase5, // Tier 9 (Quantum End-game)
}

impl GamePhase {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "phase1" | "1" | "early" => Some(GamePhase::Phase1),
            "phase2" | "2" | "steel" => Some(GamePhase::Phase2),
            "phase3" | "3" | "oil" => Some(GamePhase::Phase3),
            "phase4" | "4" | "late" | "nuclear" => Some(GamePhase::Phase4),
            "phase5" | "5" | "quantum" | "end" => Some(GamePhase::Phase5),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub fn to_str(self) -> &'static str {
        match self {
            GamePhase::Phase1 => "Phase 1 (Tiers 1-2: Early Game)",
            GamePhase::Phase2 => "Phase 2 (Tiers 3-4: Steel & Coal Power)",
            GamePhase::Phase3 => "Phase 3 (Tiers 5-6: Oil & Quartz)",
            GamePhase::Phase4 => "Phase 4 (Tiers 7-8: Aluminum & Nuclear)",
            GamePhase::Phase5 => "Phase 5 (Tier 9: Quantum End-game)",
        }
    }

    pub fn apply_weights(self, weights: &mut HashMap<String, f64>) {
        weights.clear();
        match self {
            GamePhase::Phase1 => {
                weights.insert("iron".to_string(), 1.0);
                weights.insert("copper".to_string(), 0.8);
                weights.insert("limestone".to_string(), 0.7);
                // Forward-looking: coal unlocked at Tier 3, proximity still valuable
                weights.insert("coal".to_string(), 0.2);
                weights.insert("caterium".to_string(), 0.2); // M.A.M. research value
                weights.insert("uranium".to_string(), -2.0); // severe radiation penalty (no hazmat suit)
                weights.insert("blueslug".to_string(), 0.10);
                weights.insert("yellowslug".to_string(), 0.15);
                weights.insert("purpleslug".to_string(), 0.20);
                weights.insert("mercer".to_string(), 0.15);
                weights.insert("somersloop".to_string(), 0.15);
                weights.insert("harddrive".to_string(), 0.25);
                weights.insert("paleberry".to_string(), 0.10);
                weights.insert("berylnut".to_string(), 0.08);
                weights.insert("baconagaric".to_string(), 0.12);
                weights.insert("sporeflower".to_string(), -0.01);
                weights.insert("gaspillar".to_string(), -0.02);
            }
            GamePhase::Phase2 => {
                weights.insert("iron".to_string(), 1.0);
                weights.insert("copper".to_string(), 0.8);
                weights.insert("limestone".to_string(), 0.7);
                weights.insert("coal".to_string(), 1.0);
                weights.insert("water".to_string(), 1.2); // Coal Power requires water
                weights.insert("caterium".to_string(), 0.4);
                // Black Powder (Nobelisk/ammo) requires sulfur + coal from Tier 3
                weights.insert("sulfur".to_string(), 0.3);
                // Crystal Oscillators for Computers (Tier 4) need quartz
                weights.insert("quartz".to_string(), 0.2);
                weights.insert("uranium".to_string(), -2.0); // radiation penalty (no hazmat suit)
                weights.insert("blueslug".to_string(), 0.08);
                weights.insert("yellowslug".to_string(), 0.12);
                weights.insert("purpleslug".to_string(), 0.18);
                weights.insert("mercer".to_string(), 0.12);
                weights.insert("somersloop".to_string(), 0.12);
                weights.insert("harddrive".to_string(), 0.20);
                weights.insert("paleberry".to_string(), 0.08);
                weights.insert("berylnut".to_string(), 0.06);
                weights.insert("baconagaric".to_string(), 0.10);
                weights.insert("sporeflower".to_string(), -0.008);
                weights.insert("gaspillar".to_string(), -0.015);
            }
            GamePhase::Phase3 => {
                weights.insert("iron".to_string(), 0.8);
                weights.insert("copper".to_string(), 0.8);
                weights.insert("limestone".to_string(), 0.6);
                weights.insert("coal".to_string(), 0.8);
                // Oil refinery chains (Heavy Oil Residue, Turbofuel) are extremely water-hungry
                weights.insert("water".to_string(), 0.9);
                weights.insert("oil".to_string(), 1.0); // Oil refinery focus
                weights.insert("sulfur".to_string(), 0.6);
                weights.insert("quartz".to_string(), 0.6);
                weights.insert("caterium".to_string(), 0.6);
                // Forward-looking: aluminium R&D begins transitioning here
                weights.insert("bauxite".to_string(), 0.3);
                weights.insert("uranium".to_string(), -2.0); // radiation penalty (no hazmat suit yet)
                weights.insert("blueslug".to_string(), 0.05);
                weights.insert("yellowslug".to_string(), 0.08);
                weights.insert("purpleslug".to_string(), 0.12);
                weights.insert("mercer".to_string(), 0.10);
                weights.insert("somersloop".to_string(), 0.10);
                weights.insert("harddrive".to_string(), 0.15);
                weights.insert("paleberry".to_string(), 0.05);
                weights.insert("berylnut".to_string(), 0.04);
                weights.insert("baconagaric".to_string(), 0.06);
                weights.insert("sporeflower".to_string(), -0.004);
                weights.insert("gaspillar".to_string(), -0.008);
            }
            GamePhase::Phase4 => {
                weights.insert("iron".to_string(), 0.6);
                weights.insert("copper".to_string(), 0.6);
                weights.insert("limestone".to_string(), 0.5);
                weights.insert("coal".to_string(), 0.6);
                weights.insert("water".to_string(), 1.2);
                weights.insert("oil".to_string(), 0.8);
                weights.insert("sulfur".to_string(), 0.8);
                weights.insert("quartz".to_string(), 0.8);
                weights.insert("caterium".to_string(), 0.8);
                weights.insert("bauxite".to_string(), 1.0); // Aluminum focus
                weights.insert("nitrogenwell".to_string(), 0.8);
                weights.insert("geyser".to_string(), 0.8);
                // Nuclear power is the primary goal at Tier 7-8; player has hazmat suit
                weights.insert("uranium".to_string(), 0.6);
                weights.insert("sam".to_string(), 0.6);
                weights.insert("blueslug".to_string(), 0.05);
                weights.insert("yellowslug".to_string(), 0.08);
                weights.insert("purpleslug".to_string(), 0.12);
                weights.insert("mercer".to_string(), 0.10);
                weights.insert("somersloop".to_string(), 0.10);
                weights.insert("harddrive".to_string(), 0.15);
                weights.insert("paleberry".to_string(), 0.02);
                weights.insert("berylnut".to_string(), 0.02);
                weights.insert("baconagaric".to_string(), 0.02);
                weights.insert("sporeflower".to_string(), -0.001);
                weights.insert("gaspillar".to_string(), -0.002);
            }
            GamePhase::Phase5 => {
                weights.insert("iron".to_string(), 0.5);
                weights.insert("copper".to_string(), 0.5);
                weights.insert("limestone".to_string(), 0.4);
                weights.insert("coal".to_string(), 0.5);
                weights.insert("water".to_string(), 0.5);
                weights.insert("oil".to_string(), 0.7);
                weights.insert("sulfur".to_string(), 0.8);
                weights.insert("quartz".to_string(), 0.8);
                weights.insert("caterium".to_string(), 0.8);
                weights.insert("bauxite".to_string(), 0.8);
                weights.insert("nitrogenwell".to_string(), 0.8);
                weights.insert("geyser".to_string(), 0.8);
                // Ficsonium production requires uranium; player has hazmat suit at this stage
                weights.insert("uranium".to_string(), 0.5);
                weights.insert("sam".to_string(), 1.0); // Quantum / Ficsonium focus
                weights.insert("blueslug".to_string(), 0.03);
                weights.insert("yellowslug".to_string(), 0.05);
                weights.insert("purpleslug".to_string(), 0.08);
                weights.insert("mercer".to_string(), 0.05);
                weights.insert("somersloop".to_string(), 0.05);
                weights.insert("harddrive".to_string(), 0.10);
                weights.insert("paleberry".to_string(), 0.01);
                weights.insert("berylnut".to_string(), 0.01);
                weights.insert("baconagaric".to_string(), 0.01);
                weights.insert("sporeflower".to_string(), -0.0005);
                weights.insert("gaspillar".to_string(), -0.001);
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceNode {
    #[serde(rename = "type")]
    pub resource_type: String,
    pub purity: Purity,
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub z: f64,
    #[serde(default)]
    pub obstructed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpawnLocation {
    pub name: &'static str,
    pub x: f64,
    pub y: f64,
    pub description: &'static str,
}

pub static DEFAULT_SPAWNS: &[SpawnLocation] = &[
    SpawnLocation {
        name: "Grass Fields",
        x: -110000.0,
        y: 240000.0,
        description: "Spacious, flat, mostly impure/normal nodes. Perfect for learning logistics.",
    },
    SpawnLocation {
        name: "Rocky Desert",
        x: -200000.0,
        y: -200000.0,
        description: "Balanced, flat, and spacious. Reliable access to iron, copper, and limestone.",
    },
    SpawnLocation {
        name: "Northern Forest",
        x: 0.0,
        y: -90000.0,
        description: "Lush, dense, and vertical. Exceptionally high density of Pure resource nodes.",
    },
    SpawnLocation {
        name: "Dune Desert",
        x: 240000.0,
        y: -210000.0,
        description: "Sprawling desert sand dunes. Tons of normal nodes but very sparse water and biomass.",
    },
];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SearchStrategy {
    Hybrid,
    Fast,
    Slow,
}

impl SearchStrategy {
    pub fn to_str(self) -> &'static str {
        match self {
            SearchStrategy::Hybrid => "Hybrid (recommended)",
            SearchStrategy::Fast => "Multi-Start (fast but less accurate)",
            SearchStrategy::Slow => "High-Res (slow but accurate)",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum UtilityFunction {
    CobbDouglas,
    Leontief,
    Linear,
}

impl UtilityFunction {
    pub fn to_str(self) -> &'static str {
        match self {
            UtilityFunction::CobbDouglas => "Cobb-Douglas (Balanced)",
            UtilityFunction::Leontief => "Leontief (Min-Bottleneck)",
            UtilityFunction::Linear => "Linear (Additive/Volume)",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DistanceDecay {
    Gaussian,
    Exponential,
    PowerLaw,
    Linear,
    LogisticStep,
}

impl DistanceDecay {
    pub fn to_str(self) -> &'static str {
        match self {
            DistanceDecay::Gaussian => "Gaussian (Smooth Drop)",
            DistanceDecay::Exponential => "Exponential (Linear Cost)",
            DistanceDecay::PowerLaw => "Power-Law (Heavy Tail)",
            DistanceDecay::Linear => "Linear (Hard Cutoff)",
            DistanceDecay::LogisticStep => "Logistic (Step Function / Trains)",
        }
    }
}

pub struct OptimizerConfig {
    pub sigma: f64, // Effective walking distance in meters
    pub weights: HashMap<String, f64>,
    pub purity_override: PurityOverride,
    pub strategy: SearchStrategy,
    pub utility_func: UtilityFunction,
    pub decay_func: DistanceDecay,
    pub game_phase: GamePhase,
    pub ignore_spawns: bool,
}

impl Default for OptimizerConfig {
    fn default() -> Self {
        let mut weights = HashMap::new();
        weights.insert("iron".to_string(), 1.0);
        weights.insert("copper".to_string(), 0.8);
        weights.insert("limestone".to_string(), 0.7);
        weights.insert("coal".to_string(), 0.9);
        weights.insert("caterium".to_string(), 0.3);
        weights.insert("oil".to_string(), 0.5);
        weights.insert("sulfur".to_string(), 0.2);
        weights.insert("quartz".to_string(), 0.4);
        weights.insert("bauxite".to_string(), 0.1);
        weights.insert("uranium".to_string(), 0.05);
        weights.insert("sam".to_string(), 0.05);

        // Collectibles / Research (slightly weighted to guide base selection)
        weights.insert("blueslug".to_string(), 0.05);
        weights.insert("yellowslug".to_string(), 0.08);
        weights.insert("purpleslug".to_string(), 0.12);
        weights.insert("mercer".to_string(), 0.10);
        weights.insert("somersloop".to_string(), 0.10);
        weights.insert("harddrive".to_string(), 0.15);
        weights.insert("paleberry".to_string(), 0.05);
        weights.insert("berylnut".to_string(), 0.05);
        weights.insert("baconagaric".to_string(), 0.05);
        weights.insert("sporeflower".to_string(), -0.008);
        weights.insert("gaspillar".to_string(), -0.015);

        Self {
            sigma: 700.0,
            weights,
            purity_override: PurityOverride::Default,
            strategy: SearchStrategy::Hybrid,
            utility_func: UtilityFunction::CobbDouglas,
            decay_func: DistanceDecay::Gaussian,
            game_phase: GamePhase::Phase1,
            ignore_spawns: false,
        }
    }
}
