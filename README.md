# NEON CITY // 2161

A self-running sci-fi city simulation written entirely in Rust and compiled to
WebAssembly. AI citizens manage hunger, energy, hygiene, fun and money in a
procedurally generated neon city with jobs, food production, and a day/night
cycle.

## Run it

**Native (development):**

    cargo run --release

**Website (WASM):**

    ./build_web.sh
    python3 -m http.server 8080 -d web
    # open http://localhost:8080

## Controls

- **Drag** to pan, **scroll** to zoom
- **Click** a citizen or building to inspect it; FOLLOW tracks a citizen
- **Space** pause · **1/2/3** = 1×/4×/16× speed

## How it works

- `src/sim/` — pure deterministic simulation (no rendering imports):
  procedural city, A* pathfinding, utility-based AI (needs scream louder as
  they empty; personality archetypes re-weight them), wages, a hydroponic
  food-production chain.
- `src/render/` — procedural neon visuals; swap this layer for sprites
  without touching the sim.
- `src/ui/` — camera, HUD, inspector.

One game day is 4 real minutes at 1× speed. The simulation is fully
deterministic for a given seed.

Tests: `cargo test` (31 tests over the sim core, including an end-to-end
day-cycle behavior test).
