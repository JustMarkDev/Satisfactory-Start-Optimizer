//! HTTP API server for the web dashboard.
//!
//! Endpoints:
//!   GET  /api/presets        → returns all phase presets with weights
//!   GET  /api/spawns         → returns all default spawn locations
//!   POST /api/optimize       → accepts OptimizerRequest JSON, returns top-3 OptimizationResult[]

use actix_cors::Cors;
use actix_web::{App, HttpResponse, HttpServer, Responder, web};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::models::{
    DEFAULT_SPAWNS, DistanceDecay, GamePhase, OptimizerConfig, PurityOverride, SearchStrategy,
    UtilityFunction,
};
use crate::optimizer;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

/// POST /api/optimize request body
#[derive(Debug, Deserialize)]
pub struct OptimizeRequest {
    pub utility_func: String,
    pub decay_func: String,
    pub purity_override: String,
    pub strategy: String,
    pub game_phase: String,
    pub ignore_spawns: bool,
    /// Resource weights as a map of resource_id → weight.
    /// Values of 0.0 are ignored in the calculation.
    pub weights: HashMap<String, f64>,
}

/// GET /api/presets response item
#[derive(Serialize)]
pub struct PresetResponse {
    pub id: String,
    pub name: String,
    pub sigma: f64,
    pub ignore_spawns: bool,
    pub weights: HashMap<String, f64>,
}

/// Shared application state (the parsed map nodes loaded at startup)
pub struct AppState {
    pub nodes: Vec<crate::models::ResourceNode>,
}

// ---------------------------------------------------------------------------
// Helper parsers
// ---------------------------------------------------------------------------

fn parse_utility(s: &str) -> UtilityFunction {
    match s {
        "leontief" => UtilityFunction::Leontief,
        "linear" => UtilityFunction::Linear,
        _ => UtilityFunction::CobbDouglas,
    }
}

fn parse_decay(s: &str) -> DistanceDecay {
    match s {
        "exponential" => DistanceDecay::Exponential,
        "power_law" => DistanceDecay::PowerLaw,
        "linear" => DistanceDecay::Linear,
        "logistic" => DistanceDecay::LogisticStep,
        _ => DistanceDecay::Gaussian,
    }
}

fn parse_purity(s: &str) -> PurityOverride {
    match s {
        "impure" => PurityOverride::Impure,
        "normal" => PurityOverride::Normal,
        "pure" => PurityOverride::Pure,
        _ => PurityOverride::Default,
    }
}

fn parse_strategy(s: &str) -> SearchStrategy {
    match s {
        "fast" => SearchStrategy::Fast,
        "slow" => SearchStrategy::Slow,
        _ => SearchStrategy::Hybrid,
    }
}

fn parse_phase(s: &str) -> GamePhase {
    match s {
        "phase2" => GamePhase::Phase2,
        "phase3" => GamePhase::Phase3,
        "phase4" => GamePhase::Phase4,
        "phase5" => GamePhase::Phase5,
        "collectibles" => GamePhase::Phase5,
        _ => GamePhase::Phase1,
    }
}

fn apply_collectibles_weights(weights: &mut HashMap<String, f64>) {
    weights.clear();
    weights.insert("blueslug".to_string(), 0.3);
    weights.insert("yellowslug".to_string(), 0.8);
    weights.insert("purpleslug".to_string(), 1.2);
    weights.insert("mercer".to_string(), 1.0);
    weights.insert("somersloop".to_string(), 1.0);
    weights.insert("harddrive".to_string(), 1.5);
    weights.insert("paleberry".to_string(), 0.0);
    weights.insert("berylnut".to_string(), 0.0);
    weights.insert("baconagaric".to_string(), 0.0);
}

// ---------------------------------------------------------------------------
// Phase preset builder (mirrors JS PRESETS)
// ---------------------------------------------------------------------------

