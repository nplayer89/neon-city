# Neon City — AI Citizen Simulation (Design Spec)

**Date:** 2026-06-10
**Status:** Approved by user (verbal, in-session)

## Summary

A self-running sci-fi city simulation, written entirely in Rust, compiled to
WebAssembly and served as a static website. AI-driven citizens move through a
procedurally generated city managing their needs (hunger, energy, hygiene, fun)
and money. The city contains jobs, homes, food venues, and leisure venues, with
a simple closed economy. The user observes: pan/zoom camera, click citizens and
buildings to inspect them, control simulation speed.

## Decisions (locked with user)

| Decision | Choice |
|---|---|
| Platform | Browser via WASM (also runs natively for dev) |
| Language | 100% Rust |
| Perspective | Top-down 2D |
| Art | Procedural vector graphics v1; renderer isolated so sprite assets can be added later |
| Theme | Semi sci-fi (near-future tech: fusion, hydroponics, holo-parks) |
| Interaction | Observe + inspect (no god mode in v1) |
| Engine | Macroquad + custom pure-Rust sim core |

## Architecture

Three layers, one crate:

```
src/
  sim/      Pure simulation logic. No macroquad imports. Deterministic
            (seeded RNG, fixed timestep). Unit-tested.
    city.rs       Grid, roads, lots, building placement (procgen)
    citizen.rs    Needs, wallet, personality, job, state machine
    ai.rs         Utility scoring, action selection
    path.rs       A* over the road network
    economy.rs    Wages, prices, food production/stock
    world.rs      Top-level World: tick(), spawn, queries
  render/   Reads &World, draws it. All macroquad drawing lives here.
  ui/       Inspector panels, time controls, camera (pan/zoom).
  main.rs   Game loop: fixed-timestep sim ticks + interpolated render.
```

**Isolation contract:** `sim` exposes plain data (positions, need values,
current action, path) and never references rendering. `render` may be replaced
wholesale (e.g. sprite atlas) without touching `sim`.

## Simulation design

### Needs

Each citizen has four needs in `0.0..=1.0` (1.0 = fully satisfied), decaying at
per-citizen rates, plus money (credits):

- **Hunger** — restored by eating at food venues (costs credits).
- **Energy** — restored by sleeping at home (free, slow).
- **Hygiene** — restored by showering at home (free, fast).
- **Fun** — restored at leisure venues (costs credits).

### AI: utility-based action selection

Every building advertises actions: `(need_restored, rate, price, capacity)`.
When idle, a citizen scores every available action:

```
score = urgency(need) * personality_weight * affordability - travel_cost
```

- `urgency` is non-linear (low needs dominate).
- `personality_weight` per citizen (e.g. slob: low hygiene weight) creates
  visible behavioral variety.
- Work is scheduled, not scored: during shift hours, employed citizens commute
  to work unless a need is critical.
- Chosen action → A* path → walk → perform (need refills over time) → re-decide.

### Economy

- Citizens hold jobs at workplaces (fusion plant, hydroponics farm, robotics
  fab, data center). Wages paid per completed shift hour.
- Food venues hold stock; the hydroponics farm produces stock on a timer and
  distributes it. No stock → can't eat there (drives visible scarcity dynamics).
- Prices: meals and fun cost credits. Money is a constraint, not a need bar.

### City generation

Seeded procgen on a grid (~48×48 tiles): road network first (main avenues +
side streets), then lots filled by zone: residential blocks, commercial strip,
industrial/work district, leisure scattered. Every building snaps to a road
with a door tile.

### Time

Fixed timestep (60 ticks/s). 1 in-game day ≈ 4 real minutes at 1× speed.
Day/night cycle drives work shifts, sleep pressure, and lighting.

## Rendering & look

- Dark asphalt base, neon accent palette (cyan/magenta/amber per district).
- Day/night ambient color cycle; building windows glow at night.
- Buildings: flat-roof procedural shapes with neon edge trim, district-colored.
- Citizens: small glowing figures with walk animation and subtle motion trail.
- Roads with lane markings; occasional autonomous vehicles as ambient flavor.

## UI

- Camera: drag to pan, scroll to zoom.
- Click citizen → panel: name, portrait glyph, need bars, wallet, job,
  current action + destination. Panel live-updates; camera-follow toggle.
- Click building → panel: type, occupants, stock/prices, workers.
- Time controls: pause / 1× / 4× / 16×. Clock + day counter HUD.

## Build & delivery

- `cargo run` — native window for development.
- `cargo build --release --target wasm32-unknown-unknown` + static
  `web/index.html` loader = the website. Served with any static file server.
- Tests: `cargo test` over the sim core (needs decay, utility choice,
  pathfinding, economy invariants, determinism).

## Out of scope (v1)

God mode (building placement, citizen spawning), sprite/texture assets, sound,
save/load, multiplayer. Architecture must not block any of these.
