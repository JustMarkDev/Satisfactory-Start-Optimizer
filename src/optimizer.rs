use crate::models::{DEFAULT_SPAWNS, OptimizerConfig, ResourceNode, SpawnLocation};
use rayon::prelude::*;
use std::collections::HashMap;

pub const MIN_X: f64 = -320000.0;
pub const MAX_X: f64 = 420000.0;
pub const MIN_Y: f64 = -370000.0;
pub const MAX_Y: f64 = 370000.0;

#[derive(Debug, Clone, serde::Serialize)]
pub struct OptimizationResult {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub score: f64,
    pub closest_spawn: SpawnLocation,
    pub spawn_distance: f64,
    /// Counts of accessible (non-obstructed) local nodes, keyed by "Purity Type"
    pub local_nodes: HashMap<String, u32>,
    /// Counts of obstructed local nodes (require Nobelisk) keyed by "Purity Type"
    pub obstructed_nodes: HashMap<String, u32>,
    /// Decay-weighted yield per resource type (the values the utility function actually used)
    pub resource_yields: HashMap<String, f64>,
    /// Terrain Ruggedness Index: mean absolute Z-difference to spatial neighbours (metres)
    pub terrain_ruggedness: f64,
    /// Shannon entropy diversity of resource yields (higher = more balanced)
    pub diversity_score: f64,
}

#[derive(Debug, Clone, Copy)]
struct OptNode {
    x: f64,
    y: f64,
    z: f64,
    res_idx: usize,
    multiplier: f64,
    obstructed: bool,
}

struct SpatialGrid {
    bucket_size: f64,
    cols: usize,
    rows: usize,
    min_x: f64,
    min_y: f64,
    buckets: Vec<Vec<usize>>, // Indices of nodes in OptNode list
}

impl SpatialGrid {
    fn new(nodes: &[OptNode], bucket_size: f64) -> Self {
        let min_x = MIN_X;
        let min_y = MIN_Y;
        let max_x = MAX_X;
        let max_y = MAX_Y;

        let cols = (((max_x - min_x) / bucket_size).ceil() as usize).max(1);
        let rows = (((max_y - min_y) / bucket_size).ceil() as usize).max(1);

        let mut buckets = vec![Vec::new(); cols * rows];

        for (idx, node) in nodes.iter().enumerate() {
            let col =
                (((node.x - min_x) / bucket_size) as isize).clamp(0, cols as isize - 1) as usize;
            let row =
                (((node.y - min_y) / bucket_size) as isize).clamp(0, rows as isize - 1) as usize;
            buckets[row * cols + col].push(idx);
        }

        Self {
            bucket_size,
            cols,
            rows,
            min_x,
            min_y,
            buckets,
        }
    }
}

/// Helper that calculates the minimum distance in metres from (x, y) to any
/// static water body. The old "coast edge" approximations (e.g. x < -250000 → dist=0)
/// have been REMOVED: those regions are the map's impassable mountain walls, not
/// accessible ocean. Treating them as free water caused the optimizer to inflate
/// scores at the western/northern map boundary and produce border-edge results.
fn distance_to_nearest_water(
    x: f64,
    y: f64,
    opt_nodes: &[OptNode],
    waterwell_idx: Option<usize>,
) -> f64 {
    let mut min_dist_cm = f64::MAX;

    // Major static bodies of water in Satisfactory (centres, half-widths, in cm)
    let water_bodies = [
        (140000.0, 230000.0, 80000.0, 60000.0), // Great Blue Crater Lake (South-East)
        (-70000.0, -145000.0, 60000.0, 70000.0), // Crater Lakes / Coal Lakes (Northern Forest)
        (-60000.0, 65000.0, 80000.0, 70000.0),  // Red Jungle Lakes
        (45000.0, -20000.0, 30000.0, 30000.0),  // Lake Forest Lake
        (350000.0, 225000.0, 140000.0, 150000.0), // Eastern Swamp Lakes
        (310000.0, -185000.0, 25000.0, 20000.0), // Southern Dune Desert pond cluster
        (290000.0, -230000.0, 20000.0, 15000.0), // Far-south Dune Desert pond
        (355000.0, -80000.0, 30000.0, 25000.0),  // Central-east Dune Desert oasis
    ];

    for &(cx, cy, w, h) in &water_bodies {
        let dx = (x - cx).abs() - w / 2.0;
        let dy = (y - cy).abs() - h / 2.0;
        let dist = if dx > 0.0 && dy > 0.0 {
            (dx * dx + dy * dy).sqrt()
        } else if dx > 0.0 {
            dx
        } else if dy > 0.0 {
            dy
        } else {
            0.0 // Inside the water body bounds
        };
        if dist < min_dist_cm {
            min_dist_cm = dist;
        }
    }

    let mut min_dist_m = min_dist_cm / 100.0;

    // Add dynamic waterwell checks
    if let Some(ww_idx) = waterwell_idx {
        for node in opt_nodes {
            if node.res_idx == ww_idx {
                let dx = (x - node.x) / 100.0;
                let dy = (y - node.y) / 100.0;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist < min_dist_m {
                    min_dist_m = dist;
                }
            }
        }
    }

    min_dist_m
}

