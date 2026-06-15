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
    SearchStrategy, UtilityFunction, all_presets,
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

fn parse_utility(s: &str) -> Result<UtilityFunction, String> {
    match s {
        "cobb_douglas" => Ok(UtilityFunction::CobbDouglas),
        "leontief" => Ok(UtilityFunction::Leontief),
        "linear" => Ok(UtilityFunction::Linear),
        _ => Err("Invalid optimize request: unknown utility function".to_string()),
    }
}

fn parse_decay(s: &str) -> Result<DistanceDecay, String> {
    match s {
        "gaussian" => Ok(DistanceDecay::Gaussian),
        "exponential" => Ok(DistanceDecay::Exponential),
        "power_law" => Ok(DistanceDecay::PowerLaw),
        "linear" => Ok(DistanceDecay::Linear),
        "logistic" => Ok(DistanceDecay::LogisticStep),
        _ => Err("Invalid optimize request: unknown decay function".to_string()),
    }
}

fn parse_purity(s: &str) -> Result<PurityOverride, String> {
    match s {
        "default" => Ok(PurityOverride::Default),
        "impure" => Ok(PurityOverride::Impure),
        "normal" => Ok(PurityOverride::Normal),
        "pure" => Ok(PurityOverride::Pure),
        _ => Err("Invalid optimize request: unknown purity override".to_string()),
    }
}

fn parse_strategy(s: &str) -> Result<SearchStrategy, String> {
    match s {
        "hybrid" => Ok(SearchStrategy::Hybrid),
        "fast" => Ok(SearchStrategy::Fast),
        "slow" => Ok(SearchStrategy::Slow),
        _ => Err("Invalid optimize request: unknown search strategy".to_string()),
    }
}

fn parse_phase(s: &str) -> Result<GamePhase, String> {
    match s {
        "phase1" => Ok(GamePhase::Phase1),
        "phase2" => Ok(GamePhase::Phase2),
        "phase3" => Ok(GamePhase::Phase3),
        "phase4" => Ok(GamePhase::Phase4),
        "phase5" => Ok(GamePhase::Phase5),
        "collectibles" => Ok(GamePhase::Phase5),
        _ => Err("Invalid optimize request: unknown game phase".to_string()),
    }
}

struct ParsedOptimizeRequest {
    utility_func: UtilityFunction,
    decay_func: DistanceDecay,
    purity_override: PurityOverride,
    strategy: SearchStrategy,
    game_phase: GamePhase,
}

fn parse_optimize_request(req: &OptimizeRequest) -> Result<ParsedOptimizeRequest, String> {
    Ok(ParsedOptimizeRequest {
        utility_func: parse_utility(&req.utility_func)?,
        decay_func: parse_decay(&req.decay_func)?,
        purity_override: parse_purity(&req.purity_override)?,
        strategy: parse_strategy(&req.strategy)?,
        game_phase: parse_phase(&req.game_phase)?,
    })
}

