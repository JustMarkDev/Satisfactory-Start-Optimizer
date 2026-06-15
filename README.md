# Satisfactory Start Optimizer (Rust Version)

A high-performance command-line utility built in Rust to calculate the mathematically optimal starting coordinates on the Satisfactory game map. It allows you to configure your logistical walking radius, resource priorities, and customize both the utility scoring function and distance decay models to suit your preferred gameplay style.

---

## Technical Features

1. **Customizable Distance Decay**: Models player traversal logistics. The value of a node decreases as distance grows, using Gaussian, Exponential, Power-Law (Gravity), or Linear decay models.
2. **Flexible Utility Scoring**: Combines yields of multiple resource types using Cobb-Douglas (Multiplicative Balance), Leontief (Min-Bottleneck), or Linear Additive models.
3. **Dynamic Resource Support**: The CLI and API can accept weights for resource IDs supported by the loaded map data. The web dashboard exposes a fixed set of known resources, collectibles, and threats through sliders.
4. **Parallelized Search Engine**: Uses `rayon` to perform a global 2D grid search across the entire map bounds parallelized over your CPU threads, followed by a local gradient ascent (hill climbing) refinement down to centimeter precision.

---

## Mathematical Formulation & Options

### 1. Utility Functions

The utility function defines how the yields of multiple resource types within walking range are combined into a single "goodness score" for a candidate starting location.

#### A. Cobb-Douglas (Multiplicative Balance)

- **Formula**:
  $$U = \prod_{r \in R} (Y_r + \epsilon)^{w_r}$$
  Where $Y_r$ is the distance-weighted yield of resource $r$, $w_r$ is the weight of resource $r$, and $\epsilon$ is a small smoothing constant ($10^{-5}$) to prevent the utility from dropping to zero when a non-critical resource is completely absent.
- **Gameplay Behavior**: Prioritizes balanced diversity. The score is severely penalized if any required resource is completely missing.
- **Strengths**:
  - **Balanced Resource Diversity**: Guarantees finding starting locations that have a healthy mix of all requested resources.
  - **Accurate for Early Game**: Prevents starting in locations that are completely missing a critical progression resource (e.g., copper or limestone).
- **Weaknesses**:
  - **Smoothing Dependency**: Relies on a smoothing factor $\epsilon$ to prevent zero-utility lockouts.
  - **Strict Penalty**: A location with a massive quantity of one resource but slightly missing another will be heavily penalized.

#### B. Leontief (Minimum Bottleneck)

- **Formula**:
  $$U = \min_{r \in R} \left( \frac{Y_r}{w_r} \right)$$
  Where we assume $w_r > 0$.
- **Gameplay Behavior**: Models strict crafting dependencies. Your utility is bound by your most scarce resource (the bottleneck).
- **Strengths**:
  - **Bottleneck Elimination**: Prioritizes the resource in shortest supply, ensuring you always have enough of your limiting reactant.
  - **Perfect for Ratios**: Directly maps to strict crafting recipe requirements (e.g. Iron Plates need exactly 3 Iron Ore to 1 Limestone for concrete, etc.).
- **Weaknesses**:
  - **Zero-Score Lockout**: If any weighted resource has exactly zero yield within range, the entire score is 0.
  - **Abundance Neglect**: Does not reward surplus resources. Having 100 Iron and 1 Copper yields the same utility as 1 Iron and 1 Copper.

#### C. Linear Additive (Pure Volume)

- **Formula**:
  $$U = \sum_{r \in R} w_r \cdot Y_r$$
- **Gameplay Behavior**: Pure volume calculation. Resources are perfect substitutes.
- **Strengths**:
  - **Intuitive & Simple**: Larger numbers of nodes always mean a higher score.
  - **No Lockouts**: Missing resources do not penalize the existing resources (yields of 0 simply add 0 to the sum).
- **Weaknesses**:
  - **Extreme Substitution**: A location with a massive cluster of Iron but zero Copper or Limestone can score higher than a balanced spot, making it potentially unplayable for a starting base.

---

### 2. Distance Decay Functions

The decay function models the penalty associated with the distance ($d$) from the base to a resource node, normalized by the logistical walking radius ($\sigma$).

#### A. Gaussian Decay (Smooth Drop-Off)

- **Formula**:
  $$f(d) = e^{-\frac{1}{2}\left(\frac{d}{\sigma}\right)^2}$$
