mod data_loader;
mod models;
mod optimizer;

use models::{GamePhase, OptimizerConfig, PurityOverride};
use std::collections::HashMap;
use std::env;
use std::sync::mpsc;
use std::thread;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

const PRESET_NAMES: &[&str] = &[
    "Phase 1: Early Game (Tiers 1-2)",
    "Phase 2: Steel & Coal (Tiers 3-4)",
    "Phase 3: Oil & Quartz (Tiers 5-6)",
    "Phase 4: Late Game (Tiers 7-8)",
    "Phase 5: Quantum (Tier 9)",
    "Collectible Hunting Focus",
];

const CONFIGURABLE_RESOURCES: &[&str] = &[
    "iron",
    "copper",
    "limestone",
    "coal",
    "water",
    "oil",
    "sulfur",
    "quartz",
    "caterium",
    "bauxite",
    "uranium",
    "sam",
    "nitrogenwell",
    "waterwell",
    "geyser",
    "blueslug",
    "yellowslug",
    "purpleslug",
    "mercer",
    "somersloop",
    "harddrive",
];

struct TuiState {
    sigma: f64,
    preset_idx: usize,
    purity_override: PurityOverride,
    search_strategy: models::SearchStrategy,
    utility_func: models::UtilityFunction,
    decay_func: models::DistanceDecay,
    selected_option: usize, // 0 = Preset, 1 = Purity, 2 = Strategy, 3 = Utility, 4 = Decay, 5 = Sigma, 6..26 = weights, 27 = Run button
    checklist_scroll_top: usize,
    /// Top-N optimization candidates, sorted best-first
    opt_results: Vec<optimizer::OptimizationResult>,
    /// Which candidate is currently displayed (0 = best)
    selected_candidate: usize,
    status_msg: String,
}

fn apply_preset_weights(preset_idx: usize, weights: &mut HashMap<String, f64>) {
    weights.clear();
    match preset_idx {
        0 => {
            weights.insert("iron".to_string(), 1.0);
            weights.insert("copper".to_string(), 0.8);
            weights.insert("limestone".to_string(), 0.7);
            weights.insert("coal".to_string(), 0.2); // forward-looking; unlocked at Tier 3
            weights.insert("caterium".to_string(), 0.2); // M.A.M. research value
            weights.insert("uranium".to_string(), -2.0); // severe penalty (no hazmat suit)
            weights.insert("blueslug".to_string(), 0.10);
            weights.insert("yellowslug".to_string(), 0.15);
            weights.insert("purpleslug".to_string(), 0.20);
            weights.insert("mercer".to_string(), 0.15);
            weights.insert("somersloop".to_string(), 0.15);
            weights.insert("harddrive".to_string(), 0.25);
        }
        1 => {
            weights.insert("iron".to_string(), 1.0);
            weights.insert("copper".to_string(), 0.8);
            weights.insert("limestone".to_string(), 0.7);
            weights.insert("coal".to_string(), 1.0);
            weights.insert("water".to_string(), 1.2);
            weights.insert("caterium".to_string(), 0.4);
            weights.insert("sulfur".to_string(), 0.3); // Black Powder (Nobelisk/ammo)
            weights.insert("quartz".to_string(), 0.2); // Crystal Oscillators (Tier 4)
            weights.insert("uranium".to_string(), -2.0); // radiation penalty (no hazmat suit)
            weights.insert("blueslug".to_string(), 0.08);
            weights.insert("yellowslug".to_string(), 0.12);
            weights.insert("purpleslug".to_string(), 0.18);
            weights.insert("mercer".to_string(), 0.12);
            weights.insert("somersloop".to_string(), 0.12);
            weights.insert("harddrive".to_string(), 0.20);
        }
        2 => {
            weights.insert("iron".to_string(), 0.8);
            weights.insert("copper".to_string(), 0.8);
            weights.insert("limestone".to_string(), 0.6);
            weights.insert("coal".to_string(), 0.8);
            weights.insert("water".to_string(), 0.9); // oil refinery chains are water-hungry
            weights.insert("oil".to_string(), 1.0);
            weights.insert("sulfur".to_string(), 0.6);
            weights.insert("quartz".to_string(), 0.6);
            weights.insert("caterium".to_string(), 0.6);
            weights.insert("bauxite".to_string(), 0.3); // forward-looking aluminium R&D
            weights.insert("uranium".to_string(), -2.0); // radiation penalty (no hazmat suit yet)
            weights.insert("blueslug".to_string(), 0.05);
            weights.insert("yellowslug".to_string(), 0.08);
            weights.insert("purpleslug".to_string(), 0.12);
            weights.insert("mercer".to_string(), 0.10);
            weights.insert("somersloop".to_string(), 0.10);
            weights.insert("harddrive".to_string(), 0.15);
        }
        3 => {
            weights.insert("iron".to_string(), 0.6);
            weights.insert("copper".to_string(), 0.6);
            weights.insert("limestone".to_string(), 0.5);
            weights.insert("coal".to_string(), 0.6);
            weights.insert("water".to_string(), 1.2);
            weights.insert("oil".to_string(), 0.8);
            weights.insert("sulfur".to_string(), 0.8);
            weights.insert("quartz".to_string(), 0.8);
            weights.insert("caterium".to_string(), 0.8);
            weights.insert("bauxite".to_string(), 1.0);
            weights.insert("nitrogenwell".to_string(), 0.8);
            weights.insert("geyser".to_string(), 0.8);
            // Nuclear power is primary at Tier 7-8; player has hazmat suit
            weights.insert("uranium".to_string(), 0.6);
            weights.insert("sam".to_string(), 0.6);
            weights.insert("blueslug".to_string(), 0.05);
            weights.insert("yellowslug".to_string(), 0.08);
            weights.insert("purpleslug".to_string(), 0.12);
            weights.insert("mercer".to_string(), 0.10);
            weights.insert("somersloop".to_string(), 0.10);
            weights.insert("harddrive".to_string(), 0.15);
        }
        4 => {
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
            // Ficsonium requires uranium; player has hazmat suit
            weights.insert("uranium".to_string(), 0.5);
            weights.insert("sam".to_string(), 1.0);
            weights.insert("blueslug".to_string(), 0.03);
            weights.insert("yellowslug".to_string(), 0.05);
            weights.insert("purpleslug".to_string(), 0.08);
            weights.insert("mercer".to_string(), 0.05);
            weights.insert("somersloop".to_string(), 0.05);
            weights.insert("harddrive".to_string(), 0.10);
        }
        5 => {
            weights.insert("blueslug".to_string(), 0.3);
            weights.insert("yellowslug".to_string(), 0.8);
            weights.insert("purpleslug".to_string(), 1.2);
            weights.insert("mercer".to_string(), 1.0);
            weights.insert("somersloop".to_string(), 1.0);
            weights.insert("harddrive".to_string(), 1.5);
        }
        _ => {}
    }
}

fn default_nonzero_weight(res: &str) -> f64 {
    match res {
        "uranium" => -2.0,
        "iron" => 1.0,
        "copper" => 0.8,
        "limestone" => 0.7,
        "coal" => 1.0,
        "water" => 0.8,
        "oil" => 1.0,
        "sulfur" => 0.6,
        "quartz" => 0.6,
        "caterium" => 0.4,
        "bauxite" => 1.0,
        "sam" => 0.6,
        "nitrogenwell" => 0.8,
        "waterwell" => 0.8,
        "geyser" => 0.8,
        "blueslug" => 0.1,
        "yellowslug" => 0.15,
        "purpleslug" => 0.2,
        "mercer" => 0.15,
        "somersloop" => 0.15,
        "harddrive" => 0.25,
        _ => 1.0,
    }
}