/// Estimates the ground altitude Z at a coordinate (x, y) using KNN IDW (Inverse Distance Weighting)
fn estimate_altitude(
    x: f64,
    y: f64,
    opt_nodes: &[OptNode],
    spatial_grid: &SpatialGrid,
    sigma: f64,
) -> f64 {
    let radius = 3.5 * sigma;
    let radius_cm = radius * 100.0;
    let radius_cm_sq = radius_cm * radius_cm;

    let mut nearest = [(f64::MAX, 0.0); 6]; // (dist_sq, z)
    let mut count = 0;

    let min_qx = x - radius_cm;
    let max_qx = x + radius_cm;
    let min_qy = y - radius_cm;
    let max_qy = y + radius_cm;

    let col_start = (((min_qx - spatial_grid.min_x) / spatial_grid.bucket_size) as isize)
        .clamp(0, spatial_grid.cols as isize - 1) as usize;
    let col_end = (((max_qx - spatial_grid.min_x) / spatial_grid.bucket_size) as isize)
        .clamp(0, spatial_grid.cols as isize - 1) as usize;
    let row_start = (((min_qy - spatial_grid.min_y) / spatial_grid.bucket_size) as isize)
        .clamp(0, spatial_grid.rows as isize - 1) as usize;
    let row_end = (((max_qy - spatial_grid.min_y) / spatial_grid.bucket_size) as isize)
        .clamp(0, spatial_grid.rows as isize - 1) as usize;

    for r_idx in row_start..=row_end {
        for c_idx in col_start..=col_end {
            let bucket_idx = r_idx * spatial_grid.cols + c_idx;
            for &node_idx in &spatial_grid.buckets[bucket_idx] {
                let node = &opt_nodes[node_idx];
                let dx = x - node.x;
                let dy = y - node.y;
                let dist_sq = dx * dx + dy * dy;
                if dist_sq <= radius_cm_sq {
                    if dist_sq < nearest[5].0 {
                        let mut insert_pos = 5;
                        while insert_pos > 0 && dist_sq < nearest[insert_pos - 1].0 {
                            insert_pos -= 1;
                        }
                        for idx in (insert_pos + 1..6).rev() {
                            nearest[idx] = nearest[idx - 1];
                        }
                        nearest[insert_pos] = (dist_sq, node.z);
                        if count < 6 {
                            count += 1;
                        }
                    }
                }
            }
        }
    }

    if count > 0 {
        let mut total_weight = 0.0;
        let mut weighted_z = 0.0;
        for i in 0..count {
            let dist_m = nearest[i].0.sqrt() / 100.0;
            let w = 1.0 / ((dist_m + 10.0) * (dist_m + 10.0));
            weighted_z += nearest[i].1 * w;
            total_weight += w;
        }
        weighted_z / total_weight
    } else {
        0.0
    }
}

const LAND_MASK_SECTORS: usize = 48;
const LAND_MASK_BUFFER_CM: f64 = 22_000.0; // ≈ 30 map pixels / 220 m.
const MAP_PIXEL_TO_CM: f64 = 1.0 / 0.0013653321;

#[derive(Debug, Clone)]
struct LandMask {
    points: Vec<(f64, f64)>,
}

impl LandMask {
    fn from_nodes(nodes: &[OptNode]) -> Self {
        if nodes.len() < 3 {
            return Self {
                points: vec![
                    (MIN_X, MIN_Y),
                    (MAX_X, MIN_Y),
                    (MAX_X, MAX_Y),
                    (MIN_X, MAX_Y),
                ],
            };
        }

        let (sum_x, sum_y) = nodes
            .iter()
            .fold((0.0, 0.0), |(sx, sy), n| (sx + n.x, sy + n.y));
        let center_x = sum_x / nodes.len() as f64;
        let center_y = sum_y / nodes.len() as f64;

        let mut sectors: Vec<Option<(f64, f64, f64)>> = vec![None; LAND_MASK_SECTORS];
        for node in nodes {
            let dx = node.x - center_x;
            let dy = node.y - center_y;
            let dist_sq = dx * dx + dy * dy;
            if dist_sq == 0.0 {
                continue;
            }
            let angle = dy.atan2(dx);
            let normalized = (angle + std::f64::consts::PI) / (2.0 * std::f64::consts::PI);
            let idx = ((normalized * LAND_MASK_SECTORS as f64).floor() as usize)
                .min(LAND_MASK_SECTORS - 1);

            if sectors[idx].map_or(true, |(_, _, best_dist_sq)| dist_sq > best_dist_sq) {
                sectors[idx] = Some((node.x, node.y, dist_sq));
            }
        }

        let mut points = Vec::new();
        for point in sectors.into_iter().flatten() {
            let dx = point.0 - center_x;
            let dy = point.1 - center_y;
            let dist = (dx * dx + dy * dy).sqrt();
            let buffer = LAND_MASK_BUFFER_CM.max(30.0 * MAP_PIXEL_TO_CM);
            let out_x = point.0 + dx / dist * buffer;
            let out_y = point.1 + dy / dist * buffer;
            points.push((out_x.clamp(MIN_X, MAX_X), out_y.clamp(MIN_Y, MAX_Y)));
        }

        Self { points }
    }
}