fn validate_optimize_request(req: &OptimizeRequest, nodes: &[ResourceNode]) -> Result<(), String> {
    parse_optimize_request(req)?;

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
// Phase preset builder
// ---------------------------------------------------------------------------

fn build_presets() -> Vec<PresetResponse> {
    all_presets()
        .iter()
        .map(|preset| PresetResponse {
            id: preset.id.to_string(),
            name: preset.name.to_string(),
            sigma: preset.sigma,
            ignore_spawns: preset.ignore_spawns,
            weights: preset.build_weights(),
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
    let parsed = match parse_optimize_request(&req) {
        Ok(parsed) => parsed,
        Err(message) => return HttpResponse::BadRequest().body(message),
    };

    // Build OptimizerConfig from request
    let mut config = OptimizerConfig {
        sigma: req.sigma.clamp(50.0, 1000.0),
        weights: req.weights.clone(),
        purity_override: parsed.purity_override,
        strategy: parsed.strategy,
        utility_func: parsed.utility_func,
        decay_func: parsed.decay_func,
        game_phase: parsed.game_phase,
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

    #[test]
    fn api_presets_match_shared_descriptors() {
        let api_presets = build_presets();

        for descriptor in all_presets() {
            let api_preset = api_presets
                .iter()
                .find(|preset| preset.id == descriptor.id)
                .expect("shared preset missing from API presets");

            assert_eq!(api_preset.id, descriptor.id);
            assert_eq!(api_preset.sigma, descriptor.sigma);
            assert_eq!(api_preset.ignore_spawns, descriptor.ignore_spawns);
            assert_eq!(api_preset.weights, descriptor.build_weights());
            assert!(!api_preset.weights.is_empty());
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
    fn validation_rejects_unknown_utility_func() {
        let nodes = test_nodes();
        let mut req = test_request(&[("iron", 1.0)]);
        req.utility_func = "weighted_sum".to_string();

        assert_eq!(
            validate_optimize_request(&req, &nodes),
            Err("Invalid optimize request: unknown utility function".to_string())
        );
    }

    #[test]
    fn validation_rejects_unknown_decay_func() {
        let nodes = test_nodes();
        let mut req = test_request(&[("iron", 1.0)]);
        req.decay_func = "inverse_square".to_string();

        assert_eq!(
            validate_optimize_request(&req, &nodes),
            Err("Invalid optimize request: unknown decay function".to_string())
        );
    }

    #[test]
    fn validation_rejects_unknown_purity_override() {
        let nodes = test_nodes();
        let mut req = test_request(&[("iron", 1.0)]);
        req.purity_override = "mixed".to_string();

        assert_eq!(
            validate_optimize_request(&req, &nodes),
            Err("Invalid optimize request: unknown purity override".to_string())
        );
    }

    #[test]
    fn validation_rejects_unknown_strategy() {
        let nodes = test_nodes();
        let mut req = test_request(&[("iron", 1.0)]);
        req.strategy = "medium".to_string();

        assert_eq!(
            validate_optimize_request(&req, &nodes),
            Err("Invalid optimize request: unknown search strategy".to_string())
        );
    }

    #[test]
    fn validation_rejects_unknown_game_phase() {
        let nodes = test_nodes();
        let mut req = test_request(&[("iron", 1.0)]);
        req.game_phase = "phase6".to_string();

        assert_eq!(
            validate_optimize_request(&req, &nodes),
            Err("Invalid optimize request: unknown game phase".to_string())
        );
    }

    #[test]
    fn validation_accepts_current_ui_enum_values() {
        let nodes = test_nodes();

        for utility_func in ["cobb_douglas", "leontief", "linear"] {
            let mut req = test_request(&[("iron", 1.0)]);
            req.utility_func = utility_func.to_string();
            assert!(validate_optimize_request(&req, &nodes).is_ok());
        }

        for decay_func in ["gaussian", "exponential", "power_law", "linear", "logistic"] {
            let mut req = test_request(&[("iron", 1.0)]);
            req.decay_func = decay_func.to_string();
            assert!(validate_optimize_request(&req, &nodes).is_ok());
        }

        for purity_override in ["default", "impure", "normal", "pure"] {
            let mut req = test_request(&[("iron", 1.0)]);
            req.purity_override = purity_override.to_string();
            assert!(validate_optimize_request(&req, &nodes).is_ok());
        }

        for strategy in ["hybrid", "fast", "slow"] {
            let mut req = test_request(&[("iron", 1.0)]);
            req.strategy = strategy.to_string();
            assert!(validate_optimize_request(&req, &nodes).is_ok());
        }

        for game_phase in [
            "phase1",
            "phase2",
            "phase3",
            "phase4",
            "phase5",
            "collectibles",
        ] {
            let mut req = test_request(&[("iron", 1.0)]);
            req.game_phase = game_phase.to_string();
            assert!(validate_optimize_request(&req, &nodes).is_ok());
        }
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
    async fn invalid_optimize_enum_request_returns_bad_request() {
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
                "decay_func": "inverse_square",
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
