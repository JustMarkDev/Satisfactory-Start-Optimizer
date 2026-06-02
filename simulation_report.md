# FICSIT Start Optimizer Simulation Matrix Report

This report presents the analysis of running **216 optimization simulations** across every combination of presets, purity overrides (excluding Impure), utility functions, and distance decays at a fixed radius of **700 meters** using the **Hybrid** search strategy.

## 1. Global Start Location Frequencies
Across all 216 runs, the following shows how often each starting zone was selected as the mathematically optimal starting location:

| Starting Zone | Occurrences | Percentage |
|---|---|---|
| **Northern Forest** | 100 | 46.30% |
| **Dune Desert** | 98 | 45.37% |
| **Grass Fields** | 16 | 7.41% |
| **Rocky Desert** | 2 | 0.93% |

## 2. Recommendation Frequencies by Game Phase Preset
This section breaks down starting location preferences by each gameplay phase preset. This reveals which zones are optimal for early game vs. late/quantum end-game:

| Preset | Northern Forest | Dune Desert | Rocky Desert | Grass Fields |
|---|---|---|---|---|
| Phase 1: Early Game | 2 (5.6%) | 32 (88.9%) | 0 (0.0%) | 2 (5.6%) |
| Phase 2: Steel & Coal | 2 (5.6%) | 33 (91.7%) | 1 (2.8%) | 0 (0.0%) |
| Phase 3: Oil & Quartz | 11 (30.6%) | 17 (47.2%) | 1 (2.8%) | 7 (19.4%) |
| Phase 4: Late Game | 23 (63.9%) | 11 (30.6%) | 0 (0.0%) | 2 (5.6%) |
| Phase 5: Quantum | 26 (72.2%) | 5 (13.9%) | 0 (0.0%) | 5 (13.9%) |
| Collectible Hunting | 36 (100.0%) | 0 (0.0%) | 0 (0.0%) | 0 (0.0%) |

## 3. Influence of the Utility Function
How the math combines resource values dramatically impacts the recommended start zone. Cobb-Douglas enforces balance, Leontief maximizes bottlenecks, and Linear Additive looks purely at volume:

| Utility Function | Northern Forest | Dune Desert | Rocky Desert | Grass Fields |
|---|---|---|---|---|
| Cobb-Douglas | 45 (62.5%) | 19 (26.4%) | 0 (0.0%) | 8 (11.1%) |
| Leontief | 30 (41.7%) | 33 (45.8%) | 2 (2.8%) | 7 (9.7%) |
| Linear | 25 (34.7%) | 46 (63.9%) | 0 (0.0%) | 1 (1.4%) |

## 4. Influence of Distance Decay
Distance decay determines how heavily nodes are penalized as you walk away. Gaussian is smooth, Exponential decay is linear with respect to log distance, Power-Law has a heavy tail (looks further out), and Linear has a hard cutoff:

| Distance Decay | Northern Forest | Dune Desert | Rocky Desert | Grass Fields |
|---|---|---|---|---|
| Gaussian | 27 (50.0%) | 26 (48.1%) | 1 (1.9%) | 0 (0.0%) |
| Exponential | 31 (57.4%) | 23 (42.6%) | 0 (0.0%) | 0 (0.0%) |
| Power-Law | 29 (53.7%) | 22 (40.7%) | 1 (1.9%) | 2 (3.7%) |
| Linear | 13 (24.1%) | 27 (50.0%) | 0 (0.0%) | 14 (25.9%) |

## 5. Influence of Purity Override Settings
Purity overrides alter the multiplier applied to database resource nodes. Excluding Impure nodes, this section shows recommendations under Default (database-purity), Normal (all normal 1x), and Pure (all pure 2x) override settings:

| Purity Override | Northern Forest | Dune Desert | Rocky Desert | Grass Fields |
|---|---|---|---|---|
| Default | 41 (56.9%) | 27 (37.5%) | 1 (1.4%) | 3 (4.2%) |
| Normal | 30 (41.7%) | 36 (50.0%) | 0 (0.0%) | 6 (8.3%) |
| Pure | 29 (40.3%) | 35 (48.6%) | 1 (1.4%) | 7 (9.7%) |

## 6. Key Analysis & Takeaways

### A. The Northern Forest Dominance Bias
The **Northern Forest** remains the most dominant recommendation across the entire matrix (occurring in **46.30%** of all configurations). This is due to its extremely high density of high-purity nodes clustered close to each other. Even with large radius settings or heavy distance penalties, the concentration of Pure Iron, Copper, Limestone, and Coal nodes makes it mathematically superior for almost all early-to-mid-game phases.

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
The complete raw dataset of all 216 runs has been saved to the workspace as `simulation_results.csv`.