fn draw_ascii_map(
    result: Option<&optimizer::OptimizationResult>,
    width: usize,
    height: usize,
) -> Vec<Line<'static>> {
    let min_x = optimizer::MIN_X;
    let max_x = optimizer::MAX_X;
    let min_y = optimizer::MIN_Y;
    let max_y = optimizer::MAX_Y;

    // Grid matrix matching current layout view size
    let mut map_chars = vec![vec![(" ", Color::DarkGray); width]; height];

    let spawns = [
        (
            "Grass Fields",
            -110000.0,
            240000.0,
            Color::Rgb(34, 139, 34),
            ".",
        ),
        (
            "Rocky Desert",
            -200000.0,
            -200000.0,
            Color::Rgb(210, 180, 140),
            "-",
        ),
        (
            "Northern Forest",
            0.0,
            -90000.0,
            Color::Rgb(46, 139, 87),
            "*",
        ),
        (
            "Dune Desert",
            240000.0,
            -210000.0,
            Color::Rgb(244, 164, 96),
            "~",
        ),
    ];

    // Populate biome indicators
    for r in 0..height {
        for c in 0..width {
            let px = min_x + (c as f64 / (width - 1).max(1) as f64) * (max_x - min_x);
            let py = min_y + (r as f64 / (height - 1).max(1) as f64) * (max_y - min_y);

            let mut best_color = Color::DarkGray;
            let mut best_char = " ";
            let mut min_dist = f64::MAX;

            for &(_name, sx, sy, color, ch) in &spawns {
                let dx = px - sx;
                let dy = py - sy;
                let d = dx * dx + dy * dy;
                if d < min_dist {
                    min_dist = d;
                    best_color = color;
                    best_char = ch;
                }
            }

            map_chars[r][c] = (best_char, best_color);
        }
    }

    // Render major starting spawns
    for &(_name, sx, sy, _color, _ch) in &spawns {
        let c = (((sx - min_x) / (max_x - min_x) * (width - 1) as f64).round() as usize)
            .clamp(0, width - 1);
        let r = (((sy - min_y) / (max_y - min_y) * (height - 1) as f64).round() as usize)
            .clamp(0, height - 1);
        map_chars[r][c] = ("S", Color::Rgb(240, 248, 255));
    }

    // Render optimal crosshair if solved
    if let Some(res) = result {
        let c = (((res.x - min_x) / (max_x - min_x) * (width - 1) as f64).round() as usize)
            .clamp(0, width - 1);
        let r = (((res.y - min_y) / (max_y - min_y) * (height - 1) as f64).round() as usize)
            .clamp(0, height - 1);

        map_chars[r][c] = ("X", Color::Rgb(255, 69, 0));
        if c > 0 {
            map_chars[r][c - 1] = ("[", Color::Rgb(255, 69, 0));
        }
        if c < width - 1 {
            map_chars[r][c + 1] = ("]", Color::Rgb(255, 69, 0));
        }
    }

    let mut lines = Vec::new();
    for r in 0..height {
        let mut spans = Vec::new();
        for c in 0..width {
            let (ch, color) = map_chars[r][c];
            spans.push(Span::styled(ch, Style::default().fg(color)));
        }
        lines.push(Line::from(spans));
    }

    lines
}

struct RawModeGuard;

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            std::io::stdout(),
            LeaveAlternateScreen,
            DisableMouseCapture,
            crossterm::cursor::Show
        );
    }
}

fn run_tui(
    nodes: &[models::ResourceNode],
    file_info: String,
    initial_config: OptimizerConfig,
    initial_preset_idx: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let _guard = RawModeGuard;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    run_tui_loop(
        &mut terminal,
        nodes,
        file_info,
        initial_config,
        initial_preset_idx,
    )
}