fn build_presets() -> Vec<PresetResponse> {
    let phases: &[(&str, &str, bool)] = &[
        ("phase1", "Phase 1 — Early Game (Tiers 1-2)", false),
        ("phase2", "Phase 2 — Steel & Coal (Tiers 3-4)", false),
        ("phase3", "Phase 3 — Oil & Quartz (Tiers 5-6)", false),
        ("phase4", "Phase 4 — Aluminum & Nuclear (Tiers 7-8)", true),
        ("phase5", "Phase 5 — Quantum (Tier 9)", true),
        (
            "collectibles",
            "Collectibles — Slugs, Artifacts & Hard Drives",
            true,
        ),
    ];

    phases
        .iter()
        .map(|(id, name, ignore_spawns)| {
            let mut weights = HashMap::<String, f64>::new();

            if *id == "collectibles" {
                apply_collectibles_weights(&mut weights);
            } else {
                // Apply the standard model weights via the Rust model system
                let phase = parse_phase(id);
                phase.apply_weights(&mut weights);
            }

            // Force berylnut / paleberry / baconagaric off by default
            weights.insert("paleberry".to_string(), 0.0);
            weights.insert("berylnut".to_string(), 0.0);
            weights.insert("baconagaric".to_string(), 0.0);

            PresetResponse {
                id: id.to_string(),
                name: name.to_string(),
                sigma: 700.0,
                ignore_spawns: *ignore_spawns,
                weights,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Route handlers
// ---------------------------------------------------------------------------

async fn get_presets() -> impl Responder {
    let presets = build_presets();
    HttpResponse::Ok().json(presets)
}

async fn get_spawns() -> impl Responder {
    HttpResponse::Ok().json(DEFAULT_SPAWNS)
}

async fn post_optimize(
    state: web::Data<Arc<AppState>>,
    body: web::Json<OptimizeRequest>,
) -> impl Responder {
    let req = body.into_inner();

    // Build OptimizerConfig from request
    let mut config = OptimizerConfig {
        sigma: 700.0,
        weights: req.weights.clone(),
        purity_override: parse_purity(&req.purity_override),
        strategy: parse_strategy(&req.strategy),
        utility_func: parse_utility(&req.utility_func),
        decay_func: parse_decay(&req.decay_func),
        game_phase: parse_phase(&req.game_phase),
        ignore_spawns: req.ignore_spawns,
    };

    // Zero-out weights that are exactly 0 (the frontend sends 0.0 for disabled resources)
    config.weights.retain(|_, v| *v != 0.0);

    // Run the optimizer (this is CPU-heavy but runs in a blocking thread pool)
    let nodes = Arc::clone(&state);
    let result = web::block(move || optimizer::optimize(&nodes.nodes, &config))
        .await
        .map_err(|e| {
            eprintln!("Optimizer blocking task failed: {:?}", e);
        });

    match result {
        Ok(results) => HttpResponse::Ok().json(results),
        Err(_) => HttpResponse::InternalServerError()
            .body("Optimization failed — see server logs for details"),
    }
}

/// Health check
async fn get_health() -> impl Responder {
    HttpResponse::Ok().body("OK")
}

// ---------------------------------------------------------------------------
// Server entry point
// ---------------------------------------------------------------------------

#[actix_web::main]
pub async fn run_server(port: u16) -> std::io::Result<()> {
    let nodes = crate::data_loader::load_default_nodes();
    println!("FICSIT API Server — loaded {} resource nodes", nodes.len());
    println!("Listening on http://127.0.0.1:{}", port);
    println!("  POST /api/optimize  — run optimization");
    println!("  GET  /api/presets   — get phase presets");
    println!("  GET  /api/spawns    — get spawn locations");
    println!("  GET  /api/health    — health check");

    let data = Arc::new(AppState { nodes });

    HttpServer::new(move || {
        // Allow all origins in dev (Vite runs on 3000, server on 8080)
        let cors = Cors::permissive();

        App::new()
            .wrap(cors)
            .app_data(web::Data::new(Arc::clone(&data)))
            .app_data(
                web::JsonConfig::default()
                    .limit(1024 * 1024) // 1 MB max request body
                    .error_handler(|err, _req| {
                        let response =
                            HttpResponse::BadRequest().body(format!("JSON parse error: {}", err));
                        actix_web::error::InternalError::from_response(err, response).into()
                    }),
            )
            .route("/api/health", web::get().to(get_health))
            .route("/api/presets", web::get().to(get_presets))
            .route("/api/spawns", web::get().to(get_spawns))
            .route("/api/optimize", web::post().to(post_optimize))
    })
    .bind(("127.0.0.1", port))?
    .run()
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_include_collectibles_and_late_water_wells() {
        let presets = build_presets();

        let collectibles = presets
            .iter()
            .find(|p| p.id == "collectibles")
            .expect("collectibles preset missing");
        assert!(
            collectibles
                .weights
                .get("harddrive")
                .copied()
                .unwrap_or(0.0)
                > 0.0
        );
        assert!(
            collectibles
                .weights
                .get("purpleslug")
                .copied()
                .unwrap_or(0.0)
                > 0.0
        );

        for phase_id in ["phase4", "phase5"] {
            let preset = presets
                .iter()
                .find(|p| p.id == phase_id)
                .expect("late phase preset missing");
            assert!(preset.weights.get("waterwell").copied().unwrap_or(0.0) > 0.0);
        }
    }
}