/// Ray-casting point-in-polygon test.
#[inline]
fn is_in_polygon(x: f64, y: f64, vs: &[(f64, f64)]) -> bool {
    let n = vs.len();
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = vs[i];
        let (xj, yj) = vs[j];
        if ((yi > y) != (yj > y)) && (x < (xj - xi) * (y - yi) / (yj - yi) + xi) {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Minimum distance from (x, y) to the nearest edge of the buildable-land polygon (in cm).
#[inline]
fn dist_to_polygon_edge(x: f64, y: f64, vs: &[(f64, f64)]) -> f64 {
    let n = vs.len();
    let mut min_dist = f64::MAX;
    let mut j = n - 1;
    for i in 0..n {
        let (ax, ay) = vs[j];
        let (bx, by) = vs[i];
        let dx = bx - ax;
        let dy = by - ay;
        let len_sq = dx * dx + dy * dy;
        let t = if len_sq > 0.0 {
            (((x - ax) * dx + (y - ay) * dy) / len_sq).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let proj_x = ax + t * dx;
        let proj_y = ay + t * dy;
        let ddx = x - proj_x;
        let ddy = y - proj_y;
        let dist = (ddx * ddx + ddy * ddy).sqrt();
        if dist < min_dist {
            min_dist = dist;
        }
        j = i;
    }
    min_dist
}

/// Calculates the dynamic multi-resource Cobb-Douglas utility function at a coordinate (x, y)
/// using spatial grid bucketing, KNN elevation, terrain flatness, water proximity, and radiation.
fn calculate_utility(
    x: f64,
    y: f64,
    opt_nodes: &[OptNode],
    spatial_grid: &SpatialGrid,
    config: &OptimizerConfig,
    num_resources: usize,
    weights_arr: &[f64],
    epsilons_arr: &[f64],
    res_to_idx: &HashMap<String, usize>,
    waterwell_idx: Option<usize>,
    land_mask: &LandMask,
) -> f64 {
    // Reject points outside the practical landmass polygon.
    if !is_in_polygon(x, y, &land_mask.points) {
        return 0.0;
    }

    // Border margin penalty: apply a soft penalty within 300 m of the polygon edge
    // to prevent the optimizer locking onto border corners where the Cobb-Douglas
    // epsilon floor creates false local maxima. 300 m is enough to push away from
    // the actual polygon boundary while not affecting real interior areas (all
    // starting zones are at least 1.2 km from the nearest polygon edge).
    const BORDER_MARGIN_CM: f64 = 30_000.0; // 300 m
    let border_dist = dist_to_polygon_edge(x, y, &land_mask.points);
    let border_penalty = if border_dist < BORDER_MARGIN_CM {
        (-4.0 * (1.0 - border_dist / BORDER_MARGIN_CM)).exp()
    } else {
        1.0
    };
    if border_penalty < 0.01 {
        return 0.0;
    }

    let radius = 3.5 * config.sigma;
    let radius_cm = radius * 100.0;
    let radius_m_sq = radius * radius;

    let min_qx = x - radius_cm;
    let max_qx = x + radius_cm;
    let min_qy = y - radius_cm;
    let max_qy = y + radius_cm;

    let col_start = (((min_qx - spatial_grid.min_x) / spatial_grid.bucket_size) as isize)
        .clamp(0, spatial_grid.cols as isize - 1) as usize;
    let col_end = (((max_qx - spatial_grid.min_x) / spatial_grid.bucket_size) as isize)
        .clamp(0, spatial_grid.cols as isize - 1) as usize;
    let row_start = (((min_qy - spatial_grid.min_y) / spatial_grid.bucket_size) as isize)
        .clamp(0, spatial_grid.rows as isize - 1) as usize;
    let row_end = (((max_qy - spatial_grid.min_y) / spatial_grid.bucket_size) as isize)
        .clamp(0, spatial_grid.rows as isize - 1) as usize;

    // 1. KNN IDW Ground Height Estimation
    let z = estimate_altitude(x, y, opt_nodes, spatial_grid, config.sigma);

    // 2. Sum yields for all resource types & collect local heights for terrain flatness
    let mut yields = [0.0; 128];

    let build_radius = 1.5 * config.sigma; // factory building area footprint
    let build_radius_m_sq = build_radius * build_radius;

    // Welford's algorithm variables for online variance calculation
    let mut heights_count = 0;
    let mut heights_mean = 0.0;
    let mut heights_m2 = 0.0;

    for r_idx in row_start..=row_end {
        for c_idx in col_start..=col_end {
            let bucket_idx = r_idx * spatial_grid.cols + c_idx;
            for &node_idx in &spatial_grid.buckets[bucket_idx] {
                let node = &opt_nodes[node_idx];
                let dx = (x - node.x) / 100.0;
                let dy = (y - node.y) / 100.0;
                let dz = (z - node.z) / 100.0;

                let vertical_multiplier = 4.0;
                let d_sq =
                    dx * dx + dy * dy + (dz * dz * vertical_multiplier * vertical_multiplier);

                if d_sq <= radius_m_sq {
                    let d = d_sq.sqrt();
                    let decay = match config.decay_func {
                        crate::models::DistanceDecay::Gaussian => {
                            let two_sigma_sq = 2.0 * config.sigma * config.sigma;
                            (-d_sq / two_sigma_sq).exp()
                        }
                        crate::models::DistanceDecay::Exponential => (-d / config.sigma).exp(),
                        crate::models::DistanceDecay::PowerLaw => 1.0 / (d / config.sigma + 1.0),
                        crate::models::DistanceDecay::Linear => (1.0 - d / config.sigma).max(0.0),
                        crate::models::DistanceDecay::LogisticStep => {
                            if d <= config.sigma {
                                1.0
                            } else {
                                0.05
                            }
                        }
                    };
                    let mut contribution = node.multiplier * decay;

                    // Obstructed nodes are locked behind Nobelisk explosives (Phase 1 & 2)
                    if node.obstructed
                        && (config.game_phase == crate::models::GamePhase::Phase1
                            || config.game_phase == crate::models::GamePhase::Phase2)
                    {
                        contribution = 0.0;
                    }

                    yields[node.res_idx] += contribution;

                    // Keep track of nearby heights using Welford's algorithm
                    if d_sq <= build_radius_m_sq {
                        heights_count += 1;
                        let delta = node.z - heights_mean;
                        heights_mean += delta / heights_count as f64;
                        let delta2 = node.z - heights_mean;
                        heights_m2 += delta * delta2;
                    }
                }
            }
        }
    }

    // Add virtual water yield based on proximity to mapped lakes/ponds or waterwells.
    if let Some(&water_idx) = res_to_idx.get("water") {
        let water_dist = distance_to_nearest_water(x, y, opt_nodes, waterwell_idx);
        let water_decay = match config.decay_func {
            crate::models::DistanceDecay::Gaussian => {
                let two_sigma_sq = 2.0 * config.sigma * config.sigma;
                (-(water_dist * water_dist) / two_sigma_sq).exp()
            }
            crate::models::DistanceDecay::Exponential => (-water_dist / config.sigma).exp(),
            crate::models::DistanceDecay::PowerLaw => 1.0 / (water_dist / config.sigma + 1.0),
            crate::models::DistanceDecay::Linear => (1.0 - water_dist / config.sigma).max(0.0),
            crate::models::DistanceDecay::LogisticStep => {
                if water_dist <= config.sigma {
                    1.0
                } else {
                    0.05
                }
            }
        };
        yields[water_idx] = water_decay;
    }

    // 3. Terrain Flatness Penalty (Population std dev of local node heights)
    // Scale denominator of 20m: a 20m std dev halves the score (e⁻¹ ≈ 36.8%).
    // Using 20 instead of 40 ensures mountainous terrain (e.g. Northern Forest,
    // ~50-150m std dev) is meaningfully penalised vs. flat biomes (Grass Fields ~5-15m).
    let mut flatness_mult = 1.0;
    if heights_count > 1 {
        let std_dev_cm = (heights_m2 / heights_count as f64).sqrt();
        let std_dev_m = std_dev_cm / 100.0;
        flatness_mult = (-std_dev_m / 30.0).exp();
    }

    // Gravity / Clustering Bonus: Increase yield non-linearly if multiple nodes are nearby
    for i in 0..num_resources {
        if yields[i] > 1.0 {
            yields[i] *= 1.0 + 0.1 * (yields[i] - 1.0);
        }
    }

    // 4. Combined Utility + Radiation/Threat Penalties
    //
    // Cobb-Douglas: normalise exponents to sum to 1.0 to enforce constant returns to scale.
    // Without normalisation, adding more resources inflates scores non-linearly, making
    // cross-phase comparisons meaningless (Phase 4 with 14 active weights >> Phase 1 with 4).
    let mut score = match config.utility_func {
        crate::models::UtilityFunction::CobbDouglas => {
            let weight_sum: f64 = weights_arr[..num_resources]
                .iter()
                .filter(|&&w| w > 0.0)
                .sum();
            let norm = if weight_sum > 0.0 { weight_sum } else { 1.0 };
            let mut s = 1.0;
            for i in 0..num_resources {
                let weight = weights_arr[i];
                if weight > 0.0 {
                    let res_yield = yields[i];
                    let eps = epsilons_arr[i];
                    // Normalised exponent: weight_i / Σ(positive_weights)
                    s *= (res_yield + eps).powf(weight / norm);
                }
            }
            s
        }
        crate::models::UtilityFunction::Leontief => {
            let mut s = f64::MAX;
            let mut has_pos = false;
            for i in 0..num_resources {
                let weight = weights_arr[i];
                if weight > 0.0 {
                    has_pos = true;
                    let res_yield = yields[i];
                    let eps = epsilons_arr[i];
                    let val = (res_yield + eps) / weight;
                    if val < s {
                        s = val;
                    }
                }
            }
            if has_pos { s } else { 0.0 }
        }
        crate::models::UtilityFunction::Linear => {
            let mut s = 0.0;
            for i in 0..num_resources {
                let weight = weights_arr[i];
                if weight > 0.0 {
                    s += yields[i] * weight;
                }
            }
            s
        }
    };

    // Apply threat penalties (negative weights)
    for i in 0..num_resources {
        let weight = weights_arr[i];
        if weight < 0.0 {
            let res_yield = yields[i];
            let penalty_factor = 1.0 + res_yield * weight.abs();
            score /= penalty_factor;
        }
    }

    // Phase-scaled spawn proximity penalty.
    // Early-game players cannot travel far from their drop pod on foot. This penalty
    // prevents the optimizer from recommending a remote "ideal" base location that
    // requires a vehicle to reach from spawn — which the player won't have in Phase 1.
    //
    // tolerance_m: the 1/e distance at which spawn distance is penalised 63%.
    //   Phase 1: 800 m  — strict (foot travel only)
    //   Phase 2: 1500 m — lenient (player likely has a vehicle)
    //   Phase 3: 3000 m — relaxed (trains/trucks operational)
    //   Phase 4+: no penalty (helicopters, transport network established)
    let spawn_tolerance_m: Option<f64> = if config.ignore_spawns {
        None
    } else {
        match config.game_phase {
            crate::models::GamePhase::Phase1 => Some(800.0),
            crate::models::GamePhase::Phase2 => Some(1500.0),
            crate::models::GamePhase::Phase3 => Some(3000.0),
            crate::models::GamePhase::Phase4 | crate::models::GamePhase::Phase5 => None,
        }
    };

    if let Some(tol) = spawn_tolerance_m {
        let mut min_spawn_dist_m = f64::MAX;
        for spawn in DEFAULT_SPAWNS {
            let dx = (x - spawn.x) / 100.0;
            let dy = (y - spawn.y) / 100.0;
            let d = (dx * dx + dy * dy).sqrt();
            let boundary_dist = (d - spawn.radius).max(0.0);
            if boundary_dist < min_spawn_dist_m {
                min_spawn_dist_m = boundary_dist;
            }
        }
        // Soft penalty: exp(-dist/tolerance). At dist=0 → 1.0; at dist=tol → e⁻¹ ≈ 0.37
        let spawn_penalty = (-min_spawn_dist_m / tol).exp();
        score *= spawn_penalty;
    }

    score * flatness_mult * border_penalty
}

/// Helper that runs hill climbing starting from a given coordinate (start_x, start_y)
fn run_hill_climbing(
    start_x: f64,
    start_y: f64,
    opt_nodes: &[OptNode],
    spatial_grid: &SpatialGrid,
    config: &OptimizerConfig,
    num_resources: usize,
    weights_arr: &[f64],
    epsilons_arr: &[f64],
    res_to_idx: &HashMap<String, usize>,
    waterwell_idx: Option<usize>,
    land_mask: &LandMask,
) -> OptimizationResult {
    let mut curr_x = start_x;
    let mut curr_y = start_y;

    let mut step = 10000.0;
    let tolerance = 10.0;

    let mut max_score = calculate_utility(
        curr_x,
        curr_y,
        opt_nodes,
        spatial_grid,
        config,
        num_resources,
        weights_arr,
        epsilons_arr,
        res_to_idx,
        waterwell_idx,
        land_mask,
    );

    let dirs = [
        (1.0, 0.0),
        (-1.0, 0.0),
        (0.0, 1.0),
        (0.0, -1.0),
        (0.7071, 0.7071),
        (-0.7071, 0.7071),
        (0.7071, -0.7071),
        (-0.7071, -0.7071),
    ];

    while step > tolerance {
        let mut best_neighbor_score = max_score;
        let mut next_x = curr_x;
        let mut next_y = curr_y;

        for &(dx, dy) in &dirs {
            let tx = curr_x + dx * step;
            let ty = curr_y + dy * step;

            if tx < MIN_X || tx > MAX_X || ty < MIN_Y || ty > MAX_Y {
                continue;
            }

            let score = calculate_utility(
                tx,
                ty,
                opt_nodes,
                spatial_grid,
                config,
                num_resources,
                weights_arr,
                epsilons_arr,
                res_to_idx,
                waterwell_idx,
                land_mask,
            );
            if score > best_neighbor_score {
                best_neighbor_score = score;
                next_x = tx;
                next_y = ty;
            }
        }

        if best_neighbor_score > max_score {
            max_score = best_neighbor_score;
            curr_x = next_x;
            curr_y = next_y;
        } else {
            step *= 0.5;
        }
    }

    // Re-estimate final base altitude Z using KNN IDW
    let final_z = estimate_altitude(curr_x, curr_y, opt_nodes, spatial_grid, config.sigma);

    // Find closest starting spawn point
    let mut closest_spawn = DEFAULT_SPAWNS[0].clone();
    let mut min_dist = f64::MAX;

    for spawn in DEFAULT_SPAWNS {
        let dx = curr_x - spawn.x;
        let dy = curr_y - spawn.y;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist < min_dist {
            min_dist = dist;
            closest_spawn = spawn.clone();
        }
    }

    // Build inverse resource map for display names
    let mut inv_res_map: HashMap<usize, String> = HashMap::new();
    for (k, v) in res_to_idx {
        inv_res_map.insert(*v, k.clone());
    }

    let search_radius_sq = (config.sigma * 100.0) * (config.sigma * 100.0);

    let mut local_nodes: HashMap<String, u32> = HashMap::new();
    let mut obstructed_nodes: HashMap<String, u32> = HashMap::new();

    // Per-resource weighted yield totals (for output display)
    let mut resource_yields: HashMap<String, f64> = HashMap::new();

    // Terrain Ruggedness Index: collect Z values of all nodes within sigma for TRI
    // TRI = mean(|Z_neighbour - Z_centre|) over nearby nodes
    let mut tri_sum = 0.0;
    let mut tri_count = 0usize;
    let tri_radius_sq = (config.sigma * 0.5 * 100.0) * (config.sigma * 0.5 * 100.0); // 0.5σ neighbourhood

    for node in opt_nodes {
        let dx = curr_x - node.x;
        let dy = curr_y - node.y;
        let dz = final_z - node.z;
        let d_sq_3d = dx * dx + dy * dy + (dz * dz * 16.0);
        let d_sq_2d = dx * dx + dy * dy;

        // Node inventory within sigma radius
        if d_sq_3d <= search_radius_sq {
            if let Some(name) = inv_res_map.get(&node.res_idx) {
                let purity_str = if node.multiplier > 1.5 {
                    "Pure"
                } else if node.multiplier < 0.8 {
                    "Impure"
                } else {
                    "Normal"
                };
                let display_name = format!("{} {}", purity_str, name);

                // Separate accessible vs obstructed nodes
                if node.obstructed
                    && (config.game_phase == crate::models::GamePhase::Phase1
                        || config.game_phase == crate::models::GamePhase::Phase2)
                {
                    *obstructed_nodes.entry(display_name).or_insert(0) += 1;
                } else {
                    *local_nodes.entry(display_name).or_insert(0) += 1;
                }

                // Accumulate decay-weighted yield for this resource type
                let d_m = d_sq_3d.sqrt() / 100.0;
                let decay = match config.decay_func {
                    crate::models::DistanceDecay::Gaussian => {
                        let two_sigma_sq = 2.0 * config.sigma * config.sigma;
                        (-(d_m * d_m) / two_sigma_sq).exp()
                    }
                    crate::models::DistanceDecay::Exponential => (-d_m / config.sigma).exp(),
                    crate::models::DistanceDecay::PowerLaw => 1.0 / (d_m / config.sigma + 1.0),
                    crate::models::DistanceDecay::Linear => (1.0 - d_m / config.sigma).max(0.0),
                    crate::models::DistanceDecay::LogisticStep => {
                        if d_m <= config.sigma {
                            1.0
                        } else {
                            0.05
                        }
                    }
                };
                *resource_yields.entry(name.clone()).or_insert(0.0) += node.multiplier * decay;
            }
        }

        // TRI: use 2D distance only (no vertical penalty) for a fair ruggedness measure
        if d_sq_2d <= tri_radius_sq {
            tri_sum += ((final_z - node.z) / 100.0).abs(); // convert cm → m
            tri_count += 1;
        }
    }

    let terrain_ruggedness = if tri_count > 0 {
        tri_sum / tri_count as f64
    } else {
        0.0
    };

    // Shannon entropy diversity of resource yields (higher = more balanced access)
    // diversity = -Σ p_i * ln(p_i)  where p_i = yield_i / total_yield
    let total_yield: f64 = resource_yields.values().sum();
    let diversity_score = if total_yield > 0.0 {
        resource_yields
            .values()
            .filter(|&&y| y > 0.0)
            .map(|&y| {
                let p = y / total_yield;
                -p * p.ln()
            })
            .sum()
    } else {
        0.0
    };

    OptimizationResult {
        x: curr_x,
        y: curr_y,
        z: final_z,
        score: max_score,
        closest_spawn,
        spawn_distance: min_dist / 100.0,
        local_nodes,
        obstructed_nodes,
        resource_yields,
        terrain_ruggedness,
        diversity_score,
    }
}

/// Runs a global high-resolution grid search to find candidate basins, identifies
/// all local maxima, then runs parallelized hill climbing on the top 50 candidates.
struct SearchContext {
    opt_nodes: Vec<OptNode>,
    spatial_grid: SpatialGrid,
    num_resources: usize,
    weights_arr: Vec<f64>,
    epsilons_arr: Vec<f64>,
    res_to_idx: HashMap<String, usize>,
    waterwell_idx: Option<usize>,
    land_mask: LandMask,
}

fn prepare_context(nodes: &[ResourceNode], config: &OptimizerConfig) -> SearchContext {
    let mut unique_types: Vec<String> = nodes.iter().map(|n| n.resource_type.clone()).collect();
    for res_name in config.weights.keys() {
        unique_types.push(res_name.clone());
    }
    if config.weights.contains_key("water") {
        unique_types.push("water".to_string());
    }
    unique_types.sort();
    unique_types.dedup();

    let mut res_to_idx = HashMap::new();
    for (i, t) in unique_types.iter().enumerate() {
        res_to_idx.insert(t.clone(), i);
    }

    let num_resources = unique_types.len();
    assert!(
        num_resources <= 128,
        "Too many resource types (max 128 supported by fixed array)"
    );

    let mut weights_arr = vec![0.0; num_resources];
    for (res_name, &weight) in &config.weights {
        if let Some(&idx) = res_to_idx.get(res_name) {
            weights_arr[idx] = weight;
        }
    }

    // Epsilon floors: keep small but non-zero for Cobb-Douglas/Linear to avoid log(0).
    // For Leontief, epsilon must be near-zero — otherwise the score floor
    // (eps/weight) is large enough that degenerate map-boundary plateaus
    // (where no real nodes exist) yield non-negligible scores that pollute results.
    let leontief_eps = matches!(
        config.utility_func,
        crate::models::UtilityFunction::Leontief
    );
    let mut epsilons_arr = vec![if leontief_eps { 0.001 } else { 0.1 }; num_resources];
    for (i, t) in unique_types.iter().enumerate() {
        epsilons_arr[i] = if leontief_eps {
            0.001 // uniformly tiny for Leontief
        } else {
            match t.as_str() {
                "iron" | "copper" | "limestone" => 0.005,
                "coal" | "oil" | "waterwell" | "geyser" | "nitrogenwell" => 0.05,
                _ => 0.1,
            }
        };
    }

    let opt_nodes: Vec<OptNode> = nodes
        .iter()
        .map(|n| {
            let multiplier = match config.purity_override {
                crate::models::PurityOverride::Default => n.purity.multiplier(),
                crate::models::PurityOverride::Impure => 0.5,
                crate::models::PurityOverride::Normal => 1.0,
                crate::models::PurityOverride::Pure => 2.0,
            };
            let mut obstructed = n.obstructed;

            // --- TEMPORARY HEURISTIC FOR UN-SAVED DEFAULT DATABASE OBSTRUCTIONS ---
            // To easily remove this later, just delete this block and set `obstructed = n.obstructed`.
            if !obstructed && n.resource_type == "caterium" {
                // Keep only the starting Rocky Desert Caterium node open (near x = -2200m, y = -1500m)
                let is_starting_caterium =
                    (n.x - (-220000.0)).abs() < 50000.0 && (n.y - (-150000.0)).abs() < 50000.0;
                if !is_starting_caterium {
                    obstructed = true;
                }
            }
            // ----------------------------------------------------------------------

            OptNode {
                x: n.x,
                y: n.y,
                z: n.z,
                res_idx: *res_to_idx.get(&n.resource_type).unwrap(),
                multiplier,
                obstructed,
            }
        })
        .collect();

    let spatial_grid = SpatialGrid::new(&opt_nodes, 100000.0);
    let waterwell_idx = res_to_idx.get("waterwell").copied();
    let land_mask = LandMask::from_nodes(&opt_nodes);

    SearchContext {
        opt_nodes,
        spatial_grid,
        num_resources,
        weights_arr,
        epsilons_arr,
        res_to_idx,
        waterwell_idx,
        land_mask,
    }
}

fn grid_search_refine(
    ctx: &SearchContext,
    config: &OptimizerConfig,
    grid_res: usize,
    min_dist_between_starts: f64,
    max_candidates: usize,
) -> Vec<OptimizationResult> {
    let step_x = (MAX_X - MIN_X) / grid_res as f64;
    let step_y = (MAX_Y - MIN_Y) / grid_res as f64;

    let grid_points: Vec<(f64, f64)> = (0..=grid_res)
        .flat_map(|row| {
            let y = MIN_Y + row as f64 * step_y;
            (0..=grid_res).map(move |col| {
                let x = MIN_X + col as f64 * step_x;
                (x, y)
            })
        })
        .collect();

    let scores: Vec<f64> = grid_points
        .into_par_iter()
        .map(|(x, y)| {
            calculate_utility(
                x,
                y,
                &ctx.opt_nodes,
                &ctx.spatial_grid,
                config,
                ctx.num_resources,
                &ctx.weights_arr,
                &ctx.epsilons_arr,
                &ctx.res_to_idx,
                ctx.waterwell_idx,
                &ctx.land_mask,
            )
        })
        .collect();

    let rows = grid_res + 1;
    let cols = grid_res + 1;

    let mut local_maxima = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            let idx = r * cols + c;
            let score = scores[idx];

            if score <= 1e-5 {
                continue;
            }

            let mut is_local_max = true;
            for dr in -1..=1 {
                for dc in -1..=1 {
                    if dr == 0 && dc == 0 {
                        continue;
                    }
                    let nr = r as isize + dr;
                    let nc = c as isize + dc;
                    if nr >= 0 && nr < rows as isize && nc >= 0 && nc < cols as isize {
                        let n_idx = (nr as usize) * cols + (nc as usize);
                        if scores[n_idx] > score {
                            is_local_max = false;
                            break;
                        }
                    }
                }
                if !is_local_max {
                    break;
                }
            }

            if is_local_max {
                let x = MIN_X + c as f64 * step_x;
                let y = MIN_Y + r as f64 * step_y;
                local_maxima.push((x, y, score));
            }
        }
    }

    let mut sorted_maxima = local_maxima;
    sorted_maxima.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    let mut start_candidates: Vec<(f64, f64, f64)> = Vec::new();

    for (x, y, score) in sorted_maxima {
        let is_far_enough = start_candidates.iter().all(|&(cx, cy, _)| {
            let dx = cx - x;
            let dy = cy - y;
            (dx * dx + dy * dy).sqrt() >= min_dist_between_starts
        });

        if is_far_enough {
            start_candidates.push((x, y, score));
            if start_candidates.len() >= max_candidates {
                break;
            }
        }
    }

    if start_candidates.is_empty() {
        for spawn in DEFAULT_SPAWNS {
            start_candidates.push((spawn.x, spawn.y, 0.0));
        }
    }

    let refined_results: Vec<OptimizationResult> = start_candidates
        .into_par_iter()
        .map(|(start_x, start_y, _)| {
            run_hill_climbing(
                start_x,
                start_y,
                &ctx.opt_nodes,
                &ctx.spatial_grid,
                config,
                ctx.num_resources,
                &ctx.weights_arr,
                &ctx.epsilons_arr,
                &ctx.res_to_idx,
                ctx.waterwell_idx,
                &ctx.land_mask,
            )
        })
        .collect();

    top_n_results(refined_results, 3, 150_000.0 / 100.0) // 1.5 km min separation
}

/// Returns the top N unique results from a set of refined candidates,
/// filtering out results within min_separation_m metres of a higher-scoring result.
/// Also discards degenerate results where score < min_viable_score (Leontief plateau artifacts).
fn top_n_results(
    mut results: Vec<OptimizationResult>,
    n: usize,
    min_separation_m: f64,
) -> Vec<OptimizationResult> {
    // Save the absolute best result before filtering as a fallback to prevent empty results.
    let absolute_best = results
        .iter()
        .max_by(|a, b| {
            a.score
                .partial_cmp(&b.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .cloned();

    // Filter degenerate plateau scores — these occur when hill climbing is seeded
    // at a map corner with no nodes in range, returning only the epsilon floor.
    let min_viable_score = 0.01;
    results.retain(|r| r.score >= min_viable_score);

    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut kept: Vec<OptimizationResult> = Vec::new();
    for r in results {
        let too_close = kept.iter().any(|k| {
            let dx = (r.x - k.x) / 100.0;
            let dy = (r.y - k.y) / 100.0;
            (dx * dx + dy * dy).sqrt() < min_separation_m
        });
        if !too_close {
            kept.push(r);
            if kept.len() >= n {
                break;
            }
        }
    }

    // Fall back to the single absolute best candidate if all were filtered out.
    if kept.is_empty() {
        if let Some(best) = absolute_best {
            kept.push(best);
        }
    }

    kept
}

fn optimize_hybrid(ctx: &SearchContext, config: &OptimizerConfig) -> Vec<OptimizationResult> {
    let grid_res = 500;
    let min_dist_between_starts = 300.0 * 100.0;
    let max_candidates = 50;
    grid_search_refine(
        ctx,
        config,
        grid_res,
        min_dist_between_starts,
        max_candidates,
    )
}

fn optimize_slow(ctx: &SearchContext, config: &OptimizerConfig) -> Vec<OptimizationResult> {
    let grid_res = 1000;
    let min_dist_between_starts = 200.0 * 100.0;
    let max_candidates = 100;
    grid_search_refine(
        ctx,
        config,
        grid_res,
        min_dist_between_starts,
        max_candidates,
    )
}

fn optimize_fast(ctx: &SearchContext, config: &OptimizerConfig) -> Vec<OptimizationResult> {
    let mut starts = Vec::new();

    for spawn in DEFAULT_SPAWNS {
        starts.push((spawn.x, spawn.y));
    }

    let steps = 4;
    let step_x = (MAX_X - MIN_X) / (steps + 1) as f64;
    let step_y = (MAX_Y - MIN_Y) / (steps + 1) as f64;
    for i in 1..=steps {
        let x = MIN_X + i as f64 * step_x;
        for j in 1..=steps {
            let y = MIN_Y + j as f64 * step_y;
            starts.push((x, y));
        }
    }

    let refined_results: Vec<OptimizationResult> = starts
        .into_par_iter()
        .map(|(start_x, start_y)| {
            run_hill_climbing(
                start_x,
                start_y,
                &ctx.opt_nodes,
                &ctx.spatial_grid,
                config,
                ctx.num_resources,
                &ctx.weights_arr,
                &ctx.epsilons_arr,
                &ctx.res_to_idx,
                ctx.waterwell_idx,
                &ctx.land_mask,
            )
        })
        .collect();

    top_n_results(refined_results, 3, 150_000.0 / 100.0)
}

/// Returns up to 5 geographically distinct optimal starting locations, ranked by score.
pub fn optimize(nodes: &[ResourceNode], config: &OptimizerConfig) -> Vec<OptimizationResult> {
    let ctx = prepare_context(nodes, config);
    match config.strategy {
        crate::models::SearchStrategy::Hybrid => optimize_hybrid(&ctx, config),
        crate::models::SearchStrategy::Fast => optimize_fast(&ctx, config),
        crate::models::SearchStrategy::Slow => optimize_slow(&ctx, config),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{GamePhase, OptimizerConfig};

    #[test]
    fn test_ignore_spawns() {
        let nodes = crate::data_loader::load_default_nodes();

        let mut config_constrained = OptimizerConfig::default();
        config_constrained.game_phase = GamePhase::Phase1;
        config_constrained.ignore_spawns = false;

        let mut config_ignored = OptimizerConfig::default();
        config_ignored.game_phase = GamePhase::Phase1;
        config_ignored.ignore_spawns = true;

        let ctx = prepare_context(&nodes, &config_constrained);
        let far_dune_desert_x = 291000.0;
        let far_dune_desert_y = 74000.0;

        let constrained_score = calculate_utility(
            far_dune_desert_x,
            far_dune_desert_y,
            &ctx.opt_nodes,
            &ctx.spatial_grid,
            &config_constrained,
            ctx.num_resources,
            &ctx.weights_arr,
            &ctx.epsilons_arr,
            &ctx.res_to_idx,
            ctx.waterwell_idx,
            &ctx.land_mask,
        );
        let ignored_score = calculate_utility(
            far_dune_desert_x,
            far_dune_desert_y,
            &ctx.opt_nodes,
            &ctx.spatial_grid,
            &config_ignored,
            ctx.num_resources,
            &ctx.weights_arr,
            &ctx.epsilons_arr,
            &ctx.res_to_idx,
            ctx.waterwell_idx,
            &ctx.land_mask,
        );

        assert!(ignored_score > constrained_score);
    }

    #[test]
    fn test_default_nodes_optimize() {
        let nodes = crate::data_loader::load_default_nodes();
        let config = OptimizerConfig::default();
        let results = optimize(&nodes, &config);
        assert!(!results.is_empty());
        assert!(results[0].score > 0.0);
        assert!(!results[0].local_nodes.is_empty());
    }

    #[test]
    fn test_ocean_points_are_not_valid_base_sites() {
        let nodes = crate::data_loader::load_default_nodes();
        let config = OptimizerConfig::default();
        let ctx = prepare_context(&nodes, &config);

        assert!(!is_in_polygon(-310000.0, -330000.0, &ctx.land_mask.points));
        assert!(is_in_polygon(-200000.0, -80000.0, &ctx.land_mask.points));
    }

    #[test]
    fn test_buildable_land_mask_matches_known_edge_cases() {
        let nodes = crate::data_loader::load_default_nodes();
        let config = OptimizerConfig::default();
        let ctx = prepare_context(&nodes, &config);

        assert!(is_in_polygon(-271000.0, -192000.0, &ctx.land_mask.points)); // gas pillars / yellow slug coast
        assert!(is_in_polygon(-298000.0, -37000.0, &ctx.land_mask.points)); // western blue slug coast
        assert!(is_in_polygon(291000.0, 74000.0, &ctx.land_mask.points)); // hard-drive island/shoreline area
        assert!(is_in_polygon(321000.0, -176000.0, &ctx.land_mask.points)); // Dune Desert land

        assert!(!is_in_polygon(390000.0, 74000.0, &ctx.land_mask.points)); // east ocean near hard-drive area
        assert!(!is_in_polygon(410000.0, -330000.0, &ctx.land_mask.points)); // northern Dune Desert black/ocean area
        assert!(!is_in_polygon(-50000.0, 330000.0, &ctx.land_mask.points)); // south of Grass Fields playable land
    }
}

