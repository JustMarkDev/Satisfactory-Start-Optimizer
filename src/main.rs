mod models;
mod data_loader;
mod optimizer;

use models::{OptimizerConfig, GamePhase, PurityOverride};
use std::env;
use std::collections::HashMap;

use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
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
    selected_option: usize, // 0 = Preset, 1 = Purity, 2 = Sigma, 3..23 = weights, 24 = Run button
    checklist_scroll_top: usize,
    opt_result: Option<optimizer::OptimizationResult>,
    status_msg: String,
}

fn apply_preset_weights(preset_idx: usize, weights: &mut HashMap<String, f64>) {
    weights.clear();
    match preset_idx {
        0 => {
            weights.insert("iron".to_string(), 1.0);
            weights.insert("copper".to_string(), 0.8);
            weights.insert("limestone".to_string(), 0.7);
            weights.insert("caterium".to_string(), 0.1);
            weights.insert("uranium".to_string(), -2.0); // Severe penalty
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
            weights.insert("water".to_string(), 0.8);
            weights.insert("caterium".to_string(), 0.4);
            weights.insert("uranium".to_string(), -2.0);
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
            weights.insert("water".to_string(), 0.6);
            weights.insert("oil".to_string(), 1.0);
            weights.insert("sulfur".to_string(), 0.6);
            weights.insert("quartz".to_string(), 0.6);
            weights.insert("caterium".to_string(), 0.6);
            weights.insert("uranium".to_string(), -2.0);
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
            weights.insert("water".to_string(), 0.6);
            weights.insert("oil".to_string(), 0.8);
            weights.insert("sulfur".to_string(), 0.8);
            weights.insert("quartz".to_string(), 0.8);
            weights.insert("caterium".to_string(), 0.8);
            weights.insert("bauxite".to_string(), 1.0);
            weights.insert("nitrogenwell".to_string(), 0.8);
            weights.insert("geyser".to_string(), 0.8);
            weights.insert("uranium".to_string(), 0.5);
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
            weights.insert("uranium".to_string(), 0.8);
            weights.insert("sam".to_string(), 1.0);
            weights.insert("blueslug".to_string(), 0.03);
            weights.insert("yellowslug".to_string(), 0.05);
            weights.insert("purpleslug".to_string(), 0.08);
            weights.insert("mercer".to_string(), 0.05);
            weights.insert("somersloop".to_string(), 0.05);
            weights.insert("harddrive".to_string(), 0.10);
        }
        5 => {
            weights.insert("blueslug".to_string(), 0.5);
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
    result: &Option<optimizer::OptimizationResult>,
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
        ("Grass Fields", -110000.0, 240000.0, Color::Rgb(34, 139, 34), "."),
        ("Rocky Desert", -200000.0, -200000.0, Color::Rgb(210, 180, 140), "-"),
        ("Northern Forest", 0.0, -90000.0, Color::Rgb(46, 139, 87), "*"),
        ("Dune Desert", 240000.0, -210000.0, Color::Rgb(244, 164, 96), "~"),
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
        let c = (((sx - min_x) / (max_x - min_x) * (width - 1) as f64).round() as usize).clamp(0, width - 1);
        let r = (((sy - min_y) / (max_y - min_y) * (height - 1) as f64).round() as usize).clamp(0, height - 1);
        map_chars[r][c] = ("S", Color::Rgb(240, 248, 255));
    }

    // Render optimal crosshair if solved
    if let Some(res) = result {
        let c = (((res.x - min_x) / (max_x - min_x) * (width - 1) as f64).round() as usize).clamp(0, width - 1);
        let r = (((res.y - min_y) / (max_y - min_y) * (height - 1) as f64).round() as usize).clamp(0, height - 1);
        
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

fn run_tui(
    nodes: &[models::ResourceNode],
    file_info: String,
    initial_config: OptimizerConfig,
    initial_preset_idx: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let app_result = run_tui_loop(&mut terminal, nodes, file_info, initial_config, initial_preset_idx);

    // Clean up raw mode alternate screen
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    app_result
}

fn run_tui_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    nodes: &[models::ResourceNode],
    file_info: String,
    initial_config: OptimizerConfig,
    initial_preset_idx: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut state = TuiState {
        sigma: initial_config.sigma,
        preset_idx: initial_preset_idx,
        purity_override: initial_config.purity_override,
        selected_option: 0,
        checklist_scroll_top: 0,
        opt_result: None,
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

    // Run initial optimization so map is immediately populated
    {
        let mut config = OptimizerConfig::default();
        config.sigma = state.sigma;
        config.weights = weights.clone();
        config.purity_override = state.purity_override;
        let start_time = std::time::Instant::now();
        let result = optimizer::optimize(nodes, &config);
        let duration = start_time.elapsed();
        state.opt_result = Some(result);
        state.status_msg = format!("Initial solve in {:?}", duration);
    }

    loop {
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

            let sigma_style = if state.selected_option == 2 {
                Style::default().fg(Color::Rgb(255, 152, 0)).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            left_lines.push(Line::from(vec![
                Span::raw(if state.selected_option == 2 { "> " } else { "  " }),
                Span::styled(format!("Radius: < {} meters >", state.sigma), sigma_style),
            ]));
            left_lines.push(Line::from(""));
            left_lines.push(Line::from("─".repeat(46)));
            left_lines.push(Line::from(" PARAMETER CHECKLIST (Space to Toggle):"));
            left_lines.push(Line::from(""));

            // 2. Checklist Section (Scrollable viewport inside Left Column)
            let height = chunks[0].height as usize;
            let max_visible = (height.saturating_sub(18)).max(1);
            
            let end_idx = (state.checklist_scroll_top + max_visible).min(CONFIGURABLE_RESOURCES.len());
            for idx in state.checklist_scroll_top..end_idx {
                let res = CONFIGURABLE_RESOURCES[idx];
                let val = *weights.get(res).unwrap_or(&0.0);
                
                let is_focused = state.selected_option == 3 + idx;
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
            let run_style = if state.selected_option == 24 {
                Style::default().bg(Color::Rgb(50, 205, 50)).fg(Color::Black).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Rgb(50, 205, 50))
            };
            left_lines.push(Line::from(""));
            left_lines.push(Line::from(vec![
                Span::raw(if state.selected_option == 24 { "> " } else { "  " }),
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
                &state.opt_result,
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
            if let Some(res) = &state.opt_result {
                results_lines.push(Line::from(vec![
                    Span::styled("OPTIMAL STARTING ZONE FOUND:", Style::default().fg(Color::Rgb(50, 205, 50)).add_modifier(Modifier::BOLD)),
                ]));
                results_lines.push(Line::from(vec![
                    Span::raw(format!("  Coordinate: X: {:.2}m | Y: {:.2}m | Z: {:.2}m", res.x / 100.0, res.y / 100.0, res.z / 100.0)),
                ]));
                results_lines.push(Line::from(vec![
                    Span::raw(format!("  Utility Score: {:.6}", res.score)),
                ]));
                results_lines.push(Line::from(vec![
                    Span::raw(format!("  Closest Spawn: {} Biome (Distance: {:.1}m)", res.closest_spawn.name, res.spawn_distance)),
                ]));
                
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
                    flatness_score = (-std_dev_m / 40.0).exp();
                }
                results_lines.push(Line::from(vec![
                    Span::raw(format!("  Terrain Flatness: {:.1}% (Z std dev: {:.2}m)", flatness_score * 100.0, std_dev_m)),
                ]));
            } else {
                results_lines.push(Line::from("No optimization solved yet. Press ENTER on [ RUN ] to execute."));
            }

            results_lines.push(Line::from(""));
            results_lines.push(Line::from(vec![
                Span::styled("Controls: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw("Up/Down to navigate | Left/Right to adjust values | Space to toggle weights | Enter to RUN | Q/Esc to quit"),
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
                
                // Get dynamic layout sizing to handle scrolling limits in navigation keys
                let size = terminal.size()?;
                let layout_chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Length(48),
                        Constraint::Min(20),
                    ])
                    .split(size);
                
                match key.code {
                    KeyCode::Up => {
                        if state.selected_option > 0 {
                            state.selected_option -= 1;
                            
                            // Adjust scroll top if focused on checklist
                            if state.selected_option >= 3 && state.selected_option <= 23 {
                                let item_idx = state.selected_option - 3;
                                if item_idx < state.checklist_scroll_top {
                                    state.checklist_scroll_top = item_idx;
                                }
                            }
                        }
                    }
                    KeyCode::Down => {
                        if state.selected_option < 24 {
                            state.selected_option += 1;
                            
                            // Adjust scroll top if focused on checklist
                            if state.selected_option >= 3 && state.selected_option <= 23 {
                                let item_idx = state.selected_option - 3;
                                let max_visible = (layout_chunks[0].height as usize).saturating_sub(18).max(1);
                                if max_visible > 0 && item_idx >= state.checklist_scroll_top + max_visible {
                                    state.checklist_scroll_top = item_idx + 1 - max_visible;
                                }
                            }
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
                            let mut curr_idx = modes.iter().position(|&m| m == state.purity_override).unwrap_or(0);
                            if curr_idx > 0 {
                                curr_idx -= 1;
                            } else {
                                curr_idx = 3;
                            }
                            state.purity_override = modes[curr_idx];
                        } else if state.selected_option == 2 {
                            if state.sigma > 150.0 {
                                state.sigma -= 50.0;
                            }
                        } else if state.selected_option >= 3 && state.selected_option <= 23 {
                            let res_name = CONFIGURABLE_RESOURCES[state.selected_option - 3];
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
                            let mut curr_idx = modes.iter().position(|&m| m == state.purity_override).unwrap_or(0);
                            if curr_idx < 3 {
                                curr_idx += 1;
                            } else {
                                curr_idx = 0;
                            }
                            state.purity_override = modes[curr_idx];
                        } else if state.selected_option == 2 {
                            if state.sigma < 1500.0 {
                                state.sigma += 50.0;
                            }
                        } else if state.selected_option >= 3 && state.selected_option <= 23 {
                            let res_name = CONFIGURABLE_RESOURCES[state.selected_option - 3];
                            let val = weights.entry(res_name.to_string()).or_insert(0.0);
                            *val = (*val + 0.1).clamp(-10.0, 10.0);
                            *val = (*val * 10.0).round() / 10.0;
                            if *val != 0.0 {
                                last_nonzero_weights.insert(res_name.to_string(), *val);
                            }
                        }
                    }
                    KeyCode::Char(' ') => {
                        if state.selected_option >= 3 && state.selected_option <= 23 {
                            let res_name = CONFIGURABLE_RESOURCES[state.selected_option - 3];
                            let val = weights.entry(res_name.to_string()).or_insert(0.0);
                            if *val == 0.0 {
                                let restored = last_nonzero_weights.get(res_name).copied().unwrap_or_else(|| default_nonzero_weight(res_name));
                                *val = restored;
                            } else {
                                last_nonzero_weights.insert(res_name.to_string(), *val);
                                *val = 0.0;
                            }
                        }
                    }
                    KeyCode::Enter => {
                        if state.selected_option == 24 {
                            state.status_msg = "Running mathematical solver...".to_string();
                            
                            // Re-draw once to update the status message
                            terminal.draw(|f| {
                                let size = f.size();
                                let inner_chunks = Layout::default()
                                    .direction(Direction::Horizontal)
                                    .constraints([
                                        Constraint::Length(48),
                                        Constraint::Min(20),
                                    ])
                                    .split(size);
                                let label = Span::styled("Solving...", Style::default().fg(Color::Yellow));
                                let temp_para = Paragraph::new(vec![Line::from(label)]);
                                f.render_widget(temp_para, inner_chunks[1]);
                            })?;

                            let mut config = OptimizerConfig::default();
                            config.sigma = state.sigma;
                            config.weights = weights.clone();
                            config.purity_override = state.purity_override;

                            let start_time = std::time::Instant::now();
                            let result = optimizer::optimize(nodes, &config);
                            let duration = start_time.elapsed();

                            state.opt_result = Some(result);
                            state.status_msg = format!("Solved in {:?}", duration);
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
  --sigma <meters>     Logistical walking radius in meters (default: 600)
  --tier <1-5|early|steel|oil|late|quantum>
                       Select game phase preset for non-interactive mode.
  --purity <default|impure|normal|pure>
                       Override database node purity multipliers (default: default)
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
    let mut active_phase: Option<GamePhase> = None;
    let mut is_collectibles_mode = false;
    let mut output_json = false;

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
                        config.sigma = val;
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
            "--tier" | "--phase" => {
                if i + 1 < args.len() {
                    let phase_str = &args[i + 1];
                    if let Some(phase) = GamePhase::from_str(phase_str) {
                        phase.apply_weights(&mut config.weights);
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