fn run_tui_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    nodes: &[models::ResourceNode],
    file_info: String,
    initial_config: OptimizerConfig,
    initial_preset_idx: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let run_button_index = 6 + CONFIGURABLE_RESOURCES.len();
    let max_weight_option_index = 6 + CONFIGURABLE_RESOURCES.len() - 1;

    let mut state = TuiState {
        sigma: initial_config.sigma,
        preset_idx: initial_preset_idx,
        purity_override: initial_config.purity_override,
        search_strategy: initial_config.strategy,
        utility_func: initial_config.utility_func,
        decay_func: initial_config.decay_func,
        selected_option: 0,
        checklist_scroll_top: 0,
        opt_results: Vec::new(),
        selected_candidate: 0,
        status_msg: "Use Arrow keys to navigate, Space to toggle, Enter to optimize.".to_string(),
    };

    let mut weights = initial_config.weights.clone();

    // Initialize last_nonzero_weights map
    let mut last_nonzero_weights = HashMap::new();
    for res in CONFIGURABLE_RESOURCES {
        let val = *weights.get(*res).unwrap_or(&0.0);
        if val != 0.0 {
            last_nonzero_weights.insert(res.to_string(), val);
        } else {
            last_nonzero_weights.insert(res.to_string(), default_nonzero_weight(res));
        }
    }

    let mut solving_rx: Option<mpsc::Receiver<Vec<optimizer::OptimizationResult>>> = None;
    let mut solve_start_time: Option<std::time::Instant> = None;
    let mut spinner_frame = 0;
    const SPINNER_CHARS: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

    // Run initial optimization so map is immediately populated
    {
        let mut config = OptimizerConfig::default();
        config.sigma = state.sigma;
        config.weights = weights.clone();
        config.purity_override = state.purity_override;
        config.strategy = state.search_strategy;
        config.utility_func = state.utility_func;
        config.decay_func = state.decay_func;
        config.game_phase = match state.preset_idx {
            0 => GamePhase::Phase1,
            1 => GamePhase::Phase2,
            2 => GamePhase::Phase3,
            3 => GamePhase::Phase4,
            4 => GamePhase::Phase5,
            _ => GamePhase::Phase1,
        };
        let start_time = std::time::Instant::now();
        let results = optimizer::optimize(nodes, &config);
        let duration = start_time.elapsed();
        state.opt_results = results;
        state.selected_candidate = 0;
        state.status_msg = format!("Initial solve in {:?}", duration);
    }

    loop {
        // Handle background solver messages and tick the loading spinner
        if solving_rx.is_some() {
            spinner_frame += 1;
            let mut finished_result = None;
            if let Some(ref rx) = solving_rx {
                match rx.try_recv() {
                    Ok(result) => {
                        finished_result = Some(result);
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                    Err(mpsc::TryRecvError::Disconnected) => {
                        state.status_msg = "Solver thread crashed/disconnected.".to_string();
                        solving_rx = None;
                        solve_start_time = None;
                    }
                }
            }
            if let Some(results) = finished_result {
                state.opt_results = results;
                state.selected_candidate = 0;
                let duration = solve_start_time
                    .take()
                    .map(|t| t.elapsed())
                    .unwrap_or_default();
                state.status_msg = format!("Solved in {:?}", duration);
                solving_rx = None;
            }
        }

        terminal.draw(|f| {
            // Screen division into 2 columns
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Length(48), // Column 1: FICSIT Constraints & Weights Checklist
                    Constraint::Min(20),    // Column 2: Map & Report
                ])
                .split(f.size());

            // Build layout lines for left configuration column
            let mut left_lines = Vec::new();

            // 1. Constraints Section
            let preset_name = PRESET_NAMES[state.preset_idx];
            let phase_style = if state.selected_option == 0 {
                Style::default().fg(Color::Rgb(255, 152, 0)).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            left_lines.push(Line::from(vec![
                Span::raw(if state.selected_option == 0 { "> " } else { "  " }),
                Span::styled(format!("Preset: < {} >", preset_name), phase_style),
            ]));
            left_lines.push(Line::from(""));

            let purity_style = if state.selected_option == 1 {
                Style::default().fg(Color::Rgb(255, 152, 0)).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            left_lines.push(Line::from(vec![
                Span::raw(if state.selected_option == 1 { "> " } else { "  " }),
                Span::styled(format!("Purity: < {} >", state.purity_override.to_str()), purity_style),
            ]));
            left_lines.push(Line::from(""));

            let strategy_style = if state.selected_option == 2 {
                Style::default().fg(Color::Rgb(255, 152, 0)).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            left_lines.push(Line::from(vec![
                Span::raw(if state.selected_option == 2 { "> " } else { "  " }),
                Span::styled(format!("Strategy: < {} >", state.search_strategy.to_str()), strategy_style),
            ]));
            left_lines.push(Line::from(""));

            let utility_style = if state.selected_option == 3 {
                Style::default().fg(Color::Rgb(255, 152, 0)).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            left_lines.push(Line::from(vec![
                Span::raw(if state.selected_option == 3 { "> " } else { "  " }),
                Span::styled(format!("Utility: < {} >", state.utility_func.to_str()), utility_style),
            ]));
            left_lines.push(Line::from(""));

            let decay_style = if state.selected_option == 4 {
                Style::default().fg(Color::Rgb(255, 152, 0)).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            left_lines.push(Line::from(vec![
                Span::raw(if state.selected_option == 4 { "> " } else { "  " }),
                Span::styled(format!("Decay: < {} >", state.decay_func.to_str()), decay_style),
            ]));
            left_lines.push(Line::from(""));

            let sigma_style = if state.selected_option == 5 {
                Style::default().fg(Color::Rgb(255, 152, 0)).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            left_lines.push(Line::from(vec![
                Span::raw(if state.selected_option == 5 { "> " } else { "  " }),
                Span::styled(format!("Radius: < {} meters >", state.sigma), sigma_style),
            ]));
            left_lines.push(Line::from(""));
            left_lines.push(Line::from("─".repeat(46)));
            left_lines.push(Line::from(" PARAMETER CHECKLIST (Space to Toggle):"));
            left_lines.push(Line::from(""));

            // 2. Checklist Section (Scrollable viewport inside Left Column)
            let height = chunks[0].height as usize;
            let max_visible = (height.saturating_sub(24)).max(1);

            let end_idx = (state.checklist_scroll_top + max_visible).min(CONFIGURABLE_RESOURCES.len());
            for idx in state.checklist_scroll_top..end_idx {
                let res = CONFIGURABLE_RESOURCES[idx];
                let val = *weights.get(res).unwrap_or(&0.0);

                let is_focused = state.selected_option == 6 + idx;
                let is_enabled = val != 0.0;

                let checkbox = if is_enabled {
                    Span::styled("[X] ", Style::default().fg(Color::Rgb(50, 205, 50)).add_modifier(Modifier::BOLD))
                } else {
                    Span::styled("[ ] ", Style::default().fg(Color::Rgb(128, 128, 128)))
                };

                let res_color = match res {
                    "iron" => Color::Rgb(70, 130, 180),
                    "copper" => Color::Rgb(184, 115, 51),
                    "limestone" => Color::Rgb(245, 245, 220),
                    "coal" => Color::Rgb(105, 105, 105),
                    "water" => Color::Rgb(0, 191, 255),
                    "oil" => Color::Rgb(79, 79, 79),
                    "sulfur" => Color::Rgb(218, 165, 32),
                    "quartz" => Color::Rgb(255, 228, 225),
                    "caterium" => Color::Rgb(255, 215, 0),
                    "bauxite" => Color::Rgb(205, 92, 92),
                    "uranium" => Color::Rgb(127, 255, 0),
                    "sam" => Color::Rgb(147, 112, 219),
                    "nitrogenwell" => Color::Rgb(72, 209, 204),
                    "waterwell" => Color::Rgb(30, 144, 255),
                    "geyser" => Color::Rgb(220, 220, 220),
                    "blueslug" => Color::Rgb(0, 255, 255),
                    "yellowslug" => Color::Rgb(255, 255, 0),
                    "purpleslug" => Color::Rgb(255, 0, 255),
                    "mercer" => Color::Rgb(255, 127, 80),
                    "somersloop" => Color::Rgb(230, 230, 250),
                    "harddrive" => Color::Rgb(205, 127, 50),
                    _ => Color::Gray,
                };

                let prefix = if is_focused { "> " } else { "  " };
                let item_style = if is_focused {
                    Style::default().fg(Color::Rgb(255, 152, 0)).add_modifier(Modifier::BOLD)
                } else if is_enabled {
                    Style::default()
                } else {
                    Style::default().fg(Color::Rgb(100, 100, 100))
                };

                let padded_name = format!("{:<12}", res);

                let weight_span = if val < 0.0 {
                    Span::styled(format!("{:.1}", val), Style::default().fg(Color::Rgb(255, 69, 0)).add_modifier(Modifier::BOLD))
                } else if is_enabled {
                    Span::styled(format!("{:.1}", val), Style::default().fg(res_color))
                } else {
                    Span::styled("0.0", Style::default().fg(Color::Rgb(128, 128, 128)))
                };

                left_lines.push(Line::from(vec![
                    Span::raw(prefix),
                    checkbox,
                    Span::styled(padded_name, item_style),
                    weight_span,
                ]));
            }

            // Fill empty checklist lines with blank space to keep layout static
            let actual_visible = end_idx - state.checklist_scroll_top;
            for _ in actual_visible..max_visible {
                left_lines.push(Line::from(""));
            }

            left_lines.push(Line::from(""));
            left_lines.push(Line::from("─".repeat(46)));

            // 3. Run Optimization Button
            let run_style = if state.selected_option == run_button_index {
                Style::default().bg(Color::Rgb(50, 205, 50)).fg(Color::Black).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Rgb(50, 205, 50))
            };
            left_lines.push(Line::from(""));
            left_lines.push(Line::from(vec![
                Span::raw(if state.selected_option == run_button_index { "> " } else { "  " }),
                Span::styled("   [ RUN OPTIMIZATION ENGINE ]   ", run_style),
            ]));
            left_lines.push(Line::from(""));

            // 4. File info
            left_lines.push(Line::from(vec![
                Span::raw("  File: "),
                Span::styled(file_info.clone(), Style::default().fg(Color::Rgb(0, 191, 255))),
            ]));
            left_lines.push(Line::from(vec![
                Span::raw("  Nodes Loaded: "),
                Span::styled(format!("{}", nodes.len()), Style::default().fg(Color::Rgb(255, 215, 0))),
            ]));

            let left_para = Paragraph::new(left_lines)
                .block(Block::default().title(" FICSIT CONFIGURATION & WEIGHTS ").borders(Borders::ALL));
            f.render_widget(left_para, chunks[0]);

            // Column 2 Layout (Right Pane: Map & Report)
            let main_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(10),
                    Constraint::Length(12),
                ])
                .split(chunks[1]);

            let map_block = Block::default()
                .title(" FICSIT MAP INTERACTIVE ASCII OVERLAY ")
                .borders(Borders::ALL);
            f.render_widget(map_block.clone(), main_chunks[0]);

            let map_inner = map_block.inner(main_chunks[0]);
            let map_sub_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(1),
                    Constraint::Length(1),
                ])
                .split(map_inner);

            let map_lines = draw_ascii_map(
                state.opt_results.get(state.selected_candidate),
                map_sub_chunks[0].width as usize,
                map_sub_chunks[0].height as usize,
            );
            let map_para = Paragraph::new(map_lines);
            f.render_widget(map_para, map_sub_chunks[0]);

            let legend_line = Line::from(vec![
                Span::raw("Legend: "),
                Span::styled(".", Style::default().fg(Color::Rgb(34, 139, 34)).add_modifier(Modifier::BOLD)),
                Span::raw(" Grass Fields  "),
                Span::styled("-", Style::default().fg(Color::Rgb(210, 180, 140)).add_modifier(Modifier::BOLD)),
                Span::raw(" Rocky Desert  "),
                Span::styled("*", Style::default().fg(Color::Rgb(46, 139, 87)).add_modifier(Modifier::BOLD)),
                Span::raw(" Northern Forest  "),
                Span::styled("~", Style::default().fg(Color::Rgb(244, 164, 96)).add_modifier(Modifier::BOLD)),
                Span::raw(" Dune Desert  "),
                Span::styled("S", Style::default().fg(Color::Rgb(240, 248, 255)).add_modifier(Modifier::BOLD)),
                Span::raw(" Spawn  "),
                Span::styled("[X]", Style::default().fg(Color::Rgb(255, 69, 0)).add_modifier(Modifier::BOLD)),
                Span::raw(" Target"),
            ]);
            let legend_para = Paragraph::new(legend_line);
            f.render_widget(legend_para, map_sub_chunks[1]);

            // Bottom reports block in Column 2
            let mut results_lines = Vec::new();
            let solving_in_progress = solving_rx.is_some();
            if solving_in_progress {
                let spin_char = SPINNER_CHARS[spinner_frame % SPINNER_CHARS.len()];
                results_lines.push(Line::from(vec![
                    Span::styled(format!(" {} MATHEMATICAL SOLVER RUNNING...", spin_char), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                ]));
                if let Some(start) = solve_start_time {
                    results_lines.push(Line::from(vec![
                        Span::raw(format!("  Elapsed Time: {:.1}s", start.elapsed().as_secs_f64())),
                    ]));
                }
                results_lines.push(Line::from("  Please wait while rayon parallelizes search over your CPU cores."));
            } else if let Some(res) = state.opt_results.get(state.selected_candidate) {
                let total_candidates = state.opt_results.len();
                let candidate_num = state.selected_candidate + 1;

                // --- Candidate header ---
                results_lines.push(Line::from(vec![
                    Span::styled(
                        format!("CANDIDATE #{}/{} — {}:", candidate_num, total_candidates, res.closest_spawn.name),
                        Style::default().fg(Color::Rgb(50, 205, 50)).add_modifier(Modifier::BOLD),
                    ),
                    if total_candidates > 1 {
                        Span::styled(
                            format!(" (Use [ / ] or Left/Right to cycle candidates)"),
                            Style::default().fg(Color::Rgb(180, 180, 180)),
                        )
                    } else {
                        Span::raw("")
                    },
                ]));
                results_lines.push(Line::from(vec![
                    Span::raw(format!("  Coordinate: X: {:.0}m | Y: {:.0}m | Z: {:.0}m", res.x / 100.0, res.y / 100.0, res.z / 100.0)),
                ]));
                results_lines.push(Line::from(vec![
                    Span::raw(format!("  Utility Score: {:.6}  |  Spawn Distance: {:.0}m", res.score, res.spawn_distance)),
                ]));

                // --- Terrain quality block ---
                let build_radius = 1.5 * state.sigma;
                let mut local_heights = Vec::new();
                for node in nodes {
                    let dx = (res.x - node.x) / 100.0;
                    let dy = (res.y - node.y) / 100.0;
                    let d = (dx * dx + dy * dy).sqrt();
                    if d <= build_radius {
                        local_heights.push(node.z);
                    }
                }
                let mut flatness_score = 1.0;
                let mut std_dev_m = 0.0;
                if local_heights.len() > 1 {
                    let sum: f64 = local_heights.iter().sum();
                    let mean = sum / local_heights.len() as f64;
                    let variance_sum: f64 = local_heights.iter().map(|&z| {
                        let diff = z - mean;
                        diff * diff
                    }).sum();
                    let std_dev_cm = (variance_sum / local_heights.len() as f64).sqrt();
                    std_dev_m = std_dev_cm / 100.0;
                    flatness_score = (-std_dev_m / 30.0).exp();
                }

                let terrain_label = if flatness_score >= 0.80 {
                    ("Excellent", Color::Rgb(50, 205, 50))
                } else if flatness_score >= 0.60 {
                    ("Moderate", Color::Rgb(255, 215, 0))
                } else if flatness_score >= 0.40 {
                    ("Difficult", Color::Rgb(255, 165, 0))
                } else {
                    ("Severe", Color::Rgb(255, 69, 0))
                };
                results_lines.push(Line::from(vec![
                    Span::raw(format!("  Flatness: {:.1}% (σ={:.1}m)  ", flatness_score * 100.0, std_dev_m)),
                    Span::styled(format!("[{}]", terrain_label.0), Style::default().fg(terrain_label.1).add_modifier(Modifier::BOLD)),
                    Span::raw(format!("  TRI: {:.1}m  Diversity: {:.2}",
                        res.terrain_ruggedness, res.diversity_score)),
                ]));

                // Terrain difficulty warning for high-ruggedness areas
                if flatness_score < 0.60 {
                    results_lines.push(Line::from(vec![
                        Span::styled(
                            "  ⚠ High terrain complexity — expect significant foundation costs for large bases.",
                            Style::default().fg(Color::Rgb(255, 165, 0)),
                        ),
                    ]));
                }

                // --- Resource yield breakdown ---
                results_lines.push(Line::from(""));
                results_lines.push(Line::from(vec![
                    Span::styled("WEIGHTED YIELDS:", Style::default().fg(Color::Rgb(0, 191, 255)).add_modifier(Modifier::BOLD)),
                ]));
                let mut sorted_yields: Vec<(&String, &f64)> = res.resource_yields.iter().collect();
                sorted_yields.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));
                let mut yield_line_items: Vec<String> = Vec::new();
                for (name, yield_val) in sorted_yields.iter().take(9) {
                    yield_line_items.push(format!("{}: {:.2}", name, yield_val));
                }
                // Print in rows of 3
                for chunk in yield_line_items.chunks(3) {
                    let mut spans = vec![Span::raw("  ")];
                    for (i, item) in chunk.iter().enumerate() {
                        spans.push(Span::raw(item.clone()));
                        if i < chunk.len() - 1 {
                            spans.push(Span::raw(" | "));
                        }
                    }
                    results_lines.push(Line::from(spans));
                }

                // --- Local node inventory ---
                results_lines.push(Line::from(""));
                results_lines.push(Line::from(vec![
                    Span::styled("LOCAL NODES (accessible):", Style::default().fg(Color::Rgb(255, 152, 0)).add_modifier(Modifier::BOLD)),
                ]));
                let mut sorted_nodes: Vec<_> = res.local_nodes.iter().collect();
                sorted_nodes.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
                let mut current_line: Vec<String> = Vec::new();
                for (name, count) in &sorted_nodes {
                    if current_line.len() >= 3 {
                        let mut spans = vec![Span::raw("  ")];
                        for (i, item) in current_line.iter().enumerate() {
                            spans.push(Span::raw(item.to_string()));
                            if i < current_line.len() - 1 {
                                spans.push(Span::raw(" | "));
                            }
                        }
                        results_lines.push(Line::from(spans));
                        current_line.clear();
                    }
                    current_line.push(format!("{}x {}", count, name));
                }
                if !current_line.is_empty() {
                    let mut spans = vec![Span::raw("  ")];
                    for (i, item) in current_line.iter().enumerate() {
                        spans.push(Span::raw(item.to_string()));
                        if i < current_line.len() - 1 {
                            spans.push(Span::raw(" | "));
                        }
                    }
                    results_lines.push(Line::from(spans));
                }

                // Obstructed nodes (Phase 1/2 only) — shown as warning
                if !res.obstructed_nodes.is_empty() {
                    results_lines.push(Line::from(vec![
                        Span::styled(
                            "  [Nobelisk locked]: ",
                            Style::default().fg(Color::Rgb(255, 69, 0)),
                        ),
                        Span::raw(
                            res.obstructed_nodes.iter()
                                .map(|(k, v)| format!("{}x {}", v, k))
                                .collect::<Vec<_>>()
                                .join(" | ")
                        ),
                    ]));
                }
            } else {
                results_lines.push(Line::from("No optimization solved yet. Press ENTER on [ RUN ] to execute."));
            }

            results_lines.push(Line::from(""));
            results_lines.push(Line::from(vec![
                Span::styled("Controls: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw("Up/Down: navigate | Left/Right: adjust | Space: toggle weight | Enter: RUN | [ ]: cycle candidates | Q: quit"),
            ]));
            results_lines.push(Line::from(""));
            results_lines.push(Line::from(vec![
                Span::styled("Status: ", Style::default().fg(Color::Rgb(255, 215, 0)).add_modifier(Modifier::BOLD)),
                Span::raw(state.status_msg.clone()),
            ]));

            let results_para = Paragraph::new(results_lines)
                .block(Block::default().title(" OPTIMIZATION REPORT & CONTROLS ").borders(Borders::ALL));
            f.render_widget(results_para, main_chunks[1]);
        })?;

        // Handle keys
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != event::KeyEventKind::Press {
                    continue;
                }
                if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                    break;
                }

                if solving_rx.is_some() {
                    continue;
                }

                // Get dynamic layout sizing to handle scrolling limits in navigation keys
                let size = terminal.size()?;
                let layout_chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Length(48), Constraint::Min(20)])
                    .split(size);

                match key.code {
                    KeyCode::Up => {
                        if state.selected_option > 0 {
                            state.selected_option -= 1;

                            // Adjust scroll top if focused on checklist
                            if state.selected_option >= 6
                                && state.selected_option <= max_weight_option_index
                            {
                                let item_idx = state.selected_option - 6;
                                if item_idx < state.checklist_scroll_top {
                                    state.checklist_scroll_top = item_idx;
                                }
                            }
                        }
                    }
                    KeyCode::Down => {
                        if state.selected_option < run_button_index {
                            state.selected_option += 1;

                            // Adjust scroll top if focused on checklist
                            if state.selected_option >= 6
                                && state.selected_option <= max_weight_option_index
                            {
                                let item_idx = state.selected_option - 6;
                                let max_visible =
                                    (layout_chunks[0].height as usize).saturating_sub(24).max(1);
                                if max_visible > 0
                                    && item_idx >= state.checklist_scroll_top + max_visible
                                {
                                    state.checklist_scroll_top = item_idx + 1 - max_visible;
                                }
                            }
                        }
                    }
                    KeyCode::PageUp => {
                        if state.selected_option > 0 {
                            state.selected_option = state.selected_option.saturating_sub(5);

                            // Adjust scroll top if focused on checklist
                            if state.selected_option >= 6
                                && state.selected_option <= max_weight_option_index
                            {
                                let item_idx = state.selected_option - 6;
                                if item_idx < state.checklist_scroll_top {
                                    state.checklist_scroll_top = item_idx;
                                }
                            } else if state.selected_option < 6 {
                                state.checklist_scroll_top = 0;
                            }
                        }
                    }
                    KeyCode::PageDown => {
                        if state.selected_option < run_button_index {
                            state.selected_option =
                                (state.selected_option + 5).min(run_button_index);

                            // Adjust scroll top if focused on checklist
                            if state.selected_option >= 6
                                && state.selected_option <= max_weight_option_index
                            {
                                let item_idx = state.selected_option - 6;
                                let max_visible =
                                    (layout_chunks[0].height as usize).saturating_sub(24).max(1);
                                if max_visible > 0
                                    && item_idx >= state.checklist_scroll_top + max_visible
                                {
                                    state.checklist_scroll_top = item_idx + 1 - max_visible;
                                }
                            }
                        }
                    }
                    KeyCode::Home => {
                        state.selected_option = 0;
                        state.checklist_scroll_top = 0;
                    }
                    KeyCode::End => {
                        state.selected_option = run_button_index;
                        let max_visible =
                            (layout_chunks[0].height as usize).saturating_sub(24).max(1);
                        if CONFIGURABLE_RESOURCES.len() > max_visible {
                            state.checklist_scroll_top = CONFIGURABLE_RESOURCES.len() - max_visible;
                        }
                    }
                    KeyCode::Left => {
                        if state.selected_option == 0 {
                            if state.preset_idx > 0 {
                                state.preset_idx -= 1;
                                apply_preset_weights(state.preset_idx, &mut weights);
                            }
                        } else if state.selected_option == 1 {
                            let modes = [
                                models::PurityOverride::Default,
                                models::PurityOverride::Impure,
                                models::PurityOverride::Normal,
                                models::PurityOverride::Pure,
                            ];
                            let mut curr_idx = modes
                                .iter()
                                .position(|&m| m == state.purity_override)
                                .unwrap_or(0);
                            if curr_idx > 0 {
                                curr_idx -= 1;
                            } else {
                                curr_idx = 3;
                            }
                            state.purity_override = modes[curr_idx];
                        } else if state.selected_option == 2 {
                            let strategies = [
                                models::SearchStrategy::Hybrid,
                                models::SearchStrategy::Fast,
                                models::SearchStrategy::Slow,
                            ];
                            let mut curr_idx = strategies
                                .iter()
                                .position(|&s| s == state.search_strategy)
                                .unwrap_or(0);
                            if curr_idx > 0 {
                                curr_idx -= 1;
                            } else {
                                curr_idx = 2;
                            }
                            state.search_strategy = strategies[curr_idx];
                        } else if state.selected_option == 3 {
                            let funcs = [
                                models::UtilityFunction::CobbDouglas,
                                models::UtilityFunction::Leontief,
                                models::UtilityFunction::Linear,
                            ];
                            let mut curr_idx = funcs
                                .iter()
                                .position(|&f| f == state.utility_func)
                                .unwrap_or(0);
                            if curr_idx > 0 {
                                curr_idx -= 1;
                            } else {
                                curr_idx = 2;
                            }
                            state.utility_func = funcs[curr_idx];
                        } else if state.selected_option == 4 {
                            let decays = [
                                models::DistanceDecay::Gaussian,
                                models::DistanceDecay::Exponential,
                                models::DistanceDecay::PowerLaw,
                                models::DistanceDecay::Linear,
                                models::DistanceDecay::LogisticStep,
                            ];
                            let mut curr_idx = decays
                                .iter()
                                .position(|&d| d == state.decay_func)
                                .unwrap_or(0);
                            if curr_idx > 0 {
                                curr_idx -= 1;
                            } else {
                                curr_idx = 4;
                            }
                            state.decay_func = decays[curr_idx];
                        } else if state.selected_option == 5 {
                            if state.sigma > 150.0 {
                                state.sigma -= 50.0;
                            }
                        } else if state.selected_option >= 6
                            && state.selected_option <= max_weight_option_index
                        {
                            let res_name = CONFIGURABLE_RESOURCES[state.selected_option - 6];
                            let val = weights.entry(res_name.to_string()).or_insert(0.0);
                            *val = (*val - 0.1).clamp(-10.0, 10.0);
                            *val = (*val * 10.0).round() / 10.0;
                            if *val != 0.0 {
                                last_nonzero_weights.insert(res_name.to_string(), *val);
                            }
                        }
                    }
                    KeyCode::Right => {
                        if state.selected_option == 0 {
                            if state.preset_idx + 1 < PRESET_NAMES.len() {
                                state.preset_idx += 1;
                                apply_preset_weights(state.preset_idx, &mut weights);
                            }
                        } else if state.selected_option == 1 {
                            let modes = [
                                models::PurityOverride::Default,
                                models::PurityOverride::Impure,
                                models::PurityOverride::Normal,
                                models::PurityOverride::Pure,
                            ];
                            let mut curr_idx = modes
                                .iter()
                                .position(|&m| m == state.purity_override)
                                .unwrap_or(0);
                            if curr_idx < 3 {
                                curr_idx += 1;
                            } else {
                                curr_idx = 0;
                            }
                            state.purity_override = modes[curr_idx];
                        } else if state.selected_option == 2 {
                            let strategies = [
                                models::SearchStrategy::Hybrid,
                                models::SearchStrategy::Fast,
                                models::SearchStrategy::Slow,
                            ];
                            let mut curr_idx = strategies
                                .iter()
                                .position(|&s| s == state.search_strategy)
                                .unwrap_or(0);
                            if curr_idx < 2 {
                                curr_idx += 1;
                            } else {
                                curr_idx = 0;
                            }
                            state.search_strategy = strategies[curr_idx];
                        } else if state.selected_option == 3 {
                            let funcs = [
                                models::UtilityFunction::CobbDouglas,
                                models::UtilityFunction::Leontief,
                                models::UtilityFunction::Linear,
                            ];
                            let mut curr_idx = funcs
                                .iter()
                                .position(|&f| f == state.utility_func)
                                .unwrap_or(0);
                            if curr_idx < 2 {
                                curr_idx += 1;
                            } else {
                                curr_idx = 0;
                            }
                            state.utility_func = funcs[curr_idx];
                        } else if state.selected_option == 4 {
                            let decays = [
                                models::DistanceDecay::Gaussian,
                                models::DistanceDecay::Exponential,
                                models::DistanceDecay::PowerLaw,
                                models::DistanceDecay::Linear,
                                models::DistanceDecay::LogisticStep,
                            ];
                            let mut curr_idx = decays
                                .iter()
                                .position(|&d| d == state.decay_func)
                                .unwrap_or(0);
                            if curr_idx < 4 {
                                curr_idx += 1;
                            } else {
                                curr_idx = 0;
                            }
                            state.decay_func = decays[curr_idx];
                        } else if state.selected_option == 5 {
                            if state.sigma < 1500.0 {
                                state.sigma += 50.0;
                            }
                        } else if state.selected_option >= 6
                            && state.selected_option <= max_weight_option_index
                        {
                            let res_name = CONFIGURABLE_RESOURCES[state.selected_option - 6];
                            let val = weights.entry(res_name.to_string()).or_insert(0.0);
                            *val = (*val + 0.1).clamp(-10.0, 10.0);
                            *val = (*val * 10.0).round() / 10.0;
                            if *val != 0.0 {
                                last_nonzero_weights.insert(res_name.to_string(), *val);
                            }
                        }
                    }
                    KeyCode::Char(' ') => {
                        if state.selected_option >= 6
                            && state.selected_option <= max_weight_option_index
                        {
                            let res_name = CONFIGURABLE_RESOURCES[state.selected_option - 6];
                            let val = weights.entry(res_name.to_string()).or_insert(0.0);
                            if *val == 0.0 {
                                let restored = last_nonzero_weights
                                    .get(res_name)
                                    .copied()
                                    .unwrap_or_else(|| default_nonzero_weight(res_name));
                                *val = restored;
                            } else {
                                last_nonzero_weights.insert(res_name.to_string(), *val);
                                *val = 0.0;
                            }
                        }
                    }
                    KeyCode::Enter => {
                        if state.selected_option == run_button_index {
                            if solving_rx.is_some() {
                                continue;
                            }
                            state.status_msg = "Running mathematical solver...".to_string();
                            solve_start_time = Some(std::time::Instant::now());

                            let mut config = OptimizerConfig::default();
                            config.sigma = state.sigma;
                            config.weights = weights.clone();
                            config.purity_override = state.purity_override;
                            config.strategy = state.search_strategy;
                            config.utility_func = state.utility_func;
                            config.decay_func = state.decay_func;
                            config.game_phase = match state.preset_idx {
                                0 => GamePhase::Phase1,
                                1 => GamePhase::Phase2,
                                2 => GamePhase::Phase3,
                                3 => GamePhase::Phase4,
                                4 => GamePhase::Phase5,
                                _ => GamePhase::Phase1,
                            };

                            let (tx, rx) = mpsc::channel();
                            solving_rx = Some(rx);

                            let nodes_clone = nodes.to_vec();
                            thread::spawn(move || {
                                let results = optimizer::optimize(&nodes_clone, &config);
                                let _ = tx.send(results);
                            });
                        }
                    }
                    // Cycle through top-N candidates with [ and ] keys
                    KeyCode::Char('[') => {
                        if !state.opt_results.is_empty() && state.selected_candidate > 0 {
                            state.selected_candidate -= 1;
                            state.status_msg = format!(
                                "Viewing candidate #{} of {}",
                                state.selected_candidate + 1,
                                state.opt_results.len()
                            );
                        }
                    }
                    KeyCode::Char(']') => {
                        if !state.opt_results.is_empty()
                            && state.selected_candidate + 1 < state.opt_results.len()
                        {
                            state.selected_candidate += 1;
                            state.status_msg = format!(
                                "Viewing candidate #{} of {}",
                                state.selected_candidate + 1,
                                state.opt_results.len()
                            );
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

fn print_help() {
    println!(
        r#"
FICSIT START OPTIMIZER (RUST VERSION)
--------------------------------------
Calculates the mathematically optimal starting location in Satisfactory 1.0.

Usage:
  satisfactory-start-optimizer [options]

Options:
  --file <path>        Load resource nodes from a custom JSON file
  --sigma <meters>     Logistical walking radius in meters (default: 700)
  --tier <1-5|early|steel|oil|late|quantum>
                       Select game phase preset for non-interactive mode.
  --purity <default|impure|normal|pure>
                       Override database node purity multipliers (default: default)
  --strategy <hybrid|fast|slow>
                       Select search algorithm strategy (default: hybrid)
  --utility <cobbdouglas|leontief|linear>
                       Select utility calculation function (default: cobbdouglas)
  --decay <gaussian|exponential|powerlaw|linear>
                       Select distance decay function (default: gaussian)
  --collectibles       Focus search purely on slugs, drop pods, and alien artifacts
  --json               Output only raw JSON configuration and results
  --<resource> <w>     Dynamic weight of any resource type (e.g. --iron 1.5, --uranium -2.0)
  --help               Show this help menu
"#
    );
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help();
        return;
    }

    let mut custom_file_path: Option<String> = None;
    let mut config = OptimizerConfig::default();
    // Default weights to the first preset (Phase 1)
    GamePhase::Phase1.apply_weights(&mut config.weights);
    let mut active_phase: Option<GamePhase> = None;
    let mut is_collectibles_mode = false;
    let mut output_json = false;
    let mut run_simulation = false;

    // Parse command line arguments
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--file" => {
                if i + 1 < args.len() {
                    custom_file_path = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    eprintln!("Error: --file requires a path value");
                    return;
                }
            }
            "--sigma" => {
                if i + 1 < args.len() {
                    if let Ok(val) = args[i + 1].parse::<f64>() {
                        if val <= 0.0 {
                            eprintln!("Error: --sigma value must be greater than 0.0");
                            return;
                        }
                        config.sigma = val;
                    } else {
                        eprintln!("Error: --sigma requires a numeric value");
                        return;
                    }
                    i += 2;
                } else {
                    eprintln!("Error: --sigma requires a numeric value");
                    return;
                }
            }
            "--purity" => {
                if i + 1 < args.len() {
                    config.purity_override = match args[i + 1].to_lowercase().as_str() {
                        "impure" => PurityOverride::Impure,
                        "normal" => PurityOverride::Normal,
                        "pure" => PurityOverride::Pure,
                        _ => PurityOverride::Default,
                    };
                    i += 2;
                } else {
                    eprintln!("Error: --purity requires a value");
                    return;
                }
            }
            "--strategy" => {
                if i + 1 < args.len() {
                    config.strategy = match args[i + 1].to_lowercase().as_str() {
                        "fast" => models::SearchStrategy::Fast,
                        "slow" => models::SearchStrategy::Slow,
                        _ => models::SearchStrategy::Hybrid,
                    };
                    i += 2;
                } else {
                    eprintln!("Error: --strategy requires a value");
                    return;
                }
            }
            "--utility" => {
                if i + 1 < args.len() {
                    let val = args[i + 1].to_lowercase();
                    config.utility_func = match val.as_str() {
                        "cobbdouglas" | "cobb-douglas" | "cobb_douglas" => {
                            models::UtilityFunction::CobbDouglas
                        }
                        "leontief" => models::UtilityFunction::Leontief,
                        "linear" => models::UtilityFunction::Linear,
                        _ => {
                            eprintln!(
                                "Error: Invalid utility function value '{}'. Choose from: cobbdouglas, leontief, linear",
                                args[i + 1]
                            );
                            return;
                        }
                    };
                    i += 2;
                } else {
                    eprintln!("Error: --utility requires a value");
                    return;
                }
            }
            "--decay" => {
                if i + 1 < args.len() {
                    let val = args[i + 1].to_lowercase();
                    config.decay_func = match val.as_str() {
                        "gaussian" => models::DistanceDecay::Gaussian,
                        "exponential" => models::DistanceDecay::Exponential,
                        "powerlaw" | "power-law" | "power_law" | "gravity" => {
                            models::DistanceDecay::PowerLaw
                        }
                        "linear" => models::DistanceDecay::Linear,
                        "logisticstep" | "step" => models::DistanceDecay::LogisticStep,
                        _ => {
                            eprintln!(
                                "Error: Invalid decay function value '{}'. Choose from: gaussian, exponential, powerlaw, linear",
                                args[i + 1]
                            );
                            return;
                        }
                    };
                    i += 2;
                } else {
                    eprintln!("Error: --decay requires a value");
                    return;
                }
            }
            "--tier" | "--phase" => {
                if i + 1 < args.len() {
                    let phase_str = &args[i + 1];
                    if let Some(phase) = GamePhase::from_str(phase_str) {
                        phase.apply_weights(&mut config.weights);
                        config.game_phase = phase;
                        active_phase = Some(phase);
                    } else {
                        eprintln!("Error: Invalid phase/tier value '{}'.", phase_str);
                        return;
                    }
                    i += 2;
                } else {
                    eprintln!("Error: --tier requires a value");
                    return;
                }
            }
            "--collectibles" => {
                config.weights.clear();
                config.weights.insert("blueslug".to_string(), 0.5);
                config.weights.insert("yellowslug".to_string(), 0.8);
                config.weights.insert("purpleslug".to_string(), 1.2);
                config.weights.insert("mercer".to_string(), 1.0);
                config.weights.insert("somersloop".to_string(), 1.0);
                config.weights.insert("harddrive".to_string(), 1.5);
                is_collectibles_mode = true;
                i += 1;
            }
            "--json" => {
                output_json = true;
                i += 1;
            }
            "--simulate-all" => {
                run_simulation = true;
                i += 1;
            }
            flag if flag.starts_with("--") => {
                let resource_name = flag.trim_start_matches("--").to_string();
                if i + 1 < args.len() {
                    if let Ok(val) = args[i + 1].parse::<f64>() {
                        config.weights.insert(resource_name, val);
                    } else {
                        eprintln!("Error: weight value for {} must be a float", flag);
                        return;
                    }
                    i += 2;
                } else {
                    eprintln!("Error: {} requires a numeric value", flag);
                    return;
                }
            }
            _ => {
                eprintln!("Unknown argument: {}", args[i]);
                print_help();
                return;
            }
        }
    }

    if run_simulation {
        let nodes = match &custom_file_path {
            Some(path) => match data_loader::load_nodes_from_file(path) {
                Ok(n) => n,
                Err(e) => {
                    eprintln!("Error loading JSON file: {}", e);
                    return;
                }
            },
            None => data_loader::load_default_nodes(),
        };
        run_full_simulation_matrix(&nodes);
        return;
    }

    if output_json {
        let nodes = match &custom_file_path {
            Some(path) => match data_loader::load_nodes_from_file(path) {
                Ok(n) => n,
                Err(e) => {
                    eprintln!("Error loading JSON file: {}", e);
                    return;
                }
            },
            None => data_loader::load_default_nodes(),
        };
        let result = optimizer::optimize(&nodes, &config);
        if let Ok(json_str) = serde_json::to_string_pretty(&result) {
            println!("{}", json_str);
        }
        return;
    }

    // Launch interactive TUI
    let nodes = match &custom_file_path {
        Some(path) => match data_loader::load_nodes_from_file(path) {
            Ok(n) => n,
            Err(e) => {
                eprintln!("Error loading JSON file: {}", e);
                return;
            }
        },
        None => data_loader::load_default_nodes(),
    };

    let file_info = match &custom_file_path {
        Some(path) => path.clone(),
        None => "Embedded Database (2372 nodes)".to_string(),
    };

    let initial_preset_idx = if is_collectibles_mode {
        5
    } else if let Some(phase) = active_phase {
        match phase {
            GamePhase::Phase1 => 0,
            GamePhase::Phase2 => 1,
            GamePhase::Phase3 => 2,
            GamePhase::Phase4 => 3,
            GamePhase::Phase5 => 4,
        }
    } else {
        0 // Default to Phase 1
    };

    if let Err(e) = run_tui(&nodes, file_info, config, initial_preset_idx) {
        eprintln!("Terminal UI dashboard error: {:?}", e);
    }
}

