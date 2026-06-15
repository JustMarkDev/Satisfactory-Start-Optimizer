//! HTTP API server for the web dashboard.
//!
//! Endpoints:
//!   GET  /api/presets        → returns all phase presets with weights
//!   GET  /api/spawns         → returns all default spawn locations
//!   GET  /api/nodes          → returns normalized map nodes for rendering
//!   POST /api/optimize       → accepts OptimizerRequest JSON, returns top-3 OptimizationResult[]

use actix_cors::Cors;
use actix_web::{
    App, HttpResponse, HttpServer, Responder,
    http::{Method, header},
    web,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::models::{
    DEFAULT_SPAWNS, DistanceDecay, GamePhase, OptimizerConfig, PurityOverride, ResourceNode,
    SearchStrategy, UtilityFunction,
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
    pub sigma: f64,
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

/// GET /api/nodes response item
#[derive(Debug, Serialize, PartialEq)]
pub struct NodeResponse {
    pub resource_type: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub purity: crate::models::Purity,
    #[serde(rename = "purityMultiplier")]
    pub purity_multiplier: f64,
    pub obstructed: bool,
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

fn validate_optimize_request(req: &OptimizeRequest, nodes: &[ResourceNode]) -> Result<(), String> {
    if !req.sigma.is_finite() {
        return Err("Invalid optimize request: sigma must be finite".to_string());
    }

    let mut meaningful_weights = Vec::new();
    for (resource_id, weight) in &req.weights {
        if !weight.is_finite() {
            return Err("Invalid optimize request: weight must be finite".to_string());
        }
        if *weight != 0.0 {
            meaningful_weights.push(resource_id.as_str());
        }
    }

    if meaningful_weights.is_empty() {
        return Err(
            "Invalid optimize request: at least one resource weight is required".to_string(),
        );
    }

    let mut known_resource_ids = nodes
        .iter()
        .map(|node| node.resource_type.as_str())
        .collect::<HashSet<_>>();
    known_resource_ids.insert("water");

    for resource_id in &meaningful_weights {
        if !known_resource_ids.contains(resource_id) {
            return Err("Invalid optimize request: unknown resource id".to_string());
        }
    }

    let resource_universe_size = nodes
        .iter()
        .map(|node| node.resource_type.as_str())
        .chain(meaningful_weights)
        .collect::<HashSet<_>>()
        .len();
    if resource_universe_size > 128 {
        return Err("Invalid optimize request: too many resource types".to_string());
    }

    Ok(())
}

fn local_dashboard_cors() -> Cors {
    Cors::default()
        .allowed_origin("http://127.0.0.1:3000")
        .allowed_origin("http://localhost:3000")
        .allowed_methods([Method::GET, Method::POST])
        .allowed_header(header::CONTENT_TYPE)
}

// ---------------------------------------------------------------------------
// Phase preset builder (mirrors JS PRESETS)
// ---------------------------------------------------------------------------

fn build_presets() -> Vec<PresetResponse> {
    let phases: &[(&str, &str, bool, f64)] = &[
        ("phase1", "Phase 1 — Early Game (Tiers 1-2)", false, 200.0),
        ("phase2", "Phase 2 — Steel & Coal (Tiers 3-4)", false, 300.0),
        ("phase3", "Phase 3 — Oil & Quartz (Tiers 5-6)", false, 400.0),
        (
            "phase4",
            "Phase 4 — Aluminum & Nuclear (Tiers 7-8)",
            true,
            600.0,
        ),
        ("phase5", "Phase 5 — Quantum (Tier 9)", true, 800.0),
        (
            "collectibles",
            "Collectibles — Slugs, Artifacts & Hard Drives",
            true,
            1000.0,
        ),
    ];

    phases
        .iter()
        .map(|(id, name, ignore_spawns, sigma)| {
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
                sigma: *sigma,
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

fn node_response(node: &ResourceNode) -> NodeResponse {
    NodeResponse {
        resource_type: node.resource_type.clone(),
        x: node.x,
        y: node.y,
        z: node.z,
        purity: node.purity,
        purity_multiplier: node.purity.multiplier(),
        obstructed: node.obstructed,
    }
}

async fn get_nodes(state: web::Data<Arc<AppState>>) -> impl Responder {
    let nodes = state.nodes.iter().map(node_response).collect::<Vec<_>>();
    HttpResponse::Ok().json(nodes)
}

async fn post_optimize(
    state: web::Data<Arc<AppState>>,
    body: web::Json<OptimizeRequest>,
) -> impl Responder {
    let req = body.into_inner();

    if let Err(message) = validate_optimize_request(&req, &state.nodes) {
        return HttpResponse::BadRequest().body(message);
    }

    // Build OptimizerConfig from request
    let mut config = OptimizerConfig {
        sigma: req.sigma.clamp(50.0, 1000.0),
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
    println!("  GET  /api/nodes     — get normalized map nodes");
    println!("  GET  /api/health    — health check");

    let data = Arc::new(AppState { nodes });

    HttpServer::new(move || {
        App::new()
            .wrap(local_dashboard_cors())
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
            .route("/api/nodes", web::get().to(get_nodes))
            .route("/api/optimize", web::post().to(post_optimize))
    })
    .bind(("127.0.0.1", port))?
    .run()
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Purity;
    use actix_web::{
        App,
        http::{StatusCode, header},
        test as actix_test,
    };

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

    #[test]
    fn presets_keep_phase_contract() {
        let presets = build_presets();
        let ids = presets
            .iter()
            .map(|preset| preset.id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            ids,
            [
                "phase1",
                "phase2",
                "phase3",
                "phase4",
                "phase5",
                "collectibles"
            ]
        );
        assert_eq!(presets.len(), 6);
        assert!(presets.iter().all(|preset| preset.sigma > 0.0));

        for phase_id in ["phase1", "phase2"] {
            let preset = presets
                .iter()
                .find(|preset| preset.id == phase_id)
                .expect("early phase preset missing");
            assert!(!preset.ignore_spawns);
        }

        for phase_id in ["phase4", "phase5", "collectibles"] {
            let preset = presets
                .iter()
                .find(|preset| preset.id == phase_id)
                .expect("late phase preset missing");
            assert!(preset.ignore_spawns);
        }
    }

    fn test_nodes() -> Vec<ResourceNode> {
        vec![ResourceNode {
            resource_type: "iron".to_string(),
            purity: Purity::Normal,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            obstructed: false,
        }]
    }

    #[test]
    fn node_response_contains_normalized_rendering_fields() {
        let node = ResourceNode {
            resource_type: "copper".to_string(),
            purity: Purity::Pure,
            x: 123.5,
            y: -456.25,
            z: 78.0,
            obstructed: true,
        };

        let response = node_response(&node);
        let json = serde_json::to_value(response).expect("node response should serialize");

        assert_eq!(json["resource_type"], "copper");
        assert_eq!(json["x"], 123.5);
        assert_eq!(json["y"], -456.25);
        assert_eq!(json["z"], 78.0);
        assert_eq!(json["purity"], "pure");
        assert_eq!(json["purityMultiplier"], 2.0);
        assert_eq!(json["obstructed"], true);
    }

    #[actix_web::test]
    async fn nodes_endpoint_returns_normalized_nodes() {
        let app_state = Arc::new(AppState {
            nodes: vec![ResourceNode {
                resource_type: "iron".to_string(),
                purity: Purity::Impure,
                x: 1.0,
                y: 2.0,
                z: 3.0,
                obstructed: false,
            }],
        });
        let app = actix_test::init_service(
            App::new()
                .app_data(web::Data::new(app_state))
                .route("/api/nodes", web::get().to(get_nodes)),
        )
        .await;

        let request = actix_test::TestRequest::get()
            .uri("/api/nodes")
            .to_request();
        let body: serde_json::Value = actix_test::call_and_read_body_json(&app, request).await;

        assert_eq!(body[0]["resource_type"], "iron");
        assert_eq!(body[0]["purity"], "impure");
        assert_eq!(body[0]["purityMultiplier"], 0.5);
    }

    fn test_request(weights: &[(&str, f64)]) -> OptimizeRequest {
        OptimizeRequest {
            utility_func: "cobb_douglas".to_string(),
            decay_func: "gaussian".to_string(),
            purity_override: "default".to_string(),
            strategy: "hybrid".to_string(),
            game_phase: "phase1".to_string(),
            sigma: 200.0,
            ignore_spawns: false,
            weights: weights
                .iter()
                .map(|(resource_id, weight)| (resource_id.to_string(), *weight))
                .collect(),
        }
    }

    #[test]
    fn validation_rejects_empty_weights() {
        let nodes = test_nodes();
        let req = test_request(&[]);

        assert!(validate_optimize_request(&req, &nodes).is_err());
    }

    #[test]
    fn validation_rejects_all_zero_weights() {
        let nodes = test_nodes();
        let req = test_request(&[("iron", 0.0), ("water", 0.0)]);

        assert!(validate_optimize_request(&req, &nodes).is_err());
    }

    #[test]
    fn validation_rejects_unknown_resource_ids() {
        let nodes = test_nodes();
        let req = test_request(&[("unknown", 1.0)]);

        assert!(validate_optimize_request(&req, &nodes).is_err());
    }

    #[test]
    fn validation_rejects_non_finite_values() {
        let nodes = test_nodes();
        let mut req = test_request(&[("iron", 1.0)]);
        req.sigma = f64::INFINITY;
        assert!(validate_optimize_request(&req, &nodes).is_err());

        let req = test_request(&[("iron", f64::NAN)]);
        assert!(validate_optimize_request(&req, &nodes).is_err());
    }

    #[test]
    fn validation_accepts_known_resource_ids_and_water() {
        let nodes = test_nodes();
        let req = test_request(&[("iron", 1.0), ("water", 0.5)]);

        assert!(validate_optimize_request(&req, &nodes).is_ok());
    }

    #[test]
    fn validation_rejects_resource_universe_over_fixed_limit() {
        let nodes = (0..129)
            .map(|index| ResourceNode {
                resource_type: format!("resource{index}"),
                purity: Purity::Normal,
                x: 0.0,
                y: 0.0,
                z: 0.0,
                obstructed: false,
            })
            .collect::<Vec<_>>();
        let req = test_request(&[("resource0", 1.0)]);

        assert!(validate_optimize_request(&req, &nodes).is_err());
    }

    #[actix_web::test]
    async fn invalid_optimize_request_returns_bad_request() {
        let app_state = Arc::new(AppState {
            nodes: test_nodes(),
        });
        let app = actix_test::init_service(
            App::new()
                .app_data(web::Data::new(app_state))
                .route("/api/optimize", web::post().to(post_optimize)),
        )
        .await;

        let request = actix_test::TestRequest::post()
            .uri("/api/optimize")
            .set_json(serde_json::json!({
                "utility_func": "cobb_douglas",
                "decay_func": "gaussian",
                "purity_override": "default",
                "strategy": "hybrid",
                "game_phase": "phase1",
                "sigma": 200.0,
                "ignore_spawns": false,
                "weights": {}
            }))
            .to_request();
        let response = actix_test::call_service(&app, request).await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[actix_web::test]
    async fn valid_optimize_request_returns_ok() {
        let app_state = Arc::new(AppState {
            nodes: crate::data_loader::load_default_nodes(),
        });
        let app = actix_test::init_service(
            App::new()
                .app_data(web::Data::new(app_state))
                .route("/api/optimize", web::post().to(post_optimize)),
        )
        .await;

        let request = actix_test::TestRequest::post()
            .uri("/api/optimize")
            .set_json(serde_json::json!({
                "utility_func": "cobb_douglas",
                "decay_func": "gaussian",
                "purity_override": "default",
                "strategy": "hybrid",
                "game_phase": "phase1",
                "sigma": 200.0,
                "ignore_spawns": false,
                "weights": {
                    "iron": 1.0
                }
            }))
            .to_request();
        let response = actix_test::call_service(&app, request).await;

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[actix_web::test]
    async fn cors_allows_local_dashboard_origin() {
        let app = actix_test::init_service(
            App::new()
                .wrap(local_dashboard_cors())
                .route("/api/health", web::get().to(get_health)),
        )
        .await;

        let request = actix_test::TestRequest::default()
            .method(Method::OPTIONS)
            .uri("/api/health")
            .insert_header((header::ORIGIN, "http://127.0.0.1:3000"))
            .insert_header((header::ACCESS_CONTROL_REQUEST_METHOD, "GET"))
            .to_request();
        let response = actix_test::call_service(&app, request).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN),
            Some(&header::HeaderValue::from_static("http://127.0.0.1:3000"))
        );
    }

    #[actix_web::test]
    async fn cors_rejects_arbitrary_browser_origin() {
        let app = actix_test::init_service(
            App::new()
                .wrap(local_dashboard_cors())
                .route("/api/health", web::get().to(get_health)),
        )
        .await;

        let request = actix_test::TestRequest::default()
            .method(Method::OPTIONS)
            .uri("/api/health")
            .insert_header((header::ORIGIN, "https://example.com"))
            .insert_header((header::ACCESS_CONTROL_REQUEST_METHOD, "GET"))
            .to_request();
        let response = actix_test::call_service(&app, request).await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN),
            None
        );
    }
}
