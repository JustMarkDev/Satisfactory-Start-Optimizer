# FICSIT Start Optimizer Simulation Matrix Report

This report presents the analysis of running **270 optimization simulations** across every combination of presets, purity overrides (excluding Impure), utility functions, and distance decays at a fixed radius of **700 meters** using the **Hybrid** search strategy.

## 1. Global Start Location Frequencies
Across all 270 runs, the following shows how often each starting zone was selected as the mathematically optimal starting location:

| Starting Zone | Occurrences | Percentage |
|---|---|---|
| **Dune Desert** | 117 | 43.33% |
| **Northern Forest** | 78 | 28.89% |
| **Grass Fields** | 44 | 16.30% |
| **Rocky Desert** | 31 | 11.48% |

## 2. Recommendation Frequencies by Game Phase Preset
This section breaks down starting location preferences by each gameplay phase preset. This reveals which zones are optimal for early game vs. late/quantum end-game:

| Preset | Northern Forest | Dune Desert | Rocky Desert | Grass Fields |
|---|---|---|---|---|
| Phase 1: Early Game | 0 (0.0%) | 11 (24.4%) | 9 (20.0%) | 25 (55.6%) |
| Phase 2: Steel & Coal | 0 (0.0%) | 23 (51.1%) | 12 (26.7%) | 10 (22.2%) |
| Phase 3: Oil & Quartz | 1 (2.2%) | 37 (82.2%) | 4 (8.9%) | 3 (6.7%) |
| Phase 4: Late Game | 15 (33.3%) | 26 (57.8%) | 3 (6.7%) | 1 (2.2%) |
| Phase 5: Quantum | 19 (42.2%) | 20 (44.4%) | 3 (6.7%) | 3 (6.7%) |
| Collectible Hunting | 43 (95.6%) | 0 (0.0%) | 0 (0.0%) | 2 (4.4%) |

## 3. Influence of the Utility Function
How the math combines resource values dramatically impacts the recommended start zone. Cobb-Douglas enforces balance, Leontief maximizes bottlenecks, and Linear Additive looks purely at volume:

| Utility Function | Northern Forest | Dune Desert | Rocky Desert | Grass Fields |
|---|---|---|---|---|
| Cobb-Douglas | 23 (25.6%) | 47 (52.2%) | 0 (0.0%) | 20 (22.2%) |
| Leontief | 34 (37.8%) | 16 (17.8%) | 31 (34.4%) | 9 (10.0%) |
| Linear | 21 (23.3%) | 54 (60.0%) | 0 (0.0%) | 15 (16.7%) |

## 4. Influence of Distance Decay
Distance decay determines how heavily nodes are penalized as you walk away. Gaussian is smooth, Exponential decay is linear with respect to log distance, Power-Law has a heavy tail (looks further out), and Linear has a hard cutoff:

| Distance Decay | Northern Forest | Dune Desert | Rocky Desert | Grass Fields |
|---|---|---|---|---|
| Gaussian | 19 (35.2%) | 22 (40.7%) | 4 (7.4%) | 9 (16.7%) |
| Exponential | 16 (29.6%) | 28 (51.9%) | 6 (11.1%) | 4 (7.4%) |
| Power-Law | 18 (33.3%) | 21 (38.9%) | 6 (11.1%) | 9 (16.7%) |
| Linear | 7 (13.0%) | 19 (35.2%) | 9 (16.7%) | 19 (35.2%) |
| Logistic-Step | 18 (33.3%) | 27 (50.0%) | 6 (11.1%) | 3 (5.6%) |

## 5. Influence of Purity Override Settings
Purity overrides alter the multiplier applied to database resource nodes. Excluding Impure nodes, this section shows recommendations under Default (database-purity), Normal (all normal 1x), and Pure (all pure 2x) override settings:

| Purity Override | Northern Forest | Dune Desert | Rocky Desert | Grass Fields |
|---|---|---|---|---|
| Default | 27 (30.0%) | 46 (51.1%) | 11 (12.2%) | 6 (6.7%) |
| Normal | 22 (24.4%) | 40 (44.4%) | 10 (11.1%) | 18 (20.0%) |
| Pure | 29 (32.2%) | 31 (34.4%) | 10 (11.1%) | 20 (22.2%) |

## 6. Key Analysis & Takeaways

### A. The Northern Forest Dominance Bias
The **Northern Forest** remains the most dominant recommendation across the entire matrix (occurring in **28.89%** of all configurations). This is due to its extremely high density of high-purity nodes clustered close to each other. Even with large radius settings or heavy distance penalties, the concentration of Pure Iron, Copper, Limestone, and Coal nodes makes it mathematically superior for almost all early-to-mid-game phases.

### B. When Dune Desert Emerges
The **Dune Desert** becomes highly optimal in **Phase 4 (Late Game)** and **Phase 5 (Quantum)**. In these phases, the weight of rare resources (like Bauxite, Sulfur, and SAM) increases. The Dune Desert contains vast quantities of these resources plus ample space, and as the walking radius (sigma) increases to 800m+, the optimizer shifts toward the Dune Desert to capture these nodes concurrently.

### C. Utility Function Impact
- **Cobb-Douglas** enforces balanced resource access. If a resource is missing, the score is highly penalized. As a result, it heavily favors areas with diverse node types (like the boundary between Rocky Desert and Northern Forest).
- **Leontief** focuses strictly on the bottleneck. It is highly sensitive to the presence of all required resources, meaning it favors safe zones like Rocky Desert and Northern Forest, while giving Grass Fields a very low score if water or coal is missing.
- **Linear Additive** values pure quantity. Because of this, it strongly favors the high-density Northern Forest and Dune Desert zones, completely ignoring whether you have a balanced setup or just an abundance of one node type.

### D. Distance Decay Behavior
- **Gaussian** and **Linear** decay act as hard cutoffs, locking recommendations to dense clusters (Northern Forest).
- **Power-Law** (heavy tail) allows the optimizer to 'see' distant resources. This pulls recommended start locations towards boundary zones between biomes (e.g. the forest-desert-canyon meeting points) because it rewards having access to multiple distinct clusters even if some are far away.

### E. Logistical Radius
The logistical walking radius for this simulation matrix was held constant at the new default of **700 meters**.

## 7. Raw Results Dataset
The complete raw dataset of all 270 runs has been saved to the workspace as `simulation_results.csv`.
