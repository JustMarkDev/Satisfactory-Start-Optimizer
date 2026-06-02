# Satisfactory Start Optimizer (Rust Version)

A high-performance command-line utility built in Rust to calculate the mathematically optimal starting coordinates on the Satisfactory game map. It uses a **multi-resource Cobb-Douglas utility function** and **Gaussian distance decay** to find starting zones that offer the best balance of nearby resources.

---

## Technical Features

1. **Gaussian Distance Decay**: Models player traversal logistics. The value of a node decreases exponentially as distance grows, scaled by the customizable parameter `sigma` ($\sigma$).
2. **Dynamic Cobb-Douglas Resource Balance**: Ensures the optimizer solves for a starting location with a healthy diversity of all specified resources rather than biasing towards one single type.
3. **Dynamic Resource Support**: Supports *any* resource type (Iron, Copper, Limestone, Coal, Caterium, Oil, Bauxite, Sulfur, SAM, etc.) dynamically. You can add weights for any resource type in the game from the CLI.
4. **Parallelized Search Engine**: Uses `rayon` to perform a global 2D grid search across the entire map bounds parallelized over your CPU threads, followed by a local gradient ascent (hill climbing) refinement down to centimeter precision.

---

## How to Build and Run

### Requirements
- [Rust toolchain (Cargo/rustc)](https://www.rust-lang.org/tools/install)

### Launching the Interactive TUI Dashboard
Running the application without the `--json` flag (with or without other parameters) enters a full-screen interactive terminal dashboard:
```bash
cargo run --release
```

**TUI Controls:**
*   **Up/Down Arrows**: Navigate between Preset, Purity, Radius, the configurable weights checklist, and the RUN button.
*   **Left/Right Arrows**: Cycle presets, cycle purity modes (Default, Impure, Normal, Pure), adjust Radius, or scale the dynamic weights of active resource parameters in steps of $\pm 0.1$.
*   **Space**: Toggle the checkbox next to resource parameters to enable or disable them in the utility calculation.
*   **Enter**: Execute the optimization solver when focused on `[ RUN OPTIMIZATION ENGINE ]`.
*   **Q or Esc**: Exit the alternate screen cleanly and restore the terminal configuration.

### Running in Scriptable JSON Mode (`--json`)
If you want to integrate the optimizer with automated scripting pipelines or print raw serialized outputs, use the `--json` flag:

*   Quietly solve for Phase 1 Preset and output raw JSON:
    ```bash
    cargo run --release -- --tier 1 --json
    ```

*   Quietly solve with purity overrides (e.g. treating all nodes as impure):
    ```bash
    cargo run --release -- --purity impure --json
    ```

*   Quietly solve with custom walk radius and dynamic weights:
    ```bash
    cargo run --release -- --sigma 800 --iron 1.5 --uranium -5.0 --json
    ```

*   View the full help menu:
    ```bash
    cargo run --release -- --help
    ```

---

## Loading the Complete Interactive Map Data (SCIM Extract)

If you want to run the optimizer on the **complete, 100% updated Satisfactory 1.0 game map data** from [Satisfactory Calculator (SCIM)](https://satisfactory-calculator.com/en/interactive-map):

### Step 1: Download the SCIM Data
1. Open your browser and go to the [SCIM Interactive Map](https://satisfactory-calculator.com/en/interactive-map).
2. Open **Developer Tools** (F12 or `Ctrl+Shift+I` / `Cmd+Option+I`).
3. Go to the **Network** tab and select **Fetch/XHR**.
4. In the filter box, type `mongo` or `nodes`.
5. Reload the map page (or toggle the resource filters on the sidebar).
6. Look for a network request similar to `https://satisfactory-calculator.com/en/interactive-map/index/mongo` or CDN endpoints loading markers.
7. Right-click the request and select **Copy** -> **Copy Response**.
8. Paste the copied response into a text file at `data/raw_scim_nodes.json`.

### Step 2: Format the Data for the Optimizer
We provided a Python helper utility to convert the raw Leaflet markers format from SCIM into a flat JSON format ready for Rust.

Run the script:
```bash
python scratch/scim_parser.py data/raw_scim_nodes.json data/scim_nodes.json
```

### Step 3: Run the Optimizer on the Complete Map
Run the Rust engine on the full formatted database:
```bash
cargo run --release -- --file data/scim_nodes.json
```
This allows you to calculate the absolute optimal global coordinate across the entire updated game map based on whatever resource combinations you prioritize!