- **Gameplay Behavior**: Nodes very close to the center are valued at nearly 100%, followed by a smooth transition, and a rapid drop-off as distance approaches the walking radius limit.
- **Strengths**:
  - **Realistic Travel Modeling**: Matches human intuition—walking 100m vs 200m feels similar, but walking 600m is exponentially more tedious.
  - **Excellent Convergence**: Smooth derivatives help optimization algorithms find the exact peak.
- **Weaknesses**:
  - **Hard Boundary Penalty**: Value drops to near-zero extremely fast as you approach $\sigma$.

#### B. Exponential Decay (Linear Transportation Cost)

- **Formula**:
  $$f(d) = e^{-\frac{d}{\sigma}}$$
- **Gameplay Behavior**: Constant rate of decay per unit of distance, representing a linear increase in travel time or conveyor belt cost.
- **Strengths**:
  - **Linear Cost Analogy**: Represents conveyor belt resource routing cost realistically.
  - **Forgiving Mid-Range**: Values mid-range nodes higher than Gaussian decay.
- **Weaknesses**:
  - **Steep Near-Center Drop**: Drops off quickly at very short distances compared to Gaussian.

#### C. Power-Law / Gravity Model (Heavy Tail)

- **Formula**:
  $$f(d) = \frac{1}{\frac{d}{\sigma} + 1}$$
- **Gameplay Behavior**: Slow decay with a very long tail. Nodes far away still exert a small "gravitational pull" on the start location.
- **Strengths**:
  - **Central Hub Identification**: Great for locating a central factory hub situated between several distinct resource clusters.
  - **Long-Range Awareness**: High-value distant nodes (e.g., Oil, Caterium) are still factored into the start location.
- **Weaknesses**:
  - **Spread-Out Recommendations**: May recommend a spot where everything is slightly too far to comfortably walk in the early game.

#### D. Linear / Threshold Decay (Hard Cutoff)

- **Formula**:
  $$f(d) = \max\left(0, 1 - \frac{d}{\sigma}\right)$$
- **Gameplay Behavior**: The value of the node drops linearly to zero at $d = \sigma$. Any node beyond $\sigma$ is completely ignored.
- **Strengths**:
  - **Strict Boundaries**: Simple and intuitive hard cutoff.
- **Weaknesses**:
  - **Discontinuous Derivative**: Optimization algorithms (like gradient ascent) can get stuck or exhibit jumpy behavior at the $d = \sigma$ boundary.
  - **No Long-Distance Distinction**: A node at $1.1\sigma$ has the exact same value (0) as a node at $5\sigma$.

---

## How to Build and Run

### Requirements

- [Rust toolchain (Cargo/rustc)](https://www.rust-lang.org/tools/install)
- [Bun](https://bun.sh/) (for the Web Dashboard)

### 1. Launching the Web Dashboard

The web dashboard provides an interactive satellite/in-game map overlay with real-time UI controls to adjust your priorities, Sigma radius, utility models, and visualize optimal starting coordinates instantly.

1. **Install frontend dependencies**:

   ```bash
   bun install
   ```

2. **Start backend and frontend with one command**:

   ```bash
   bun run dev
   ```

   This is the recommended startup path. It starts the Rust API server on `http://127.0.0.1:8080` and the Vite+ web UI on `http://127.0.0.1:3000` (with automatic browser opening). The UI proxies all API calls (`/api/*`) to the Rust backend.

3. **Optional commands**:

   ```bash
   bun run server         # backend only, debug/dev mode on http://127.0.0.1:8080
   bun run server:release # backend only, optimized release mode
   bun run ui             # frontend only; assumes the backend is already running on 127.0.0.1:8080
   ```

4. **Optional Vite+ maintenance commands**:
   ```bash
   bun run build
   bun run check
   bun run check:fix
   bun run pack
   ```

---

### 2. Running in Scriptable JSON Mode (`--json`)

If you want to integrate the optimizer with automated scripting pipelines or print raw serialized outputs, use the `--json` flag:

- Quietly solve using specific utility and distance decay strategies:

  ```bash
  cargo run --release -- --utility leontief --decay exponential --tier 2 --json
  ```

- Quietly solve for Phase 1 Preset and output raw JSON:

  ```bash
  cargo run --release -- --tier 1 --json
  ```

- Quietly solve with purity overrides (e.g. treating all nodes as impure):

  ```bash
  cargo run --release -- --purity impure --json
  ```

- Quietly solve with custom dynamic weights:

  ```bash
  cargo run --release -- --iron 1.5 --uranium -5.0 --json
  ```

- View the full help menu:
  ```bash
  cargo run --release -- --help
  ```
