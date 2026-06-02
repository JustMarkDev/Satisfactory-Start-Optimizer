use crate::models::{ResourceNode, OptimizerConfig, SpawnLocation, DEFAULT_SPAWNS};
use std::collections::HashMap;
use rayon::prelude::*;

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
}

#[derive(Debug, Clone, Copy)]
struct OptNode {
    x: f64,
    y: f64,
    z: f64,
    res_idx: usize,
    multiplier: f64,
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
            let col = (((node.x - min_x) / bucket_size) as isize).clamp(0, cols as isize - 1) as usize;
            let row = (((node.y - min_y) / bucket_size) as isize).clamp(0, rows as isize - 1) as usize;
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

/// Helper to calculate distance in meters from (x, y) coordinate to the nearest water body.
/// Satisfactory map has several prominent static lakes/oceans, plus any dynamic water wells.
fn distance_to_nearest_water(x: f64, y: f64, opt_nodes: &[OptNode], waterwell_idx: Option<usize>) -> f64 {
    let mut min_dist_cm = f64::MAX;
    
    // Major static bodies of water in Satisfactory (centers and bounding boxes)
    let water_bodies = [
        (140000.0, 230000.0, 80000.0, 60000.0),    // Great Blue Crater Lake (South-East)
        (-70000.0, -145000.0, 60000.0, 70000.0),  // Crater Lakes / Coal Lakes (North)
        (-60000.0, 65000.0, 80000.0, 70000.0),    // Red Jungle Lakes
        (45000.0, -20000.0, 30000.0, 30000.0),     // Lake Forest Lake
        (350000.0, 225000.0, 140000.0, 150000.0),  // Eastern Swamp Lakes
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
    
    // Check distances to ocean boundaries (West, North, East coasts)
    if x < -250000.0 {
        min_dist_cm = min_dist_cm.min(0.0);
    } else {
        min_dist_cm = min_dist_cm.min(x + 250000.0);
    }
    if y < -280000.0 {
        min_dist_cm = min_dist_cm.min(0.0);
    } else {
        min_dist_cm = min_dist_cm.min(y + 280000.0);
    }
    if x > 380000.0 {
        min_dist_cm = min_dist_cm.min(0.0);
    } else {
        min_dist_cm = min_dist_cm.min(380000.0 - x);
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
) -> f64 {
    let radius = 3.5 * config.sigma;
    let radius_cm = radius * 100.0;
    let radius_cm_sq = radius_cm * radius_cm;
    let radius_m_sq = radius * radius;

    // 1. KNN IDW Ground Height Estimation (Top 6 nearest nodes horizontally)
    let mut nearest = [(f64::MAX, 0.0); 6]; // (dist_sq, z)
    let mut count = 0;

    let min_qx = x - radius_cm;
    let max_qx = x + radius_cm;
    let min_qy = y - radius_cm;
    let max_qy = y + radius_cm;

    let col_start = (((min_qx - spatial_grid.min_x) / spatial_grid.bucket_size) as isize).clamp(0, spatial_grid.cols as isize - 1) as usize;
    let col_end = (((max_qx - spatial_grid.min_x) / spatial_grid.bucket_size) as isize).clamp(0, spatial_grid.cols as isize - 1) as usize;
    let row_start = (((min_qy - spatial_grid.min_y) / spatial_grid.bucket_size) as isize).clamp(0, spatial_grid.rows as isize - 1) as usize;
    let row_end = (((max_qy - spatial_grid.min_y) / spatial_grid.bucket_size) as isize).clamp(0, spatial_grid.rows as isize - 1) as usize;

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

    let z = if count > 0 {
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
    };

    // 2. Sum yields for all resource types & collect local heights for terrain flatness
    let mut yields = [0.0; 32];
    let two_sigma_sq = 2.0 * config.sigma * config.sigma;

    let build_radius = 1.5 * config.sigma; // factory building area footprint
    let build_radius_m_sq = build_radius * build_radius;
    let mut local_heights = [0.0; 64];
    let mut heights_count = 0;

    for r_idx in row_start..=row_end {
        for c_idx in col_start..=col_end {
            let bucket_idx = r_idx * spatial_grid.cols + c_idx;
            for &node_idx in &spatial_grid.buckets[bucket_idx] {
                let node = &opt_nodes[node_idx];
                let dx = (x - node.x) / 100.0;
                let dy = (y - node.y) / 100.0;
                let dz = (z - node.z) / 100.0;

                let vertical_multiplier = 4.0;
                let d_sq = dx * dx + dy * dy + (dz * dz * vertical_multiplier * vertical_multiplier);

                if d_sq <= radius_m_sq {
                    let decay = (-d_sq / two_sigma_sq).exp();
                    let contribution = node.multiplier * decay;
                    yields[node.res_idx] += contribution;

                    // Keep track of nearby heights to measure slope variance (terrain flatness)
                    if d_sq <= build_radius_m_sq && heights_count < 64 {
                        local_heights[heights_count] = node.z;
                        heights_count += 1;
                    }
                }
            }
        }
    }

    // Add virtual water yield based on proximity to lakes, oceans, or waterwells
    if let Some(&water_idx) = res_to_idx.get("water") {
        let water_dist = distance_to_nearest_water(x, y, opt_nodes, waterwell_idx);
        let water_decay = (- (water_dist * water_dist) / two_sigma_sq).exp();
        yields[water_idx] = water_decay;
    }

    // 3. Terrain Flatness Penalty (Standard Deviation of local ground heights)
    let mut flatness_mult = 1.0;
    if heights_count > 1 {
        let mut sum = 0.0;
        for i in 0..heights_count {
            sum += local_heights[i];
        }
        let mean = sum / heights_count as f64;
        
        let mut variance_sum = 0.0;
        for i in 0..heights_count {
            let diff = local_heights[i] - mean;
            variance_sum += diff * diff;
        }
        let std_dev_cm = (variance_sum / heights_count as f64).sqrt();
        let std_dev_m = std_dev_cm / 100.0;
        
        // Penalize variance exponentially; scaling denominator of 40 meters standard deviation.
        flatness_mult = (-std_dev_m / 40.0).exp();
    }

    // 4. Cobb-Douglas combined utility + Radiation Penalties
    let mut score = 1.0;
    for i in 0..num_resources {
        let weight = weights_arr[i];
        if weight > 0.0 {
            let res_yield = yields[i];
            let eps = epsilons_arr[i];
            score *= (res_yield + eps).powf(weight);
        } else if weight < 0.0 {
            // Negative weights represent threats (like radiation from Uranium).
            // We penalize the score exponentially based on threat yield.
            let res_yield = yields[i];
            let penalty_factor = (res_yield * weight.abs()).exp();
            score /= penalty_factor;
        }
    }

    score * flatness_mult
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
    );

    let dirs = [
        (1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0),
        (0.7071, 0.7071), (-0.7071, 0.7071), (0.7071, -0.7071), (-0.7071, -0.7071)
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
    let radius = 3.5 * config.sigma;
    let radius_cm = radius * 100.0;
    let radius_cm_sq = radius_cm * radius_cm;
    
    let mut nearest = [(f64::MAX, 0.0); 6];
    let mut count = 0;

    let min_qx = curr_x - radius_cm;
    let max_qx = curr_x + radius_cm;
    let min_qy = curr_y - radius_cm;
    let max_qy = curr_y + radius_cm;

    let col_start = (((min_qx - spatial_grid.min_x) / spatial_grid.bucket_size) as isize).clamp(0, spatial_grid.cols as isize - 1) as usize;
    let col_end = (((max_qx - spatial_grid.min_x) / spatial_grid.bucket_size) as isize).clamp(0, spatial_grid.cols as isize - 1) as usize;
    let row_start = (((min_qy - spatial_grid.min_y) / spatial_grid.bucket_size) as isize).clamp(0, spatial_grid.rows as isize - 1) as usize;
    let row_end = (((max_qy - spatial_grid.min_y) / spatial_grid.bucket_size) as isize).clamp(0, spatial_grid.rows as isize - 1) as usize;

    for r_idx in row_start..=row_end {
        for c_idx in col_start..=col_end {
            let bucket_idx = r_idx * spatial_grid.cols + c_idx;
            for &node_idx in &spatial_grid.buckets[bucket_idx] {
                let node = &opt_nodes[node_idx];
                let dx = curr_x - node.x;
                let dy = curr_y - node.y;
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

    let final_z = if count > 0 {
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
    };

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

    OptimizationResult {
        x: curr_x,
        y: curr_y,
        z: final_z,
        score: max_score,
        closest_spawn,
        spawn_distance: min_dist / 100.0,
    }
}

/// Runs a global high-resolution grid search to find candidate basins, identifies
/// all local maxima, then runs parallelized hill climbing on the top 50 candidates.
pub fn optimize(nodes: &[ResourceNode], config: &OptimizerConfig) -> OptimizationResult {
    // 1. Map resource types to indices dynamically
    let mut unique_types: Vec<String> = nodes.iter().map(|n| n.resource_type.clone()).collect();
    for res_name in config.weights.keys() {
        unique_types.push(res_name.clone());
    }
    // Explicitly add "water" resource key if present in weights but not nodes
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
    assert!(num_resources <= 32, "Too many resource types (max 32 supported by fixed array)");

    let mut weights_arr = vec![0.0; num_resources];
    for (res_name, &weight) in &config.weights {
        if let Some(&idx) = res_to_idx.get(res_name) {
            weights_arr[idx] = weight;
        }
    }

    let mut epsilons_arr = vec![0.1; num_resources];
    for (i, t) in unique_types.iter().enumerate() {
        epsilons_arr[i] = match t.as_str() {
            "iron" | "copper" | "limestone" => 0.005,
            "coal" | "oil" | "waterwell" | "geyser" | "nitrogenwell" => 0.05,
            "blueslug" | "yellowslug" | "purpleslug" | "mercer" | "somersloop" | "harddrive" => 0.1,
            _ => 0.1,
        };
    }

    // Convert ResourceNode to OptNode for cache-friendly execution
    let opt_nodes: Vec<OptNode> = nodes
        .iter()
        .map(|n| {
            let multiplier = match config.purity_override {
                crate::models::PurityOverride::Default => n.purity.multiplier(),
                crate::models::PurityOverride::Impure => 0.5,
                crate::models::PurityOverride::Normal => 1.0,
                crate::models::PurityOverride::Pure => 2.0,
            };
            OptNode {
                x: n.x,
                y: n.y,
                z: n.z,
                res_idx: *res_to_idx.get(&n.resource_type).unwrap(),
                multiplier,
            }
        })
        .collect();

    // 2. Build Spatial Grid (Bucket size = 1000m = 100,000 cm)
    let spatial_grid = SpatialGrid::new(&opt_nodes, 100000.0);
    let waterwell_idx = res_to_idx.get("waterwell").copied();

    // 3. Grid Search: Check 500x500 points (250,000 points checked every ~14.8 meters)
    let grid_res = 500;
    let step_x = (MAX_X - MIN_X) / grid_res as f64;
    let step_y = (MAX_Y - MIN_Y) / grid_res as f64;

    let grid_points: Vec<(f64, f64)> = (0..=grid_res)
        .flat_map(|i| {
            let x = MIN_X + i as f64 * step_x;
            (0..=grid_res).map(move |j| {
                let y = MIN_Y + j as f64 * step_y;
                (x, y)
            })
        })
        .collect();

    // Evaluate all grid points in parallel
    let scores: Vec<f64> = grid_points
        .into_par_iter()
        .map(|(x, y)| {
            calculate_utility(
                x,
                y,
                &opt_nodes,
                &spatial_grid,
                config,
                num_resources,
                &weights_arr,
                &epsilons_arr,
                &res_to_idx,
                waterwell_idx,
            )
        })
        .collect();

    // 4. Find all local maxima in the grid sequentially
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

    // Sort local maxima descending by score
    let mut sorted_maxima = local_maxima;
    sorted_maxima.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    // Choose top local maxima candidates
    let min_dist_between_starts = 300.0 * 100.0; // 300 meters
    let mut start_candidates: Vec<(f64, f64, f64)> = Vec::new();

    for (x, y, score) in sorted_maxima {
        let is_far_enough = start_candidates.iter().all(|&(cx, cy, _)| {
            let dx = cx - x;
            let dy = cy - y;
            (dx * dx + dy * dy).sqrt() >= min_dist_between_starts
        });

        if is_far_enough {
            start_candidates.push((x, y, score));
            if start_candidates.len() >= 50 {
                break;
            }
        }
    }

    // 5. Refine all top candidates in parallel using hill climbing
    let refined_results: Vec<OptimizationResult> = start_candidates
        .into_par_iter()
        .map(|(start_x, start_y, _)| {
            run_hill_climbing(
                start_x,
                start_y,
                &opt_nodes,
                &spatial_grid,
                config,
                num_resources,
                &weights_arr,
                &epsilons_arr,
                &res_to_idx,
                waterwell_idx,
            )
        })
        .collect();

    // 6. Select the absolute best refined peak
    refined_results
        .into_iter()
        .max_by(|a, b| a.score.partial_cmp(&b.score).unwrap_or(std::cmp::Ordering::Equal))
        .expect("No candidates optimized successfully")
}