fn run_full_simulation_matrix(nodes: &[models::ResourceNode]) {
    use std::collections::HashMap;
    use std::fs::File;
    use std::io::Write;

    struct SimConfig {
        preset_idx: usize,
        purity: PurityOverride,
        utility: models::UtilityFunction,
        decay: models::DistanceDecay,
        sigma: f64,
    }

    let mut sim_configs = Vec::new();
    let presets = 0..6;
    let purities = [
        PurityOverride::Default,
        PurityOverride::Normal,
        PurityOverride::Pure,
    ];
    let utilities = [
        models::UtilityFunction::CobbDouglas,
        models::UtilityFunction::Leontief,
        models::UtilityFunction::Linear,
    ];
    let decays = [
        models::DistanceDecay::Gaussian,
        models::DistanceDecay::Exponential,
        models::DistanceDecay::PowerLaw,
        models::DistanceDecay::Linear,
        models::DistanceDecay::LogisticStep,
    ];
    let sigma = 700.0;

    for preset_idx in presets {
        for &purity in &purities {
            for &utility in &utilities {
                for &decay in &decays {
                    sim_configs.push(SimConfig {
                        preset_idx,
                        purity,
                        utility,
                        decay,
                        sigma,
                    });
                }
            }
        }
    }

    println!(
        "Running {} simulation configurations sequentially (each internally parallelized)...",
        sim_configs.len()
    );
    let results: Vec<(SimConfig, optimizer::OptimizationResult)> = sim_configs
        .into_iter()
        .map(|config| {
            let mut opt_config = OptimizerConfig {
                sigma: config.sigma,
                weights: HashMap::new(),
                purity_override: config.purity,
                strategy: models::SearchStrategy::Hybrid,
                utility_func: config.utility,
                decay_func: config.decay,
                game_phase: match config.preset_idx {
                    0 => models::GamePhase::Phase1,
                    1 => models::GamePhase::Phase2,
                    2 => models::GamePhase::Phase3,
                    3 => models::GamePhase::Phase4,
                    4 => models::GamePhase::Phase5,
                    _ => models::GamePhase::Phase1,
                },
            };
            apply_preset_weights(config.preset_idx, &mut opt_config.weights);
            // Simulation mode: take the single best result (#1 candidate) from the Vec
            let mut all_res = optimizer::optimize(nodes, &opt_config);
            let res = all_res.remove(0);
            (config, res)
        })
        .collect();

    // Write CSV
    let csv_path = "simulation_results.csv";
    let mut file = File::create(csv_path).expect("Failed to create CSV file");
    writeln!(
        file,
        "Preset,Purity,Utility,Decay,Radius,X,Y,Z,Score,Spawn,SpawnDistance"
    )
    .unwrap();
    for (conf, res) in &results {
        let preset_name = match conf.preset_idx {
            0 => "Phase 1: Early Game",
            1 => "Phase 2: Steel & Coal",
            2 => "Phase 3: Oil & Quartz",
            3 => "Phase 4: Late Game",
            4 => "Phase 5: Quantum",
            5 => "Collectible Hunting",
            _ => "Unknown",
        };
        let purity_str = match conf.purity {
            PurityOverride::Default => "Default",
            PurityOverride::Impure => "Impure",
            PurityOverride::Normal => "Normal",
            PurityOverride::Pure => "Pure",
        };
        let utility_str = match conf.utility {
            models::UtilityFunction::CobbDouglas => "Cobb-Douglas",
            models::UtilityFunction::Leontief => "Leontief",
            models::UtilityFunction::Linear => "Linear",
        };
        let decay_str = match conf.decay {
            models::DistanceDecay::Gaussian => "Gaussian",
            models::DistanceDecay::Exponential => "Exponential",
            models::DistanceDecay::PowerLaw => "Power-Law",
            models::DistanceDecay::Linear => "Linear",
            models::DistanceDecay::LogisticStep => "Logistic-Step",
        };
        writeln!(
            file,
            "\"{}\",\"{}\",\"{}\",\"{}\",{},{:.2},{:.2},{:.2},{:.4},\"{}\",{:.2}",
            preset_name,
            purity_str,
            utility_str,
            decay_str,
            conf.sigma,
            res.x,
            res.y,
            res.z,
            res.score,
            res.closest_spawn.name,
            res.spawn_distance
        )
        .unwrap();
    }
    println!(
        "Saved raw simulation dataset ({} rows) to {}",
        results.len(),
        csv_path
    );

    // Compute analysis
    let mut total_counts = HashMap::new();
    let mut preset_spawn_counts = HashMap::new();
    let mut utility_spawn_counts = HashMap::new();
    let mut decay_spawn_counts = HashMap::new();
    let mut purity_spawn_counts = HashMap::new();

    for (conf, res) in &results {
        let spawn_name = res.closest_spawn.name.to_string();
        *total_counts.entry(spawn_name.clone()).or_insert(0) += 1;

        let preset_name = match conf.preset_idx {
            0 => "Phase 1: Early Game",
            1 => "Phase 2: Steel & Coal",
            2 => "Phase 3: Oil & Quartz",
            3 => "Phase 4: Late Game",
            4 => "Phase 5: Quantum",
            5 => "Collectible Hunting",
            _ => "Unknown",
        }
        .to_string();
        *preset_spawn_counts
            .entry((preset_name, spawn_name.clone()))
            .or_insert(0) += 1;

        let utility_str = match conf.utility {
            models::UtilityFunction::CobbDouglas => "Cobb-Douglas",
            models::UtilityFunction::Leontief => "Leontief",
            models::UtilityFunction::Linear => "Linear",
        }
        .to_string();
        *utility_spawn_counts
            .entry((utility_str, spawn_name.clone()))
            .or_insert(0) += 1;

        let decay_str = match conf.decay {
            models::DistanceDecay::Gaussian => "Gaussian",
            models::DistanceDecay::Exponential => "Exponential",
            models::DistanceDecay::PowerLaw => "Power-Law",
            models::DistanceDecay::Linear => "Linear",
            models::DistanceDecay::LogisticStep => "Logistic-Step",
        }
        .to_string();
        *decay_spawn_counts
            .entry((decay_str, spawn_name.clone()))
            .or_insert(0) += 1;

        let purity_str = match conf.purity {
            PurityOverride::Default => "Default",
            PurityOverride::Impure => "Impure",
            PurityOverride::Normal => "Normal",
            PurityOverride::Pure => "Pure",
        }
        .to_string();
        *purity_spawn_counts
            .entry((purity_str, spawn_name.clone()))
            .or_insert(0) += 1;
    }

    // Write markdown report
    let report_path = "simulation_report.md";
    let mut rfile = File::create(report_path).expect("Failed to create report file");

    writeln!(rfile, "# FICSIT Start Optimizer Simulation Matrix Report").unwrap();
    writeln!(rfile, "\nThis report presents the analysis of running **{} optimization simulations** across every combination of presets, purity overrides (excluding Impure), utility functions, and distance decays at a fixed radius of **700 meters** using the **Hybrid** search strategy.", results.len()).unwrap();

    writeln!(rfile, "\n## 1. Global Start Location Frequencies").unwrap();
    writeln!(rfile, "Across all {} runs, the following shows how often each starting zone was selected as the mathematically optimal starting location:", results.len()).unwrap();
    writeln!(rfile, "\n| Starting Zone | Occurrences | Percentage |").unwrap();
    writeln!(rfile, "|---|---|---|").unwrap();
    let mut sorted_totals: Vec<(String, i32)> = total_counts.clone().into_iter().collect();
    sorted_totals.sort_by(|a, b| b.1.cmp(&a.1));
    let total_runs_f = results.len() as f64;
    for (name, count) in &sorted_totals {
        let pct = (*count as f64 / total_runs_f) * 100.0;
        writeln!(rfile, "| **{}** | {} | {:.2}% |", name, count, pct).unwrap();
    }

    writeln!(
        rfile,
        "\n## 2. Recommendation Frequencies by Game Phase Preset"
    )
    .unwrap();
    writeln!(rfile, "This section breaks down starting location preferences by each gameplay phase preset. This reveals which zones are optimal for early game vs. late/quantum end-game:").unwrap();
    writeln!(
        rfile,
        "\n| Preset | Northern Forest | Dune Desert | Rocky Desert | Grass Fields |"
    )
    .unwrap();
    writeln!(rfile, "|---|---|---|---|---|").unwrap();
    let preset_list = vec![
        "Phase 1: Early Game",
        "Phase 2: Steel & Coal",
        "Phase 3: Oil & Quartz",
        "Phase 4: Late Game",
        "Phase 5: Quantum",
        "Collectible Hunting",
    ];
    let preset_denom = total_runs_f / 6.0;
    for preset in &preset_list {
        let nf = *preset_spawn_counts
            .get(&(preset.to_string(), "Northern Forest".to_string()))
            .unwrap_or(&0);
        let dd = *preset_spawn_counts
            .get(&(preset.to_string(), "Dune Desert".to_string()))
            .unwrap_or(&0);
        let rd = *preset_spawn_counts
            .get(&(preset.to_string(), "Rocky Desert".to_string()))
            .unwrap_or(&0);
        let gf = *preset_spawn_counts
            .get(&(preset.to_string(), "Grass Fields".to_string()))
            .unwrap_or(&0);
        writeln!(
            rfile,
            "| {} | {} ({:.1}%) | {} ({:.1}%) | {} ({:.1}%) | {} ({:.1}%) |",
            preset,
            nf,
            (nf as f64 / preset_denom) * 100.0,
            dd,
            (dd as f64 / preset_denom) * 100.0,
            rd,
            (rd as f64 / preset_denom) * 100.0,
            gf,
            (gf as f64 / preset_denom) * 100.0,
        )
        .unwrap();
    }

    writeln!(rfile, "\n## 3. Influence of the Utility Function").unwrap();
    writeln!(rfile, "How the math combines resource values dramatically impacts the recommended start zone. Cobb-Douglas enforces balance, Leontief maximizes bottlenecks, and Linear Additive looks purely at volume:").unwrap();
    writeln!(
        rfile,
        "\n| Utility Function | Northern Forest | Dune Desert | Rocky Desert | Grass Fields |"
    )
    .unwrap();
    writeln!(rfile, "|---|---|---|---|---|").unwrap();
    let utility_list = vec!["Cobb-Douglas", "Leontief", "Linear"];
    let utility_denom = total_runs_f / 3.0;
    for utility in &utility_list {
        let nf = *utility_spawn_counts
            .get(&(utility.to_string(), "Northern Forest".to_string()))
            .unwrap_or(&0);
        let dd = *utility_spawn_counts
            .get(&(utility.to_string(), "Dune Desert".to_string()))
            .unwrap_or(&0);
        let rd = *utility_spawn_counts
            .get(&(utility.to_string(), "Rocky Desert".to_string()))
            .unwrap_or(&0);
        let gf = *utility_spawn_counts
            .get(&(utility.to_string(), "Grass Fields".to_string()))
            .unwrap_or(&0);
        writeln!(
            rfile,
            "| {} | {} ({:.1}%) | {} ({:.1}%) | {} ({:.1}%) | {} ({:.1}%) |",
            utility,
            nf,
            (nf as f64 / utility_denom) * 100.0,
            dd,
            (dd as f64 / utility_denom) * 100.0,
            rd,
            (rd as f64 / utility_denom) * 100.0,
            gf,
            (gf as f64 / utility_denom) * 100.0,
        )
        .unwrap();
    }

    writeln!(rfile, "\n## 4. Influence of Distance Decay").unwrap();
    writeln!(rfile, "Distance decay determines how heavily nodes are penalized as you walk away. Gaussian is smooth, Exponential decay is linear with respect to log distance, Power-Law has a heavy tail (looks further out), and Linear has a hard cutoff:").unwrap();
    writeln!(
        rfile,
        "\n| Distance Decay | Northern Forest | Dune Desert | Rocky Desert | Grass Fields |"
    )
    .unwrap();
    writeln!(rfile, "|---|---|---|---|---|").unwrap();
    let decay_list = vec![
        "Gaussian",
        "Exponential",
        "Power-Law",
        "Linear",
        "Logistic-Step",
    ];
    let decay_denom = total_runs_f / 5.0;
    for decay in &decay_list {
        let nf = *decay_spawn_counts
            .get(&(decay.to_string(), "Northern Forest".to_string()))
            .unwrap_or(&0);
        let dd = *decay_spawn_counts
            .get(&(decay.to_string(), "Dune Desert".to_string()))
            .unwrap_or(&0);
        let rd = *decay_spawn_counts
            .get(&(decay.to_string(), "Rocky Desert".to_string()))
            .unwrap_or(&0);
        let gf = *decay_spawn_counts
            .get(&(decay.to_string(), "Grass Fields".to_string()))
            .unwrap_or(&0);
        writeln!(
            rfile,
            "| {} | {} ({:.1}%) | {} ({:.1}%) | {} ({:.1}%) | {} ({:.1}%) |",
            decay,
            nf,
            (nf as f64 / decay_denom) * 100.0,
            dd,
            (dd as f64 / decay_denom) * 100.0,
            rd,
            (rd as f64 / decay_denom) * 100.0,
            gf,
            (gf as f64 / decay_denom) * 100.0,
        )
        .unwrap();
    }

    writeln!(rfile, "\n## 5. Influence of Purity Override Settings").unwrap();
    writeln!(rfile, "Purity overrides alter the multiplier applied to database resource nodes. Excluding Impure nodes, this section shows recommendations under Default (database-purity), Normal (all normal 1x), and Pure (all pure 2x) override settings:").unwrap();
    writeln!(
        rfile,
        "\n| Purity Override | Northern Forest | Dune Desert | Rocky Desert | Grass Fields |"
    )
    .unwrap();
    writeln!(rfile, "|---|---|---|---|---|").unwrap();
    let purity_list = vec!["Default", "Normal", "Pure"];
    let purity_denom = total_runs_f / 3.0;
    for purity in &purity_list {
        let nf = *purity_spawn_counts
            .get(&(purity.to_string(), "Northern Forest".to_string()))
            .unwrap_or(&0);
        let dd = *purity_spawn_counts
            .get(&(purity.to_string(), "Dune Desert".to_string()))
            .unwrap_or(&0);
        let rd = *purity_spawn_counts
            .get(&(purity.to_string(), "Rocky Desert".to_string()))
            .unwrap_or(&0);
        let gf = *purity_spawn_counts
            .get(&(purity.to_string(), "Grass Fields".to_string()))
            .unwrap_or(&0);
        writeln!(
            rfile,
            "| {} | {} ({:.1}%) | {} ({:.1}%) | {} ({:.1}%) | {} ({:.1}%) |",
            purity,
            nf,
            (nf as f64 / purity_denom) * 100.0,
            dd,
            (dd as f64 / purity_denom) * 100.0,
            rd,
            (rd as f64 / purity_denom) * 100.0,
            gf,
            (gf as f64 / purity_denom) * 100.0,
        )
        .unwrap();
    }

    writeln!(rfile, "\n## 6. Key Analysis & Takeaways").unwrap();
    writeln!(rfile, "\n### A. The Northern Forest Dominance Bias").unwrap();
    writeln!(rfile, "The **Northern Forest** remains the most dominant recommendation across the entire matrix (occurring in **{:.2}%** of all configurations). This is due to its extremely high density of high-purity nodes clustered close to each other. Even with large radius settings or heavy distance penalties, the concentration of Pure Iron, Copper, Limestone, and Coal nodes makes it mathematically superior for almost all early-to-mid-game phases.", (*total_counts.get("Northern Forest").unwrap_or(&0) as f64 / total_runs_f) * 100.0).unwrap();
    writeln!(rfile, "\n### B. When Dune Desert Emerges").unwrap();
    writeln!(rfile, "The **Dune Desert** becomes highly optimal in **Phase 4 (Late Game)** and **Phase 5 (Quantum)**. In these phases, the weight of rare resources (like Bauxite, Sulfur, and SAM) increases. The Dune Desert contains vast quantities of these resources plus ample space, and as the walking radius (sigma) increases to 800m+, the optimizer shifts toward the Dune Desert to capture these nodes concurrently.").unwrap();
    writeln!(rfile, "\n### C. Utility Function Impact").unwrap();
    writeln!(rfile, "- **Cobb-Douglas** enforces balanced resource access. If a resource is missing, the score is highly penalized. As a result, it heavily favors areas with diverse node types (like the boundary between Rocky Desert and Northern Forest).").unwrap();
    writeln!(rfile, "- **Leontief** focuses strictly on the bottleneck. It is highly sensitive to the presence of all required resources, meaning it favors safe zones like Rocky Desert and Northern Forest, while giving Grass Fields a very low score if water or coal is missing.").unwrap();
    writeln!(rfile, "- **Linear Additive** values pure quantity. Because of this, it strongly favors the high-density Northern Forest and Dune Desert zones, completely ignoring whether you have a balanced setup or just an abundance of one node type.").unwrap();
    writeln!(rfile, "\n### D. Distance Decay Behavior").unwrap();
    writeln!(rfile, "- **Gaussian** and **Linear** decay act as hard cutoffs, locking recommendations to dense clusters (Northern Forest).").unwrap();
    writeln!(rfile, "- **Power-Law** (heavy tail) allows the optimizer to 'see' distant resources. This pulls recommended start locations towards boundary zones between biomes (e.g. the forest-desert-canyon meeting points) because it rewards having access to multiple distinct clusters even if some are far away.").unwrap();
    writeln!(rfile, "\n### E. Logistical Radius").unwrap();
    writeln!(rfile, "The logistical walking radius for this simulation matrix was held constant at the new default of **700 meters**.").unwrap();

    writeln!(rfile, "\n## 7. Raw Results Dataset").unwrap();
    writeln!(rfile, "The complete raw dataset of all {} runs has been saved to the workspace as `simulation_results.csv`.", results.len()).unwrap();

    println!("Saved detailed analysis report to {}", report_path);
}
