# Neon City Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A self-running sci-fi city sim — AI citizens managing hunger/energy/hygiene/fun and money in a procedural neon city — 100% Rust, compiled to WASM, served as a static website.

**Architecture:** Three layers in one crate: `sim/` (pure deterministic logic, no rendering imports, unit-tested), `render/` (macroquad drawing, reads `&World`), `ui/` (camera, HUD, inspector). Fixed 60 Hz timestep; 1 game day = 4 real minutes at 1×.

**Tech Stack:** Rust 2021, macroquad 0.4 (native dev + wasm32-unknown-unknown), zero other deps (own PCG32 RNG for determinism and WASM-friendliness).

**Spec:** `docs/superpowers/specs/2026-06-10-neon-city-sim-design.md`

**Conventions used throughout:**
- World coordinates are in **tile units** (f32). The grid is 49×49; roads every 6th row/col.
- Time: `TICKS_PER_HOUR = 600`, 24h days. Needs are `0.0..=1.0` (1.0 = satisfied).
- Tests live in `#[cfg(test)] mod tests` inside each sim file. `cargo test` must pass after every task.
- Commit after every task with the message given in the task.

---

### Task 1: Project scaffold

**Files:**
- Create: `Cargo.toml`, `.gitignore`, `src/main.rs`, `src/sim/mod.rs`, `src/render/mod.rs`, `src/ui/mod.rs`

- [ ] **Step 1: Create the crate files**

`Cargo.toml`:
```toml
[package]
name = "neon_city"
version = "0.1.0"
edition = "2021"

[dependencies]
macroquad = "0.4"

[profile.release]
opt-level = "s"
lto = true
```

`.gitignore`:
```
/target
web/neon_city.wasm
```

`src/sim/mod.rs`:
```rust
pub mod rng;
pub mod time;
pub mod city;
pub mod path;
pub mod citizen;
pub mod economy;
pub mod ai;
pub mod world;
```
(Comment out `pub mod` lines for files that don't exist yet; uncomment as tasks add them. For this task, create the file with all lines commented.)

`src/render/mod.rs` and `src/ui/mod.rs`: empty files for now.

`src/main.rs`:
```rust
mod render;
mod sim;
mod ui;

use macroquad::prelude::*;

fn window_conf() -> Conf {
    Conf {
        window_title: "NEON CITY".to_string(),
        window_width: 1360,
        window_height: 860,
        high_dpi: true,
        sample_count: 4,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    loop {
        clear_background(Color::new(0.04, 0.05, 0.09, 1.0));
        draw_text("NEON CITY — booting…", 40.0, 60.0, 32.0, Color::new(0.2, 0.9, 1.0, 1.0));
        next_frame().await
    }
}
```

- [ ] **Step 2: Verify it builds**

Run: `cargo check`
Expected: compiles clean (first run downloads macroquad).

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "feat: scaffold neon_city crate with macroquad"
```

---

### Task 2: Deterministic RNG (`sim/rng.rs`)

**Files:**
- Create: `src/sim/rng.rs` (uncomment `pub mod rng;` in `src/sim/mod.rs`)

- [ ] **Step 1: Write failing tests + implementation skeleton**

`src/sim/rng.rs`:
```rust
/// PCG32 — deterministic across platforms incl. WASM, no external deps.
pub struct Rng {
    state: u64,
    inc: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        let mut r = Rng { state: 0, inc: (seed << 1) | 1 };
        r.state = seed.wrapping_add(0x853c49e6748fea9b);
        r.next_u32();
        r
    }

    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old
            .wrapping_mul(6364136223846793005)
            .wrapping_add(self.inc);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    /// Uniform f32 in [0, 1).
    pub fn gen_f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }

    /// Uniform i32 in [lo, hi). Requires lo < hi.
    pub fn gen_range(&mut self, lo: i32, hi: i32) -> i32 {
        lo + (self.next_u32() % (hi - lo) as u32) as i32
    }

    pub fn gen_f32_range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + self.gen_f32() * (hi - lo)
    }

    pub fn chance(&mut self, p: f32) -> bool {
        self.gen_f32() < p
    }

    /// Fisher–Yates shuffle.
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.gen_range(0, i as i32 + 1) as usize;
            items.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..1000 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }

    #[test]
    fn different_seeds_differ() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(2);
        let same = (0..100).filter(|_| a.next_u32() == b.next_u32()).count();
        assert!(same < 5);
    }

    #[test]
    fn gen_range_bounds() {
        let mut r = Rng::new(7);
        for _ in 0..10_000 {
            let v = r.gen_range(3, 9);
            assert!((3..9).contains(&v));
        }
    }

    #[test]
    fn gen_f32_bounds() {
        let mut r = Rng::new(7);
        for _ in 0..10_000 {
            let v = r.gen_f32();
            assert!((0.0..1.0).contains(&v));
        }
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test rng`
Expected: 4 passed.

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "feat: deterministic PCG32 rng for sim core"
```

---

### Task 3: Sim time (`sim/time.rs`)

**Files:**
- Create: `src/sim/time.rs` (uncomment `pub mod time;`)

- [ ] **Step 1: Write implementation + tests**

`src/sim/time.rs`:
```rust
pub const TICKS_PER_SECOND: u32 = 60;
/// 1 game day = 4 real minutes at 1x speed.
pub const TICKS_PER_HOUR: u64 = 600;
pub const HOURS_PER_DAY: u64 = 24;
pub const TICKS_PER_DAY: u64 = TICKS_PER_HOUR * HOURS_PER_DAY;
pub const TICK_DT: f32 = 1.0 / TICKS_PER_SECOND as f32;

/// Day number, starting at 1.
pub fn day(tick: u64) -> u64 {
    tick / TICKS_PER_DAY + 1
}

/// Hour of day, 0..24.
pub fn hour(tick: u64) -> u32 {
    ((tick % TICKS_PER_DAY) / TICKS_PER_HOUR) as u32
}

/// Fractional hour of day, 0.0..24.0 (drives lighting).
pub fn hour_f(tick: u64) -> f32 {
    (tick % TICKS_PER_DAY) as f32 / TICKS_PER_HOUR as f32
}

/// Daylight factor 0.0 (deep night) ..= 1.0 (midday), peaking at 13:00.
pub fn daylight(tick: u64) -> f32 {
    let t = (hour_f(tick) - 13.0) / 24.0 * std::f32::consts::TAU;
    (t.cos() * 0.5 + 0.5).powf(1.4)
}

pub fn is_night(tick: u64) -> bool {
    let h = hour(tick);
    h >= 22 || h < 6
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_math() {
        assert_eq!(day(0), 1);
        assert_eq!(hour(0), 0);
        assert_eq!(hour(TICKS_PER_HOUR * 13), 13);
        assert_eq!(day(TICKS_PER_DAY), 2);
        assert_eq!(hour(TICKS_PER_DAY + TICKS_PER_HOUR * 5), 5);
    }

    #[test]
    fn daylight_curve() {
        let noon = TICKS_PER_HOUR * 13;
        let midnight = TICKS_PER_HOUR * 1;
        assert!(daylight(noon) > 0.95);
        assert!(daylight(midnight) < 0.05);
        assert!(is_night(midnight) && !is_night(noon));
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test time`
Expected: 2 passed.

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "feat: sim clock with day/night curve"
```

---

### Task 4: City grid + procedural generation (`sim/city.rs`)

**Files:**
- Create: `src/sim/city.rs` (uncomment `pub mod city;`)

- [ ] **Step 1: Write the failing tests**

At the bottom of the new `src/sim/city.rs` (types from Step 2 above them):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::rng::Rng;

    #[test]
    fn generates_required_buildings() {
        for seed in 1..=5u64 {
            let city = City::generate(&mut Rng::new(seed));
            let count = |k| city.buildings.iter().filter(|b| b.kind == k).count();
            assert!(count(BuildingKind::Apartment) >= 10, "seed {seed}");
            assert!(count(BuildingKind::NoodleBar) >= 2);
            assert!(count(BuildingKind::VendingPlaza) >= 2);
            assert!(count(BuildingKind::FusionPlant) >= 1);
            assert!(count(BuildingKind::HydroFarm) >= 1);
            assert!(count(BuildingKind::RoboticsFab) >= 1);
            assert!(count(BuildingKind::DataCenter) >= 1);
            assert!(count(BuildingKind::Arcade) >= 1);
            assert!(count(BuildingKind::HoloPark) >= 1);
        }
    }

    #[test]
    fn doors_are_roads_adjacent_to_building() {
        let city = City::generate(&mut Rng::new(3));
        for b in &city.buildings {
            let (dx, dy) = b.door;
            assert!(city.is_road(dx, dy), "door of {:?} not on road", b.kind);
            // door must touch the building rect
            let touches = dx >= b.x - 1
                && dx <= b.x + b.w
                && dy >= b.y - 1
                && dy <= b.y + b.h;
            assert!(touches, "door of {:?} not adjacent", b.kind);
        }
    }

    #[test]
    fn building_tiles_match_rects() {
        let city = City::generate(&mut Rng::new(9));
        for y in 0..city.h {
            for x in 0..city.w {
                if let Tile::Building(id) = city.tile(x, y) {
                    let b = &city.buildings[id as usize];
                    assert!(x >= b.x && x < b.x + b.w && y >= b.y && y < b.y + b.h);
                }
            }
        }
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test city`
Expected: FAIL — types don't exist yet.

- [ ] **Step 3: Write the implementation**

Top of `src/sim/city.rs`:
```rust
use crate::sim::rng::Rng;

pub const CITY_W: i32 = 49;
pub const CITY_H: i32 = 49;
pub const BLOCK: i32 = 6; // road every 6th row/col

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tile {
    Road,
    Pavement,
    Building(u16),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BuildingKind {
    Apartment,
    NoodleBar,
    VendingPlaza,
    FusionPlant,
    HydroFarm,
    RoboticsFab,
    DataCenter,
    Arcade,
    HoloPark,
}

impl BuildingKind {
    pub fn name(&self) -> &'static str {
        match self {
            BuildingKind::Apartment => "Hab Block",
            BuildingKind::NoodleBar => "Noodle Bar",
            BuildingKind::VendingPlaza => "Vending Plaza",
            BuildingKind::FusionPlant => "Fusion Plant",
            BuildingKind::HydroFarm => "Hydro Farm",
            BuildingKind::RoboticsFab => "Robotics Fab",
            BuildingKind::DataCenter => "Data Center",
            BuildingKind::Arcade => "Holo Arcade",
            BuildingKind::HoloPark => "Holo Park",
        }
    }

    pub fn is_workplace(&self) -> bool {
        matches!(
            self,
            BuildingKind::FusionPlant
                | BuildingKind::HydroFarm
                | BuildingKind::RoboticsFab
                | BuildingKind::DataCenter
        )
    }

    pub fn is_food(&self) -> bool {
        matches!(self, BuildingKind::NoodleBar | BuildingKind::VendingPlaza)
    }

    pub fn is_leisure(&self) -> bool {
        matches!(self, BuildingKind::Arcade | BuildingKind::HoloPark)
    }
}

pub struct Building {
    pub id: u16,
    pub kind: BuildingKind,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    /// Road tile adjacent to the building where citizens enter.
    pub door: (i32, i32),
    /// Food venues only: meals in stock.
    pub stock: f32,
    /// Citizen ids currently inside.
    pub occupants: Vec<usize>,
    /// Citizen ids employed here.
    pub workers: Vec<usize>,
    /// Per-building visual variation seed.
    pub vis_seed: u32,
}

pub struct City {
    pub w: i32,
    pub h: i32,
    pub tiles: Vec<Tile>,
    pub buildings: Vec<Building>,
}

impl City {
    pub fn tile(&self, x: i32, y: i32) -> Tile {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return Tile::Pavement;
        }
        self.tiles[(y * self.w + x) as usize]
    }

    pub fn is_road(&self, x: i32, y: i32) -> bool {
        self.tile(x, y) == Tile::Road
    }

    pub fn buildings_of(&self, pred: impl Fn(BuildingKind) -> bool) -> impl Iterator<Item = &Building> {
        self.buildings.iter().filter(move |b| pred(b.kind))
    }

    /// A random road tile within `radius` (chebyshev) of `from`.
    pub fn random_road_near(&self, rng: &mut Rng, from: (i32, i32), radius: i32) -> (i32, i32) {
        for _ in 0..32 {
            let x = rng.gen_range(from.0 - radius, from.0 + radius + 1).clamp(0, self.w - 1);
            let y = rng.gen_range(from.1 - radius, from.1 + radius + 1).clamp(0, self.h - 1);
            if self.is_road(x, y) {
                return (x, y);
            }
        }
        from
    }

    pub fn generate(rng: &mut Rng) -> City {
        let (w, h) = (CITY_W, CITY_H);
        let mut tiles = vec![Tile::Pavement; (w * h) as usize];
        for y in 0..h {
            for x in 0..w {
                if x % BLOCK == 0 || y % BLOCK == 0 {
                    tiles[(y * w + x) as usize] = Tile::Road;
                }
            }
        }

        // 8x8 blocks of 5x5 interior tiles, classified into rings from center.
        let mut rings: [Vec<(i32, i32)>; 3] = [vec![], vec![], vec![]];
        for by in 0..8 {
            for bx in 0..8 {
                let d = (bx as f32 - 3.5).abs().max((by as f32 - 3.5).abs());
                let ring = if d <= 1.5 { 0 } else if d <= 2.5 { 1 } else { 2 };
                rings[ring].push((bx, by));
            }
        }
        for r in rings.iter_mut() {
            rng.shuffle(r);
        }

        use BuildingKind::*;
        // (kind, count, preferred ring): commercial center, residential mid, industrial outer.
        let wishlist: [(BuildingKind, usize, usize); 9] = [
            (NoodleBar, 4, 0),
            (VendingPlaza, 3, 0),
            (Arcade, 3, 0),
            (DataCenter, 2, 0),
            (Apartment, 12, 1),
            (HoloPark, 4, 1),
            (FusionPlant, 2, 2),
            (HydroFarm, 2, 2),
            (RoboticsFab, 3, 2),
        ];

        let mut city = City { w, h, tiles, buildings: vec![] };
        for (kind, count, pref) in wishlist {
            for _ in 0..count {
                // take a block from preferred ring, falling back to any non-empty
                let order = [pref, (pref + 1) % 3, (pref + 2) % 3];
                let block = order.iter().find_map(|&r| rings[r].pop());
                let Some((bx, by)) = block else { break };
                city.place_building(rng, kind, bx, by);
            }
        }
        city
    }

    fn place_building(&mut self, rng: &mut Rng, kind: BuildingKind, bx: i32, by: i32) {
        let (ox, oy) = (bx * BLOCK + 1, by * BLOCK + 1);
        let full = matches!(kind, BuildingKind::FusionPlant | BuildingKind::HoloPark);
        let bw = if full { 5 } else { rng.gen_range(3, 6) };
        let bh = if full { 5 } else { rng.gen_range(3, 6) };
        // snap to a corner of the block so at least two sides face roads
        let x = ox + if rng.chance(0.5) { 0 } else { 5 - bw };
        let y = oy + if rng.chance(0.5) { 0 } else { 5 - bh };

        let id = self.buildings.len() as u16;
        for ty in y..y + bh {
            for tx in x..x + bw {
                self.tiles[(ty * self.w + tx) as usize] = Tile::Building(id);
            }
        }

        // collect road tiles adjacent to the rect perimeter, pick one as the door
        let mut candidates = vec![];
        for tx in x..x + bw {
            for &dy in &[y - 1, y + bh] {
                if self.is_road(tx, dy) {
                    candidates.push((tx, dy));
                }
            }
        }
        for ty in y..y + bh {
            for &dx in &[x - 1, x + bw] {
                if self.is_road(dx, ty) {
                    candidates.push((dx, ty));
                }
            }
        }
        let door = candidates[rng.gen_range(0, candidates.len() as i32) as usize];

        let stock = if kind.is_food() { 20.0 } else { 0.0 };
        self.buildings.push(Building {
            id,
            kind,
            x,
            y,
            w: bw,
            h: bh,
            door,
            stock,
            occupants: vec![],
            workers: vec![],
            vis_seed: rng.next_u32(),
        });
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test city`
Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: procedural city grid with zoned districts"
```

---

### Task 5: A* pathfinding (`sim/path.rs`)

**Files:**
- Create: `src/sim/path.rs` (uncomment `pub mod path;`)

- [ ] **Step 1: Write the failing tests**

Bottom of new `src/sim/path.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::city::City;
    use crate::sim::rng::Rng;

    #[test]
    fn straight_road_path() {
        let city = City::generate(&mut Rng::new(1));
        // row 0 is all road
        let p = find_path(&city, (0, 0), (10, 0)).expect("path");
        assert_eq!(p.first(), Some(&(0, 0)));
        assert_eq!(p.last(), Some(&(10, 0)));
        assert_eq!(p.len(), 11); // manhattan-optimal along one road
    }

    #[test]
    fn all_doors_reachable() {
        let city = City::generate(&mut Rng::new(4));
        let start = city.buildings[0].door;
        for b in &city.buildings {
            assert!(
                find_path(&city, start, b.door).is_some(),
                "{:?} unreachable",
                b.kind
            );
        }
    }

    #[test]
    fn non_road_target_fails() {
        let city = City::generate(&mut Rng::new(1));
        // find a building tile
        let b = &city.buildings[0];
        assert!(find_path(&city, (0, 0), (b.x, b.y)).is_none());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test path`
Expected: FAIL — `find_path` not defined.

- [ ] **Step 3: Write the implementation**

Top of `src/sim/path.rs`:
```rust
use crate::sim::city::City;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// A* over road tiles. Returns waypoints from `from` to `to` inclusive.
pub fn find_path(city: &City, from: (i32, i32), to: (i32, i32)) -> Option<Vec<(i32, i32)>> {
    if !city.is_road(from.0, from.1) || !city.is_road(to.0, to.1) {
        return None;
    }
    let w = city.w;
    let idx = |p: (i32, i32)| (p.1 * w + p.0) as usize;
    let n = (city.w * city.h) as usize;
    let mut g = vec![u32::MAX; n];
    let mut came: Vec<u32> = vec![u32::MAX; n];
    let heur = |p: (i32, i32)| ((p.0 - to.0).abs() + (p.1 - to.1).abs()) as u32;

    let mut open = BinaryHeap::new();
    g[idx(from)] = 0;
    open.push(Reverse((heur(from), idx(from))));

    while let Some(Reverse((_, cur))) = open.pop() {
        let cur_p = (cur as i32 % w, cur as i32 / w);
        if cur_p == to {
            let mut path = vec![to];
            let mut at = cur;
            while came[at] != u32::MAX {
                at = came[at] as usize;
                path.push((at as i32 % w, at as i32 / w));
            }
            path.reverse();
            return Some(path);
        }
        let ng = g[cur] + 1;
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let np = (cur_p.0 + dx, cur_p.1 + dy);
            if !city.is_road(np.0, np.1) {
                continue;
            }
            let ni = idx(np);
            if ng < g[ni] {
                g[ni] = ng;
                came[ni] = cur as u32;
                open.push(Reverse((ng + heur(np), ni)));
            }
        }
    }
    None
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test path`
Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: A* pathfinding over road network"
```

---

### Task 6: Citizens — needs, personality, names (`sim/citizen.rs`)

**Files:**
- Create: `src/sim/citizen.rs` (uncomment `pub mod citizen;`)

- [ ] **Step 1: Write the failing tests**

Bottom of new `src/sim/citizen.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::rng::Rng;

    #[test]
    fn needs_decay_over_time() {
        let mut c = Citizen::spawn(&mut Rng::new(5), 0, 0, (1.5, 0.5));
        c.needs = Needs::full();
        for _ in 0..1000 {
            c.decay_needs();
        }
        assert!(c.needs.hunger < 1.0);
        assert!(c.needs.energy < 1.0);
        assert!(c.needs.hygiene < 1.0);
        assert!(c.needs.fun < 1.0);
        assert!(c.needs.hunger > 0.5, "decay too fast");
    }

    #[test]
    fn needs_clamp_at_zero() {
        let mut c = Citizen::spawn(&mut Rng::new(5), 0, 0, (1.5, 0.5));
        for _ in 0..2_000_000 {
            c.decay_needs();
        }
        assert!(c.needs.hunger >= 0.0 && c.needs.fun >= 0.0);
    }

    #[test]
    fn spawn_is_deterministic() {
        let a = Citizen::spawn(&mut Rng::new(9), 3, 1, (0.5, 0.5));
        let b = Citizen::spawn(&mut Rng::new(9), 3, 1, (0.5, 0.5));
        assert_eq!(a.name, b.name);
        assert_eq!(a.personality.archetype, b.personality.archetype);
        assert_eq!(a.money, b.money);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test citizen`
Expected: FAIL — types not defined.

- [ ] **Step 3: Write the implementation**

Top of `src/sim/citizen.rs`:
```rust
use crate::sim::rng::Rng;
use crate::sim::time::TICKS_PER_HOUR;
use std::collections::VecDeque;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NeedKind {
    Hunger,
    Energy,
    Hygiene,
    Fun,
}

pub const NEED_KINDS: [NeedKind; 4] = [
    NeedKind::Hunger,
    NeedKind::Energy,
    NeedKind::Hygiene,
    NeedKind::Fun,
];

impl NeedKind {
    pub fn label(&self) -> &'static str {
        match self {
            NeedKind::Hunger => "HUNGER",
            NeedKind::Energy => "ENERGY",
            NeedKind::Hygiene => "HYGIENE",
            NeedKind::Fun => "FUN",
        }
    }
    fn index(&self) -> usize {
        *self as usize
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Needs {
    pub hunger: f32,
    pub energy: f32,
    pub hygiene: f32,
    pub fun: f32,
}

impl Needs {
    pub fn full() -> Self {
        Needs { hunger: 1.0, energy: 1.0, hygiene: 1.0, fun: 1.0 }
    }
    pub fn get(&self, k: NeedKind) -> f32 {
        match k {
            NeedKind::Hunger => self.hunger,
            NeedKind::Energy => self.energy,
            NeedKind::Hygiene => self.hygiene,
            NeedKind::Fun => self.fun,
        }
    }
    pub fn add(&mut self, k: NeedKind, dv: f32) {
        let v = (self.get(k) + dv).clamp(0.0, 1.0);
        match k {
            NeedKind::Hunger => self.hunger = v,
            NeedKind::Energy => self.energy = v,
            NeedKind::Hygiene => self.hygiene = v,
            NeedKind::Fun => self.fun = v,
        }
    }
    pub fn min_value(&self) -> f32 {
        self.hunger.min(self.energy).min(self.hygiene).min(self.fun)
    }
}

/// Base decay per tick, indexed [hunger, energy, hygiene, fun]:
/// full bar drains in 16h / 20h / 24h / 14h of game time.
const BASE_DECAY: [f32; 4] = [
    1.0 / (16.0 * TICKS_PER_HOUR as f32),
    1.0 / (20.0 * TICKS_PER_HOUR as f32),
    1.0 / (24.0 * TICKS_PER_HOUR as f32),
    1.0 / (14.0 * TICKS_PER_HOUR as f32),
];

#[derive(Clone, Copy, Debug)]
pub struct Personality {
    pub archetype: &'static str,
    /// Importance multiplier per need when scoring actions.
    pub weights: [f32; 4],
    /// Decay-rate multiplier per need.
    pub decay_mult: [f32; 4],
}

const ARCHETYPES: [Personality; 5] = [
    Personality { archetype: "Balanced", weights: [1.0, 1.0, 1.0, 1.0], decay_mult: [1.0, 1.0, 1.0, 1.0] },
    Personality { archetype: "Workaholic", weights: [1.0, 0.9, 0.9, 0.6], decay_mult: [1.0, 1.1, 1.0, 0.8] },
    Personality { archetype: "Hedonist", weights: [1.0, 0.9, 0.8, 1.6], decay_mult: [1.0, 1.0, 1.0, 1.3] },
    Personality { archetype: "Slob", weights: [1.2, 1.0, 0.5, 1.1], decay_mult: [1.1, 1.0, 0.7, 1.0] },
    Personality { archetype: "Neat Freak", weights: [0.9, 1.0, 1.7, 0.9], decay_mult: [1.0, 1.0, 1.4, 1.0] },
];

#[derive(Clone, Copy, Debug)]
pub struct Job {
    pub workplace: u16,
    pub shift_start: u32,
    pub shift_end: u32,
    pub wage_per_hour: f32,
}

impl Job {
    pub fn in_shift(&self, hour: u32) -> bool {
        self.shift_start <= hour && hour < self.shift_end
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Activity {
    Sleep,
    Shower,
    Eat,
    Fun,
    Work,
    Stroll,
}

impl Activity {
    pub fn label(&self) -> &'static str {
        match self {
            Activity::Sleep => "Sleeping",
            Activity::Shower => "Showering",
            Activity::Eat => "Eating",
            Activity::Fun => "Having fun",
            Activity::Work => "Working",
            Activity::Stroll => "Strolling",
        }
    }
    pub fn need(&self) -> Option<NeedKind> {
        match self {
            Activity::Sleep => Some(NeedKind::Energy),
            Activity::Shower => Some(NeedKind::Hygiene),
            Activity::Eat => Some(NeedKind::Hunger),
            Activity::Fun => Some(NeedKind::Fun),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CitizenState {
    /// Thinking; will pick a new action at `until` tick.
    Idle { until: u64 },
    /// Walking. `to: None` means an aimless stroll.
    Traveling { to: Option<u16>, activity: Activity },
    Performing { at: u16, activity: Activity },
}

pub struct Citizen {
    pub id: usize,
    pub name: String,
    pub pos: (f32, f32),
    pub home: u16,
    pub job: Option<Job>,
    pub needs: Needs,
    pub money: f32,
    pub personality: Personality,
    pub state: CitizenState,
    pub path: VecDeque<(i32, i32)>,
    /// Tiles per tick.
    pub speed: f32,
}

const FIRST_NAMES: [&str; 20] = [
    "Aria", "Juno", "Kai", "Vesper", "Orion", "Nova", "Ezra", "Lyra", "Dex", "Mira",
    "Caspian", "Zara", "Niko", "Echo", "Sol", "Indra", "Rune", "Vega", "Atlas", "Wren",
];
const LAST_NAMES: [&str; 16] = [
    "Tanaka", "Voss", "Okafor", "Reyes", "Stellan", "Qiu", "Marlowe", "Ito",
    "Kade", "Sorenson", "Anand", "Petrov", "Calloway", "Nyx", "Moreau", "Zhou",
];

impl Citizen {
    pub fn spawn(rng: &mut Rng, id: usize, home: u16, door_pos: (f32, f32)) -> Citizen {
        let first = FIRST_NAMES[rng.gen_range(0, FIRST_NAMES.len() as i32) as usize];
        let last = LAST_NAMES[rng.gen_range(0, LAST_NAMES.len() as i32) as usize];
        let personality = ARCHETYPES[rng.gen_range(0, ARCHETYPES.len() as i32) as usize];
        Citizen {
            id,
            name: format!("{first} {last}"),
            pos: door_pos,
            home,
            job: None,
            needs: Needs {
                hunger: rng.gen_f32_range(0.5, 1.0),
                energy: rng.gen_f32_range(0.5, 1.0),
                hygiene: rng.gen_f32_range(0.5, 1.0),
                fun: rng.gen_f32_range(0.5, 1.0),
            },
            money: rng.gen_f32_range(40.0, 80.0),
            personality,
            state: CitizenState::Idle { until: 0 },
            path: VecDeque::new(),
            speed: rng.gen_f32_range(0.045, 0.06),
        }
    }

    pub fn tile(&self) -> (i32, i32) {
        (self.pos.0 as i32, self.pos.1 as i32)
    }

    pub fn decay_needs(&mut self) {
        for k in NEED_KINDS {
            let i = k.index();
            self.needs.add(k, -BASE_DECAY[i] * self.personality.decay_mult[i]);
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test citizen`
Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: citizens with needs, personality archetypes, decay"
```

---

### Task 7: Economy constants (`sim/economy.rs`)

**Files:**
- Create: `src/sim/economy.rs` (uncomment `pub mod economy;`)

- [ ] **Step 1: Write implementation + tests**

`src/sim/economy.rs`:
```rust
use crate::sim::city::{BuildingKind, City};
use crate::sim::time::TICKS_PER_HOUR;

pub fn meal_price(kind: BuildingKind) -> f32 {
    match kind {
        BuildingKind::NoodleBar => 12.0,
        BuildingKind::VendingPlaza => 5.0,
        _ => 0.0,
    }
}

/// Hunger restored per tick while eating.
pub fn eat_rate(kind: BuildingKind) -> f32 {
    match kind {
        BuildingKind::NoodleBar => 1.0 / 300.0,
        BuildingKind::VendingPlaza => 1.0 / 450.0,
        _ => 0.0,
    }
}

pub fn fun_price(kind: BuildingKind) -> f32 {
    match kind {
        BuildingKind::Arcade => 8.0,
        _ => 0.0,
    }
}

/// Fun restored per tick.
pub fn fun_rate(kind: BuildingKind) -> f32 {
    match kind {
        BuildingKind::Arcade => 1.0 / 900.0,
        BuildingKind::HoloPark => 1.0 / 1500.0,
        _ => 0.0,
    }
}

pub const SLEEP_RATE: f32 = 1.0 / (7.0 * TICKS_PER_HOUR as f32);
pub const SHOWER_RATE: f32 = 1.0 / (0.4 * TICKS_PER_HOUR as f32);
pub const STOCK_CAP: f32 = 60.0;
/// Meals produced per hydro farm per production hour (06:00–22:00).
pub const FARM_OUTPUT_PER_HOUR: f32 = 6.0;

/// Hourly tick: farms grow food, distributed evenly to food venues.
pub fn produce_food(city: &mut City, hour: u32) {
    if !(6..22).contains(&hour) {
        return;
    }
    let farms = city
        .buildings
        .iter()
        .filter(|b| b.kind == BuildingKind::HydroFarm)
        .count() as f32;
    let venues: Vec<usize> = city
        .buildings
        .iter()
        .filter(|b| b.kind.is_food())
        .map(|b| b.id as usize)
        .collect();
    if venues.is_empty() || farms == 0.0 {
        return;
    }
    let share = farms * FARM_OUTPUT_PER_HOUR / venues.len() as f32;
    for id in venues {
        let b = &mut city.buildings[id];
        b.stock = (b.stock + share).min(STOCK_CAP);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::rng::Rng;

    #[test]
    fn farms_stock_food_venues() {
        let mut city = City::generate(&mut Rng::new(2));
        for b in city.buildings.iter_mut().filter(|b| b.kind.is_food()) {
            b.stock = 0.0;
        }
        produce_food(&mut city, 10);
        for b in city.buildings.iter().filter(|b| b.kind.is_food()) {
            assert!(b.stock > 0.0);
        }
    }

    #[test]
    fn no_production_at_night() {
        let mut city = City::generate(&mut Rng::new(2));
        for b in city.buildings.iter_mut().filter(|b| b.kind.is_food()) {
            b.stock = 0.0;
        }
        produce_food(&mut city, 3);
        for b in city.buildings.iter().filter(|b| b.kind.is_food()) {
            assert_eq!(b.stock, 0.0);
        }
    }

    #[test]
    fn stock_caps() {
        let mut city = City::generate(&mut Rng::new(2));
        for _ in 0..1000 {
            produce_food(&mut city, 10);
        }
        for b in city.buildings.iter().filter(|b| b.kind.is_food()) {
            assert!(b.stock <= STOCK_CAP);
        }
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test economy`
Expected: 3 passed.

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "feat: economy — prices, rates, farm food production"
```

---

### Task 8: Utility AI (`sim/ai.rs`)

**Files:**
- Create: `src/sim/ai.rs` (uncomment `pub mod ai;`)

- [ ] **Step 1: Write the failing tests**

Bottom of new `src/sim/ai.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::citizen::{Citizen, Job};
    use crate::sim::city::City;
    use crate::sim::rng::Rng;

    fn setup() -> (City, Citizen) {
        let mut rng = Rng::new(11);
        let city = City::generate(&mut rng);
        let home = city.buildings.iter().find(|b| b.kind == BuildingKind::Apartment).unwrap();
        let door = (home.door.0 as f32 + 0.5, home.door.1 as f32 + 0.5);
        let mut c = Citizen::spawn(&mut rng, 0, home.id, door);
        c.needs = crate::sim::citizen::Needs::full();
        c.job = None;
        (city, c)
    }

    #[test]
    fn urgency_grows_as_need_empties() {
        assert!(urgency(0.1) > urgency(0.5));
        assert!(urgency(0.5) > urgency(0.9));
        assert!(urgency(1.0) < 0.01);
    }

    #[test]
    fn hungry_citizen_with_money_eats() {
        let (city, mut c) = setup();
        c.needs.hunger = 0.1;
        c.money = 100.0;
        let (b, act) = choose_action(&c, &city, 12, false).expect("action");
        assert_eq!(act, Activity::Eat);
        assert!(city.buildings[b as usize].kind.is_food());
    }

    #[test]
    fn broke_citizen_does_not_buy_food() {
        let (city, mut c) = setup();
        c.needs.hunger = 0.1;
        c.money = 0.0;
        let choice = choose_action(&c, &city, 12, false);
        if let Some((b, act)) = choice {
            assert!(
                !(act == Activity::Eat && city.buildings[b as usize].kind.is_food()),
                "bought food with no money"
            );
        }
    }

    #[test]
    fn satisfied_citizen_does_nothing() {
        let (city, c) = setup();
        assert!(choose_action(&c, &city, 12, false).is_none());
    }

    #[test]
    fn employed_citizen_works_during_shift() {
        let (city, mut c) = setup();
        let wp = city.buildings.iter().find(|b| b.kind.is_workplace()).unwrap();
        c.job = Some(Job { workplace: wp.id, shift_start: 8, shift_end: 16, wage_per_hour: 14.0 });
        let (b, act) = choose_action(&c, &city, 10, false).expect("action");
        assert_eq!(act, Activity::Work);
        assert_eq!(b, wp.id);
    }

    #[test]
    fn critical_need_overrides_work() {
        let (city, mut c) = setup();
        let wp = city.buildings.iter().find(|b| b.kind.is_workplace()).unwrap();
        c.job = Some(Job { workplace: wp.id, shift_start: 8, shift_end: 16, wage_per_hour: 14.0 });
        c.needs.hunger = 0.05;
        c.money = 100.0;
        let (_, act) = choose_action(&c, &city, 10, false).expect("action");
        assert_eq!(act, Activity::Eat);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test ai`
Expected: FAIL — functions not defined.

- [ ] **Step 3: Write the implementation**

Top of `src/sim/ai.rs`:
```rust
use crate::sim::citizen::{Activity, Citizen, NeedKind};
use crate::sim::city::{BuildingKind, City};
use crate::sim::economy;

/// How loudly an unmet need screams. 0 when full, ~3 when empty, non-linear.
pub fn urgency(value: f32) -> f32 {
    let d = 1.0 - value.clamp(0.0, 1.0);
    d * d * (1.0 + 2.0 * d)
}

const CRITICAL: f32 = 0.15;
/// Needs above this are not worth acting on.
const ACT_BELOW: f32 = 0.8;
const DIST_PENALTY: f32 = 0.01;
const MIN_SCORE: f32 = 0.02;

fn need_index(k: NeedKind) -> usize {
    k as usize
}

/// Pick the best (building, activity) for an idle citizen, or None to laze.
pub fn choose_action(
    c: &Citizen,
    city: &City,
    hour: u32,
    is_night: bool,
) -> Option<(u16, Activity)> {
    let critical = c.needs.min_value() < CRITICAL;

    // Work is scheduled, not scored — unless something is critical.
    if !critical {
        if let Some(job) = &c.job {
            if job.in_shift(hour) {
                return Some((job.workplace, Activity::Work));
            }
        }
    }

    let from = c.tile();
    let mut best: Option<(f32, u16, Activity)> = None;
    let mut consider = |score: f32, b: u16, act: Activity| {
        if score > MIN_SCORE && best.map_or(true, |(s, _, _)| score > s) {
            best = Some((score, b, act));
        }
    };

    let dist = |door: (i32, i32)| {
        ((door.0 - from.0).abs() + (door.1 - from.1).abs()) as f32 * DIST_PENALTY
    };
    let weight = |k: NeedKind| c.personality.weights[need_index(k)];

    // Home: sleep & shower, free.
    let home = &city.buildings[c.home as usize];
    if c.needs.energy < ACT_BELOW {
        let night_bonus = if is_night { 1.5 } else { 1.0 };
        let s = urgency(c.needs.energy) * weight(NeedKind::Energy) * night_bonus - dist(home.door);
        consider(s, home.id, Activity::Sleep);
    }
    if c.needs.hygiene < ACT_BELOW {
        let s = urgency(c.needs.hygiene) * weight(NeedKind::Hygiene) - dist(home.door);
        consider(s, home.id, Activity::Shower);
    }

    // Food venues: must have stock and be affordable.
    if c.needs.hunger < ACT_BELOW {
        for b in city.buildings_of(BuildingKind::is_food) {
            if b.stock < 1.0 || c.money < economy::meal_price(b.kind) {
                continue;
            }
            let s = urgency(c.needs.hunger) * weight(NeedKind::Hunger) - dist(b.door);
            consider(s, b.id, Activity::Eat);
        }
    }

    // Leisure.
    if c.needs.fun < ACT_BELOW {
        for b in city.buildings_of(BuildingKind::is_leisure) {
            if c.money < economy::fun_price(b.kind) {
                continue;
            }
            let s = urgency(c.needs.fun) * weight(NeedKind::Fun) - dist(b.door);
            consider(s, b.id, Activity::Fun);
        }
    }

    best.map(|(_, b, a)| (b, a))
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test ai`
Expected: 6 passed.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: utility-based AI action selection"
```

---

### Task 9: World — spawn, tick, integration (`sim/world.rs`)

**Files:**
- Create: `src/sim/world.rs` (uncomment `pub mod world;`)

- [ ] **Step 1: Write the failing tests**

Bottom of new `src/sim/world.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::citizen::{Activity, CitizenState, Needs};
    use crate::sim::time::TICKS_PER_HOUR;

    #[test]
    fn deterministic_across_runs() {
        let mut a = World::new(42, 30);
        let mut b = World::new(42, 30);
        for _ in 0..5000 {
            a.tick();
            b.tick();
        }
        assert_eq!(a.fingerprint(), b.fingerprint());
    }

    #[test]
    fn citizens_have_homes_and_most_have_jobs() {
        let w = World::new(7, 48);
        assert_eq!(w.citizens.len(), 48);
        let employed = w.citizens.iter().filter(|c| c.job.is_some()).count();
        assert!(employed >= 24, "only {employed} employed");
        for c in &w.citizens {
            assert_eq!(w.city.buildings[c.home as usize].kind, crate::sim::city::BuildingKind::Apartment);
        }
    }

    #[test]
    fn hungry_citizen_eventually_eats() {
        let mut w = World::new(13, 20);
        w.citizens[0].needs = Needs::full();
        w.citizens[0].needs.hunger = 0.12;
        w.citizens[0].money = 100.0;
        w.citizens[0].job = None;
        let money_before = w.citizens[0].money;
        let mut ate = false;
        for _ in 0..20_000 {
            w.tick();
            if matches!(w.citizens[0].state, CitizenState::Performing { activity: Activity::Eat, .. }) {
                ate = true;
                break;
            }
        }
        assert!(ate, "never started eating");
        assert!(w.citizens[0].money < money_before, "meal was free");
    }

    #[test]
    fn working_pays_wages() {
        let mut w = World::new(21, 20);
        // force citizen 0 into an always-on shift starting now (hour 0)
        let wp = w.city.buildings.iter().find(|b| b.kind.is_workplace()).unwrap().id;
        w.citizens[0].job = Some(crate::sim::citizen::Job {
            workplace: wp,
            shift_start: 0,
            shift_end: 24,
            wage_per_hour: 14.0,
        });
        w.citizens[0].needs = Needs::full();
        let before = w.citizens[0].money;
        for _ in 0..(TICKS_PER_HOUR * 3) {
            w.tick();
        }
        assert!(w.citizens[0].money > before, "no wages paid");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test world`
Expected: FAIL — `World` not defined.

- [ ] **Step 3: Write the implementation**

Top of `src/sim/world.rs`:
```rust
use crate::sim::ai;
use crate::sim::citizen::{Activity, Citizen, CitizenState, Job, NeedKind};
use crate::sim::city::{BuildingKind, City};
use crate::sim::economy;
use crate::sim::path;
use crate::sim::rng::Rng;
use crate::sim::time::{self, TICKS_PER_HOUR};
use std::collections::VecDeque;

pub struct World {
    pub rng: Rng,
    pub city: City,
    pub citizens: Vec<Citizen>,
    pub tick: u64,
    pub seed: u64,
}

const SHIFTS: [(u32, u32); 3] = [(8, 16), (16, 24), (0, 8)];
/// Weighted shift pick: most people work days.
const SHIFT_WEIGHTS: [f32; 3] = [0.6, 0.25, 0.15];

impl World {
    pub fn new(seed: u64, n_citizens: usize) -> World {
        let mut rng = Rng::new(seed);
        let city = City::generate(&mut rng);
        let mut world = World { rng, city, citizens: vec![], tick: 0, seed };

        let homes: Vec<u16> = world
            .city
            .buildings_of(|k| k == BuildingKind::Apartment)
            .map(|b| b.id)
            .collect();
        let workplaces: Vec<u16> = world
            .city
            .buildings_of(BuildingKind::is_workplace)
            .map(|b| b.id)
            .collect();

        for i in 0..n_citizens {
            let home = homes[i % homes.len()];
            let door = world.city.buildings[home as usize].door;
            let pos = (door.0 as f32 + 0.5, door.1 as f32 + 0.5);
            let mut c = Citizen::spawn(&mut world.rng, i, home, pos);
            // ~80% employed
            if world.rng.chance(0.8) {
                let wp = workplaces[i % workplaces.len()];
                let roll = world.rng.gen_f32();
                let shift = if roll < SHIFT_WEIGHTS[0] {
                    SHIFTS[0]
                } else if roll < SHIFT_WEIGHTS[0] + SHIFT_WEIGHTS[1] {
                    SHIFTS[1]
                } else {
                    SHIFTS[2]
                };
                c.job = Some(Job {
                    workplace: wp,
                    shift_start: shift.0,
                    shift_end: shift.1,
                    wage_per_hour: world.rng.gen_f32_range(11.0, 18.0),
                });
                world.city.buildings[wp as usize].workers.push(i);
            }
            c.state = CitizenState::Idle { until: world.rng.gen_range(0, 120) as u64 };
            world.citizens.push(c);
        }
        world
    }

    pub fn hour(&self) -> u32 {
        time::hour(self.tick)
    }
    pub fn hour_f(&self) -> f32 {
        time::hour_f(self.tick)
    }
    pub fn day(&self) -> u64 {
        time::day(self.tick)
    }

    pub fn tick(&mut self) {
        self.tick += 1;
        if self.tick % TICKS_PER_HOUR == 0 {
            economy::produce_food(&mut self.city, self.hour());
        }
        let (tick, hour, night) = (self.tick, self.hour(), time::is_night(self.tick));
        let city = &mut self.city;
        let rng = &mut self.rng;
        for c in self.citizens.iter_mut() {
            c.decay_needs();
            tick_citizen(c, city, rng, tick, hour, night);
        }
    }

    /// FNV-style digest of mutable sim state, for determinism tests.
    pub fn fingerprint(&self) -> u64 {
        fn mix(h: u64, v: u64) -> u64 {
            (h ^ v).wrapping_mul(0x100000001b3)
        }
        let mut h: u64 = 0xcbf29ce484222325;
        for c in &self.citizens {
            h = mix(h, c.pos.0.to_bits() as u64);
            h = mix(h, c.pos.1.to_bits() as u64);
            h = mix(h, c.money.to_bits() as u64);
            h = mix(h, c.needs.hunger.to_bits() as u64);
            h = mix(h, c.needs.energy.to_bits() as u64);
        }
        for b in &self.city.buildings {
            h = mix(h, b.stock.to_bits() as u64);
            h = mix(h, b.occupants.len() as u64);
        }
        h
    }
}

fn tick_citizen(
    c: &mut Citizen,
    city: &mut City,
    rng: &mut Rng,
    tick: u64,
    hour: u32,
    night: bool,
) {
    match c.state {
        CitizenState::Idle { until } => {
            if tick < until {
                return;
            }
            if let Some((b, act)) = ai::choose_action(c, city, hour, night) {
                start_travel(c, city, Some(b), act, tick, rng);
            } else if rng.chance(0.003) {
                // laze, occasionally wander toward a nearby corner
                let target = city.random_road_near(rng, c.tile(), 8);
                if let Some(p) = path::find_path(city, c.tile(), target) {
                    c.path = VecDeque::from(p);
                    c.state = CitizenState::Traveling { to: None, activity: Activity::Stroll };
                }
            }
        }
        CitizenState::Traveling { to, activity } => {
            if let Some(&(tx, ty)) = c.path.front() {
                let target = (tx as f32 + 0.5, ty as f32 + 0.5);
                let (dx, dy) = (target.0 - c.pos.0, target.1 - c.pos.1);
                let d = (dx * dx + dy * dy).sqrt();
                if d <= c.speed {
                    c.pos = target;
                    c.path.pop_front();
                } else {
                    c.pos.0 += dx / d * c.speed;
                    c.pos.1 += dy / d * c.speed;
                }
            }
            if c.path.is_empty() {
                match to {
                    Some(b) => arrive(c, city, b, activity, tick),
                    None => c.state = CitizenState::Idle { until: tick + rng.gen_range(60, 240) as u64 },
                }
            }
        }
        CitizenState::Performing { at, activity } => {
            let kind = city.buildings[at as usize].kind;
            let mut done = false;
            match activity {
                Activity::Sleep => {
                    c.needs.add(NeedKind::Energy, economy::SLEEP_RATE);
                    done = c.needs.energy >= 0.98;
                }
                Activity::Shower => {
                    c.needs.add(NeedKind::Hygiene, economy::SHOWER_RATE);
                    done = c.needs.hygiene >= 0.98;
                }
                Activity::Eat => {
                    c.needs.add(NeedKind::Hunger, economy::eat_rate(kind));
                    done = c.needs.hunger >= 0.97;
                }
                Activity::Fun => {
                    c.needs.add(NeedKind::Fun, economy::fun_rate(kind));
                    done = c.needs.fun >= 0.97;
                }
                Activity::Work => {
                    if let Some(job) = &c.job {
                        c.money += job.wage_per_hour / TICKS_PER_HOUR as f32;
                        done = !job.in_shift(hour) || c.needs.min_value() < 0.08;
                    } else {
                        done = true;
                    }
                }
                Activity::Stroll => done = true,
            }
            if done {
                city.buildings[at as usize].occupants.retain(|&id| id != c.id);
                let door = city.buildings[at as usize].door;
                c.pos = (door.0 as f32 + 0.5, door.1 as f32 + 0.5);
                c.state = CitizenState::Idle { until: tick + rng.gen_range(20, 90) as u64 };
            }
        }
    }
}

fn start_travel(c: &mut Citizen, city: &City, to: Option<u16>, act: Activity, tick: u64, rng: &mut Rng) {
    let Some(b) = to else { return };
    let door = city.buildings[b as usize].door;
    if c.tile() == door {
        // already at the door — walk straight in (arrive handles payment)
        c.path.clear();
        c.state = CitizenState::Traveling { to, activity: act };
        return;
    }
    match path::find_path(city, c.tile(), door) {
        Some(p) => {
            c.path = VecDeque::from(p);
            c.state = CitizenState::Traveling { to, activity: act };
        }
        None => c.state = CitizenState::Idle { until: tick + rng.gen_range(120, 360) as u64 },
    }
}

fn arrive(c: &mut Citizen, city: &mut City, b: u16, act: Activity, tick: u64) {
    let building = &mut city.buildings[b as usize];
    match act {
        Activity::Eat => {
            let price = economy::meal_price(building.kind);
            if building.stock < 1.0 || c.money < price {
                c.state = CitizenState::Idle { until: tick + 60 };
                return;
            }
            building.stock -= 1.0;
            c.money -= price;
        }
        Activity::Fun => {
            let price = economy::fun_price(building.kind);
            if c.money < price {
                c.state = CitizenState::Idle { until: tick + 60 };
                return;
            }
            c.money -= price;
        }
        _ => {}
    }
    building.occupants.push(c.id);
    c.state = CitizenState::Performing { at: b, activity: act };
}
```

- [ ] **Step 4: Run the full test suite**

Run: `cargo test`
Expected: all tests pass (rng 4, time 2, city 3, path 3, citizen 3, economy 3, ai 6, world 4 = 28).

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: world tick — AI, travel, activities, wages, determinism"
```

---

### Task 10: Camera (`ui/camera.rs`)

**Files:**
- Create: `src/ui/camera.rs`
- Modify: `src/ui/mod.rs` → `pub mod camera;`

- [ ] **Step 1: Write implementation + math tests**

`src/ui/camera.rs`:
```rust
use macroquad::prelude::*;

pub struct Camera {
    /// World-space (tile units) point at screen center.
    pub center: (f32, f32),
    /// Pixels per tile.
    pub ppt: f32,
    drag_anchor: Option<(f32, f32)>,
    pub dragged: bool,
}

impl Camera {
    pub fn new(center: (f32, f32), ppt: f32) -> Camera {
        Camera { center, ppt, drag_anchor: None, dragged: false }
    }

    pub fn to_screen(&self, wx: f32, wy: f32) -> (f32, f32) {
        (
            (wx - self.center.0) * self.ppt + screen_width() / 2.0,
            (wy - self.center.1) * self.ppt + screen_height() / 2.0,
        )
    }

    pub fn to_world(&self, sx: f32, sy: f32) -> (f32, f32) {
        (
            (sx - screen_width() / 2.0) / self.ppt + self.center.0,
            (sy - screen_height() / 2.0) / self.ppt + self.center.1,
        )
    }

    /// Handle pan (left-drag) + zoom (wheel, toward cursor).
    /// `ui_hover`: pointer is over UI; ignore input then.
    pub fn update(&mut self, ui_hover: bool) {
        let (mx, my) = mouse_position();
        let wheel = mouse_wheel().1;
        if wheel.abs() > 0.0 && !ui_hover {
            let before = self.to_world(mx, my);
            self.ppt = (self.ppt * (1.0 + wheel.signum() * 0.12)).clamp(6.0, 72.0);
            let after = self.to_world(mx, my);
            self.center.0 += before.0 - after.0;
            self.center.1 += before.1 - after.1;
        }
        if is_mouse_button_pressed(MouseButton::Left) && !ui_hover {
            self.drag_anchor = Some((mx, my));
            self.dragged = false;
        }
        if is_mouse_button_down(MouseButton::Left) {
            if let Some((ax, ay)) = self.drag_anchor {
                let (dx, dy) = (mx - ax, my - ay);
                if dx.abs() + dy.abs() > 4.0 {
                    self.dragged = true;
                }
                if self.dragged {
                    self.center.0 -= dx / self.ppt;
                    self.center.1 -= dy / self.ppt;
                    self.drag_anchor = Some((mx, my));
                }
            }
        } else {
            self.drag_anchor = None;
        }
    }
}
```

Note: `to_screen`/`to_world` use `screen_width()` so they can't be unit-tested headlessly; the round-trip identity is `(s - W/2)/ppt + c` ∘ `(w - c)*ppt + W/2` — verify by inspection. No tests for this file.

- [ ] **Step 2: Verify it compiles**

Run: `cargo check`
Expected: clean (warnings about unused code are fine until main.rs wires it).

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "feat: pan/zoom camera with cursor-anchored zoom"
```

---

### Task 11: City renderer — tiles, buildings, day/night (`render/mod.rs`, `render/buildings.rs`)

**Files:**
- Rewrite: `src/render/mod.rs`
- Create: `src/render/buildings.rs`
- Modify: `src/main.rs` (wire world + camera + render)

- [ ] **Step 1: Write the render orchestrator**

`src/render/mod.rs`:
```rust
pub mod agents;
pub mod buildings;

use crate::sim::time;
use crate::sim::world::World;
use crate::ui::camera::Camera;
use macroquad::prelude::*;

pub const ROAD: Color = Color::new(0.075, 0.085, 0.12, 1.0);
pub const PAVEMENT: Color = Color::new(0.11, 0.125, 0.17, 1.0);
pub const LANE: Color = Color::new(0.22, 0.26, 0.36, 1.0);

/// Tint-multiply a color by daylight ambient.
pub fn lit(c: Color, amb: f32) -> Color {
    let a = 0.45 + 0.55 * amb;
    Color::new(c.r * a, c.g * a, c.b * a, c.a)
}

pub fn draw_world(world: &World, cam: &Camera, t: f32, selected_building: Option<u16>) {
    let amb = time::daylight(world.tick);
    clear_background(Color::new(0.016, 0.02, 0.045, 1.0));

    // visible tile bounds
    let (wx0, wy0) = cam.to_world(0.0, 0.0);
    let (wx1, wy1) = cam.to_world(screen_width(), screen_height());
    let x0 = (wx0.floor() as i32 - 1).max(0);
    let y0 = (wy0.floor() as i32 - 1).max(0);
    let x1 = (wx1.ceil() as i32 + 1).min(world.city.w);
    let y1 = (wy1.ceil() as i32 + 1).min(world.city.h);

    // ground
    for y in y0..y1 {
        for x in x0..x1 {
            let (sx, sy) = cam.to_screen(x as f32, y as f32);
            let c = match world.city.tile(x, y) {
                crate::sim::city::Tile::Road => ROAD,
                _ => PAVEMENT,
            };
            draw_rectangle(sx, sy, cam.ppt + 1.0, cam.ppt + 1.0, lit(c, amb));
        }
    }
    // lane markings on the road grid lines
    for y in y0..y1 {
        for x in x0..x1 {
            if !world.city.is_road(x, y) {
                continue;
            }
            let center_row = y % crate::sim::city::BLOCK == 0 && x % 2 == 0;
            let center_col = x % crate::sim::city::BLOCK == 0 && y % 2 == 0;
            if center_row || center_col {
                let (sx, sy) = cam.to_screen(x as f32 + 0.42, y as f32 + 0.42);
                draw_rectangle(sx, sy, cam.ppt * 0.16, cam.ppt * 0.16, lit(LANE, amb));
            }
        }
    }

    buildings::draw_buildings(world, cam, t, amb, selected_building);

    // night overlay — neon layers draw after this and appear to glow
    let dark = (1.0 - amb) * 0.45;
    draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.01, 0.015, 0.06, dark));
    // dusk/dawn warm wash
    let h = world.hour_f();
    let dusk = (1.0 - ((h - 6.5).abs().min((h - 19.5).abs()) / 1.5)).max(0.0);
    if dusk > 0.0 {
        draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.9, 0.35, 0.1, dusk * 0.08));
    }

    buildings::draw_neon(world, cam, t, amb);
    agents::draw_citizens(world, cam, t);
}
```

- [ ] **Step 2: Write the building renderer**

`src/render/buildings.rs`:
```rust
use crate::sim::city::{Building, BuildingKind};
use crate::sim::time;
use crate::sim::world::World;
use crate::ui::camera::Camera;
use macroquad::prelude::*;

pub fn body_color(kind: BuildingKind) -> Color {
    match kind {
        BuildingKind::Apartment => Color::new(0.10, 0.14, 0.22, 1.0),
        BuildingKind::NoodleBar => Color::new(0.16, 0.09, 0.18, 1.0),
        BuildingKind::VendingPlaza => Color::new(0.10, 0.16, 0.13, 1.0),
        BuildingKind::FusionPlant => Color::new(0.17, 0.13, 0.08, 1.0),
        BuildingKind::HydroFarm => Color::new(0.08, 0.16, 0.12, 1.0),
        BuildingKind::RoboticsFab => Color::new(0.16, 0.12, 0.10, 1.0),
        BuildingKind::DataCenter => Color::new(0.09, 0.12, 0.20, 1.0),
        BuildingKind::Arcade => Color::new(0.15, 0.08, 0.20, 1.0),
        BuildingKind::HoloPark => Color::new(0.07, 0.13, 0.10, 1.0),
    }
}

pub fn trim_color(kind: BuildingKind) -> Color {
    match kind {
        BuildingKind::Apartment => Color::new(0.21, 0.88, 1.00, 1.0),
        BuildingKind::NoodleBar => Color::new(1.00, 0.24, 0.94, 1.0),
        BuildingKind::VendingPlaza => Color::new(0.62, 1.00, 0.34, 1.0),
        BuildingKind::FusionPlant => Color::new(1.00, 0.72, 0.24, 1.0),
        BuildingKind::HydroFarm => Color::new(0.34, 1.00, 0.62, 1.0),
        BuildingKind::RoboticsFab => Color::new(1.00, 0.55, 0.20, 1.0),
        BuildingKind::DataCenter => Color::new(0.30, 0.55, 1.00, 1.0),
        BuildingKind::Arcade => Color::new(1.00, 0.24, 0.82, 1.0),
        BuildingKind::HoloPark => Color::new(0.40, 1.00, 0.80, 1.0),
    }
}

fn hash(a: u32, b: u32, c: u32) -> u32 {
    let mut h = a ^ 0x9e3779b9;
    h = h.wrapping_mul(0x85ebca6b) ^ b;
    h = h.wrapping_mul(0xc2b2ae35) ^ c;
    h ^ (h >> 16)
}

pub fn draw_buildings(world: &World, cam: &Camera, t: f32, amb: f32, selected: Option<u16>) {
    let day_seed = world.day() as u32;
    for b in &world.city.buildings {
        let (sx, sy) = cam.to_screen(b.x as f32, b.y as f32);
        let (w, h) = (b.w as f32 * cam.ppt, b.h as f32 * cam.ppt);

        if b.kind == BuildingKind::HoloPark {
            draw_park(b, cam, t, amb);
            continue;
        }

        // base + inner roof slab
        let base = crate::render::lit(body_color(b.kind), amb);
        draw_rectangle(sx, sy, w, h, base);
        let inset = cam.ppt * 0.22;
        let roof = Color::new(base.r * 1.45 + 0.02, base.g * 1.45 + 0.02, base.b * 1.45 + 0.02, 1.0);
        draw_rectangle(sx + inset, sy + inset, w - inset * 2.0, h - inset * 2.0, roof);

        // roof greebles: AC units / vents, seeded
        let n = 2 + (b.vis_seed % 3) as i32;
        for i in 0..n {
            let hx = hash(b.vis_seed, i as u32, 1) % 1000;
            let hy = hash(b.vis_seed, i as u32, 2) % 1000;
            let gx = sx + inset + (hx as f32 / 1000.0) * (w - inset * 2.0 - cam.ppt * 0.3);
            let gy = sy + inset + (hy as f32 / 1000.0) * (h - inset * 2.0 - cam.ppt * 0.3);
            draw_rectangle(gx, gy, cam.ppt * 0.3, cam.ppt * 0.3, crate::render::lit(Color::new(0.05, 0.06, 0.1, 1.0), amb));
        }

        // skylight windows, lit per-night-per-building hash
        let lit_ratio = if time::is_night(world.tick) { 7 } else { 2 };
        let step = cam.ppt * 0.5;
        let (cols, rows) = (((w - inset * 2.0) / step) as i32, ((h - inset * 2.0) / step) as i32);
        for wy in 0..rows {
            for wx in 0..cols {
                if hash(b.vis_seed ^ day_seed, wx as u32, wy as u32) % 10 < lit_ratio {
                    let px = sx + inset + wx as f32 * step + step * 0.25;
                    let py = sy + inset + wy as f32 * step + step * 0.25;
                    draw_rectangle(px, py, step * 0.4, step * 0.4, Color::new(1.0, 0.85, 0.55, 0.5 + 0.5 * (1.0 - amb)));
                }
            }
        }

        if selected == Some(b.id) {
            draw_rectangle_lines(sx - 3.0, sy - 3.0, w + 6.0, h + 6.0, 3.0, WHITE);
        }
    }
}

fn draw_park(b: &Building, cam: &Camera, t: f32, amb: f32) {
    let (sx, sy) = cam.to_screen(b.x as f32, b.y as f32);
    let (w, h) = (b.w as f32 * cam.ppt, b.h as f32 * cam.ppt);
    draw_rectangle(sx, sy, w, h, crate::render::lit(body_color(BuildingKind::HoloPark), amb));
    // holo-trees
    for i in 0..5u32 {
        let hx = hash(b.vis_seed, i, 7) % 1000;
        let hy = hash(b.vis_seed, i, 8) % 1000;
        let px = sx + (0.15 + 0.7 * hx as f32 / 1000.0) * w;
        let py = sy + (0.15 + 0.7 * hy as f32 / 1000.0) * h;
        let r = cam.ppt * (0.28 + 0.06 * ((t * 1.3 + i as f32).sin()));
        draw_circle(px, py, r, Color::new(0.25, 0.95, 0.65, 0.35));
        draw_circle(px, py, r * 0.45, Color::new(0.5, 1.0, 0.8, 0.5));
    }
}

/// Neon pass — drawn after the night overlay so it pops.
pub fn draw_neon(world: &World, cam: &Camera, t: f32, amb: f32) {
    let glow = 0.55 + 0.45 * (1.0 - amb);
    for b in &world.city.buildings {
        let (sx, sy) = cam.to_screen(b.x as f32, b.y as f32);
        let (w, h) = (b.w as f32 * cam.ppt, b.h as f32 * cam.ppt);
        let mut c = trim_color(b.kind);
        c.a = glow;
        draw_rectangle_lines(sx, sy, w, h, (cam.ppt * 0.09).max(1.5), c);

        // door marker
        let (dx, dy) = b.door;
        let (dsx, dsy) = cam.to_screen(dx as f32 + 0.5, dy as f32 + 0.5);
        draw_circle(dsx, dsy, cam.ppt * 0.12, Color::new(c.r, c.g, c.b, glow * 0.8));

        // fusion plant core pulse
        if b.kind == BuildingKind::FusionPlant {
            let (cx, cy) = (sx + w / 2.0, sy + h / 2.0);
            let r = cam.ppt * (0.7 + 0.12 * (t * 2.4).sin());
            draw_circle(cx, cy, r, Color::new(1.0, 0.72, 0.24, 0.18));
            draw_circle(cx, cy, r * 0.5, Color::new(1.0, 0.85, 0.5, 0.35));
        }

        // signs when zoomed in
        if cam.ppt > 20.0 && !matches!(b.kind, BuildingKind::Apartment) {
            let label = b.kind.name().to_uppercase();
            let fs = (cam.ppt * 0.45).max(12.0);
            let dim = measure_text(&label, None, fs as u16, 1.0);
            draw_text(&label, sx + (w - dim.width) / 2.0, sy - cam.ppt * 0.15, fs, c);
        }
    }
}
```

- [ ] **Step 3: Stub `render/agents.rs` (filled in next task)**

`src/render/agents.rs`:
```rust
use crate::sim::world::World;
use crate::ui::camera::Camera;

pub fn draw_citizens(_world: &World, _cam: &Camera, _t: f32) {}
```

- [ ] **Step 4: Wire main.rs**

Replace `src/main.rs` body:
```rust
mod render;
mod sim;
mod ui;

use macroquad::prelude::*;
use sim::time::TICK_DT;
use sim::world::World;
use ui::camera::Camera;

fn window_conf() -> Conf {
    Conf {
        window_title: "NEON CITY".to_string(),
        window_width: 1360,
        window_height: 860,
        high_dpi: true,
        sample_count: 4,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let seed = 2161;
    let mut world = World::new(seed, 48);
    let mut cam = Camera::new((sim::city::CITY_W as f32 / 2.0, sim::city::CITY_H as f32 / 2.0), 16.0);
    let mut acc: f32 = 0.0;
    let speed: u32 = 1;

    loop {
        let t = get_time() as f32;
        acc += get_frame_time() * speed as f32;
        let mut steps = 0;
        while acc >= TICK_DT && steps < 240 {
            world.tick();
            acc -= TICK_DT;
            steps += 1;
        }
        if steps == 240 {
            acc = 0.0;
        }

        cam.update(false);
        render::draw_world(&world, &cam, t, None);
        next_frame().await
    }
}
```

- [ ] **Step 5: Verify build + run + tests**

Run: `cargo test` → all still pass.
Run: `cargo run` briefly → a neon-lit grid city renders, day/night drifts, windows flicker at night. Pan/zoom works.

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat: city renderer — districts, neon trim, day/night cycle"
```

---

### Task 12: Citizens + ambient vehicles renderer (`render/agents.rs`)

**Files:**
- Rewrite: `src/render/agents.rs`
- Modify: `src/render/mod.rs` (add vehicle layer call), `src/main.rs` (vehicle system + speed already wired later)

- [ ] **Step 1: Write the agent renderer**

Replace `src/render/agents.rs`:
```rust
use crate::sim::citizen::{Activity, CitizenState};
use crate::sim::rng::Rng;
use crate::sim::time;
use crate::sim::world::World;
use crate::ui::camera::Camera;
use macroquad::prelude::*;

pub fn activity_color(state: &CitizenState) -> Color {
    let act = match state {
        CitizenState::Traveling { activity, .. } => Some(*activity),
        CitizenState::Performing { activity, .. } => Some(*activity),
        CitizenState::Idle { .. } => None,
    };
    match act {
        Some(Activity::Sleep) => Color::new(0.35, 0.55, 1.0, 1.0),
        Some(Activity::Eat) => Color::new(1.0, 0.62, 0.2, 1.0),
        Some(Activity::Work) => Color::new(1.0, 0.85, 0.3, 1.0),
        Some(Activity::Fun) => Color::new(1.0, 0.3, 0.85, 1.0),
        Some(Activity::Shower) => Color::new(0.4, 0.9, 1.0, 1.0),
        Some(Activity::Stroll) | None => Color::new(0.85, 0.95, 1.0, 1.0),
    }
}

pub fn draw_citizens(world: &World, cam: &Camera, t: f32) {
    for c in &world.citizens {
        // citizens inside buildings aren't drawn
        if matches!(c.state, CitizenState::Performing { .. }) {
            continue;
        }
        let (sx, sy) = cam.to_screen(c.pos.0, c.pos.1);
        if sx < -40.0 || sy < -40.0 || sx > screen_width() + 40.0 || sy > screen_height() + 40.0 {
            continue;
        }
        let col = activity_color(&c.state);
        let bob = (t * 9.0 + c.id as f32 * 1.7).sin() * cam.ppt * 0.03;
        let r = cam.ppt * 0.16;

        // motion streak behind walkers
        if let Some(&(nx, ny)) = c.path.front() {
            let (dx, dy) = (nx as f32 + 0.5 - c.pos.0, ny as f32 + 0.5 - c.pos.1);
            let d = (dx * dx + dy * dy).sqrt().max(0.001);
            for i in 1..=3 {
                let f = i as f32 / 3.0;
                let (tx, ty) = cam.to_screen(c.pos.0 - dx / d * f * 0.45, c.pos.1 - dy / d * f * 0.45);
                draw_circle(tx, ty, r * (1.0 - f * 0.6), Color::new(col.r, col.g, col.b, 0.18 * (1.0 - f)));
            }
        }

        draw_circle(sx, sy + bob, r * 2.1, Color::new(col.r, col.g, col.b, 0.16)); // glow
        draw_circle(sx, sy + bob, r, col);
        draw_circle(sx, sy + bob - r * 0.55, r * 0.45, Color::new(1.0, 1.0, 1.0, 0.9)); // head
    }
}

// ---- ambient vehicles (visual flavor only; lives outside the sim) ----

pub struct Vehicle {
    pos: (f32, f32),
    dir: (f32, f32),
    speed: f32,
    color: Color,
}

pub struct Traffic {
    pub vehicles: Vec<Vehicle>,
}

const CAR_COLORS: [Color; 4] = [
    Color::new(0.2, 0.9, 1.0, 1.0),
    Color::new(1.0, 0.3, 0.8, 1.0),
    Color::new(0.95, 0.75, 0.3, 1.0),
    Color::new(0.6, 1.0, 0.5, 1.0),
];

impl Traffic {
    pub fn new(city_w: i32, city_h: i32, seed: u64) -> Traffic {
        let mut rng = Rng::new(seed ^ 0xCAB5);
        let mut vehicles = vec![];
        for _ in 0..16 {
            let along_x = rng.chance(0.5);
            let lane = rng.gen_range(0, 8) * crate::sim::city::BLOCK;
            let sign = if rng.chance(0.5) { 1.0 } else { -1.0 };
            let off = 0.5 + sign * 0.22; // drive on the right
            let (pos, dir) = if along_x {
                ((rng.gen_f32() * city_w as f32, lane as f32 + off), (sign, 0.0))
            } else {
                ((lane as f32 + off, rng.gen_f32() * city_h as f32), (0.0, sign))
            };
            vehicles.push(Vehicle {
                pos,
                dir,
                speed: rng.gen_f32_range(2.5, 5.0),
                color: CAR_COLORS[rng.gen_range(0, 4) as usize],
            });
        }
        Traffic { vehicles }
    }

    pub fn update(&mut self, dt: f32, city_w: i32, city_h: i32) {
        for v in &mut self.vehicles {
            v.pos.0 += v.dir.0 * v.speed * dt;
            v.pos.1 += v.dir.1 * v.speed * dt;
            if v.pos.0 < -1.0 { v.pos.0 = city_w as f32 + 1.0 }
            if v.pos.0 > city_w as f32 + 1.0 { v.pos.0 = -1.0 }
            if v.pos.1 < -1.0 { v.pos.1 = city_h as f32 + 1.0 }
            if v.pos.1 > city_h as f32 + 1.0 { v.pos.1 = -1.0 }
        }
    }

    pub fn draw(&self, cam: &Camera, tick: u64) {
        let night = time::is_night(tick);
        for v in &self.vehicles {
            let (sx, sy) = cam.to_screen(v.pos.0, v.pos.1);
            let (l, w) = (cam.ppt * 0.5, cam.ppt * 0.26);
            let horizontal = v.dir.0.abs() > 0.0;
            let (rw, rh) = if horizontal { (l, w) } else { (w, l) };
            draw_circle(sx, sy, cam.ppt * 0.4, Color::new(v.color.r, v.color.g, v.color.b, 0.10));
            draw_rectangle(sx - rw / 2.0, sy - rh / 2.0, rw, rh, v.color);
            if night {
                let (hx, hy) = (sx + v.dir.0 * l * 0.6, sy + v.dir.1 * l * 0.6);
                draw_circle(hx, hy, cam.ppt * 0.09, Color::new(1.0, 1.0, 0.9, 0.9));
            }
        }
    }
}
```

- [ ] **Step 2: Wire traffic into main.rs**

In `src/main.rs`, after creating `cam`:
```rust
    let mut traffic = render::agents::Traffic::new(sim::city::CITY_W, sim::city::CITY_H, seed);
```
In the loop, after sim steps (so it pauses with the sim):
```rust
        traffic.update(get_frame_time() * speed as f32, sim::city::CITY_W, sim::city::CITY_H);
```
In `src/render/mod.rs`, change `draw_world`'s signature and end:
```rust
pub fn draw_world(world: &World, cam: &Camera, t: f32, selected_building: Option<u16>, traffic: &agents::Traffic) {
    // ... existing body ...
    buildings::draw_neon(world, cam, t, amb);
    traffic.draw(cam, world.tick);
    agents::draw_citizens(world, cam, t);
}
```
And the call in main.rs: `render::draw_world(&world, &cam, t, None, &traffic);`

- [ ] **Step 3: Verify**

Run: `cargo test` → pass. `cargo run` → citizens walk between buildings with glow trails; hover vehicles cruise the avenues with headlights at night.

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "feat: citizen + ambient traffic rendering with glow"
```

---

### Task 13: HUD — clock, speed controls (`ui/hud.rs`)

**Files:**
- Create: `src/ui/hud.rs`
- Modify: `src/ui/mod.rs` → add `pub mod hud;`
- Modify: `src/main.rs` (speed state + HUD wiring)

- [ ] **Step 1: Write the HUD**

`src/ui/hud.rs`:
```rust
use crate::sim::world::World;
use macroquad::prelude::*;

pub const CYAN: Color = Color::new(0.2, 0.9, 1.0, 1.0);
pub const PANEL: Color = Color::new(0.03, 0.05, 0.1, 0.88);
pub const PANEL_EDGE: Color = Color::new(0.2, 0.9, 1.0, 0.35);

pub struct HudState {
    pub speed: u32,
    pub paused: bool,
    /// True when the pointer is over any UI element this frame.
    pub pointer_over_ui: bool,
}

impl HudState {
    pub fn new() -> HudState {
        HudState { speed: 1, paused: false, pointer_over_ui: false }
    }
}

fn over(x: f32, y: f32, w: f32, h: f32) -> bool {
    let (mx, my) = mouse_position();
    mx >= x && mx <= x + w && my >= y && my <= y + h
}

/// Immediate-mode button. Returns true on click.
pub fn button(x: f32, y: f32, w: f32, h: f32, label: &str, active: bool, ui_hit: &mut bool) -> bool {
    let hover = over(x, y, w, h);
    if hover {
        *ui_hit = true;
    }
    let bg = if active {
        Color::new(0.16, 0.5, 0.6, 0.95)
    } else if hover {
        Color::new(0.1, 0.2, 0.32, 0.95)
    } else {
        Color::new(0.05, 0.09, 0.16, 0.9)
    };
    draw_rectangle(x, y, w, h, bg);
    draw_rectangle_lines(x, y, w, h, 1.5, if active { CYAN } else { PANEL_EDGE });
    let dim = measure_text(label, None, 18, 1.0);
    draw_text(label, x + (w - dim.width) / 2.0, y + h / 2.0 + 6.0, 18.0, if active { WHITE } else { CYAN });
    hover && is_mouse_button_pressed(MouseButton::Left)
}

/// Draws the HUD; updates speed/pause from clicks and keys.
pub fn draw_hud(world: &World, hud: &mut HudState) {
    hud.pointer_over_ui = false;

    // top bar
    let bar_h = 52.0;
    draw_rectangle(0.0, 0.0, screen_width(), bar_h, PANEL);
    draw_line(0.0, bar_h, screen_width(), bar_h, 1.5, PANEL_EDGE);
    if over(0.0, 0.0, screen_width(), bar_h) {
        hud.pointer_over_ui = true;
    }

    draw_text("NEON CITY", 18.0, 33.0, 30.0, CYAN);
    draw_text("// 2161", 168.0, 33.0, 20.0, Color::new(1.0, 0.3, 0.85, 0.9));

    let h = world.hour_f();
    let clock = format!("DAY {}  {:02}:{:02}", world.day(), h as u32, ((h % 1.0) * 60.0) as u32);
    draw_text(&clock, 290.0, 33.0, 24.0, WHITE);

    // speed buttons
    let bx = 470.0;
    let mut ui_hit = hud.pointer_over_ui;
    if button(bx, 10.0, 48.0, 32.0, "||", hud.paused, &mut ui_hit) {
        hud.paused = !hud.paused;
    }
    for (i, (label, s)) in [("1x", 1u32), ("4x", 4), ("16x", 16)].iter().enumerate() {
        if button(bx + 56.0 + i as f32 * 56.0, 10.0, 48.0, 32.0, label, !hud.paused && hud.speed == *s, &mut ui_hit) {
            hud.speed = *s;
            hud.paused = false;
        }
    }
    hud.pointer_over_ui = ui_hit;

    // keyboard shortcuts
    if is_key_pressed(KeyCode::Space) {
        hud.paused = !hud.paused;
    }
    if is_key_pressed(KeyCode::Key1) { hud.speed = 1; hud.paused = false; }
    if is_key_pressed(KeyCode::Key2) { hud.speed = 4; hud.paused = false; }
    if is_key_pressed(KeyCode::Key3) { hud.speed = 16; hud.paused = false; }

    // population strip, bottom-left
    let employed = world.citizens.iter().filter(|c| c.job.is_some()).count();
    let info = format!("POP {}   EMPLOYED {}   SEED {}", world.citizens.len(), employed, world.seed);
    draw_text(&info, 18.0, screen_height() - 14.0, 18.0, Color::new(0.6, 0.75, 0.9, 0.8));
}
```

- [ ] **Step 2: Wire into main.rs**

In `src/main.rs`: replace `let speed: u32 = 1;` with `let mut hud = ui::hud::HudState::new();`
Sim stepping becomes:
```rust
        if !hud.paused {
            acc += get_frame_time() * hud.speed as f32;
        }
        let mut steps = 0;
        while acc >= TICK_DT && steps < 240 {
            world.tick();
            acc -= TICK_DT;
            steps += 1;
        }
        if steps == 240 {
            acc = 0.0;
        }
        let traffic_dt = if hud.paused { 0.0 } else { get_frame_time() * hud.speed.min(4) as f32 };
        traffic.update(traffic_dt, sim::city::CITY_W, sim::city::CITY_H);

        cam.update(hud.pointer_over_ui);
        render::draw_world(&world, &cam, t, None, &traffic);
        ui::hud::draw_hud(&world, &mut hud);
```
(Note `cam.update` now receives last frame's `pointer_over_ui` — acceptable one-frame lag, standard immediate-mode pattern.)

- [ ] **Step 3: Verify**

Run: `cargo test` → pass. `cargo run` → clock advances, pause/1×/4×/16× buttons and Space/1/2/3 keys work; camera doesn't pan when clicking buttons.

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "feat: HUD with clock, pause and speed controls"
```

---

### Task 14: Inspector — click to inspect citizens & buildings (`ui/inspector.rs`)

**Files:**
- Create: `src/ui/inspector.rs`
- Modify: `src/ui/mod.rs` → add `pub mod inspector;`
- Modify: `src/main.rs`

- [ ] **Step 1: Write the inspector**

`src/ui/inspector.rs`:
```rust
use crate::render::agents::activity_color;
use crate::sim::citizen::{CitizenState, NEED_KINDS};
use crate::sim::city::Tile;
use crate::sim::world::World;
use crate::ui::camera::Camera;
use crate::ui::hud::{button, HudState, CYAN, PANEL, PANEL_EDGE};
use macroquad::prelude::*;

#[derive(Clone, Copy, PartialEq)]
pub enum Selection {
    None,
    Citizen(usize),
    Building(u16),
}

pub struct Inspector {
    pub selection: Selection,
    pub follow: bool,
}

impl Inspector {
    pub fn new() -> Inspector {
        Inspector { selection: Selection::None, follow: false }
    }

    /// Click-to-select. Call after camera update; respects drags and UI hover.
    pub fn handle_click(&mut self, world: &World, cam: &Camera, hud: &HudState) {
        if hud.pointer_over_ui || cam.dragged || !is_mouse_button_released(MouseButton::Left) {
            return;
        }
        let (mx, my) = mouse_position();
        let (wx, wy) = cam.to_world(mx, my);

        // nearest visible citizen within 0.7 tiles
        let mut best: Option<(f32, usize)> = None;
        for c in &world.citizens {
            if matches!(c.state, CitizenState::Performing { .. }) {
                continue;
            }
            let d2 = (c.pos.0 - wx).powi(2) + (c.pos.1 - wy).powi(2);
            if d2 < 0.49 && best.map_or(true, |(bd, _)| d2 < bd) {
                best = Some((d2, c.id));
            }
        }
        if let Some((_, id)) = best {
            self.selection = Selection::Citizen(id);
            self.follow = false;
            return;
        }
        if let Tile::Building(id) = world.city.tile(wx as i32, wy as i32) {
            self.selection = Selection::Building(id);
            self.follow = false;
            return;
        }
        self.selection = Selection::None;
        self.follow = false;
    }

    pub fn draw(&mut self, world: &World, cam: &mut Camera, hud: &mut HudState) {
        match self.selection {
            Selection::None => {}
            Selection::Citizen(id) => self.draw_citizen_panel(world, cam, hud, id),
            Selection::Building(id) => self.draw_building_panel(world, hud, id),
        }
    }

    fn panel_rect(&self) -> (f32, f32, f32, f32) {
        let w = 300.0;
        (screen_width() - w - 14.0, 66.0, w, 330.0)
    }

    fn draw_citizen_panel(&mut self, world: &World, cam: &mut Camera, hud: &mut HudState, id: usize) {
        let c = &world.citizens[id];
        let (x, y, w, h) = self.panel_rect();
        if mouse_position().0 >= x && mouse_position().1 <= y + h {
            hud.pointer_over_ui = true;
        }
        draw_rectangle(x, y, w, h, PANEL);
        draw_rectangle_lines(x, y, w, h, 1.5, PANEL_EDGE);

        draw_text(&c.name, x + 14.0, y + 30.0, 26.0, WHITE);
        draw_text(c.personality.archetype, x + 14.0, y + 52.0, 18.0, Color::new(1.0, 0.3, 0.85, 0.9));
        draw_text(&format!("₢ {:.0}", c.money), x + w - 80.0, y + 30.0, 22.0, Color::new(0.95, 0.85, 0.3, 1.0));

        // need bars
        let mut by = y + 78.0;
        for k in NEED_KINDS {
            let v = c.needs.get(k);
            draw_text(k.label(), x + 14.0, by + 12.0, 15.0, Color::new(0.6, 0.75, 0.9, 0.9));
            let (bx, bw, bh) = (x + 90.0, w - 110.0, 12.0);
            draw_rectangle(bx, by, bw, bh, Color::new(0.08, 0.1, 0.16, 1.0));
            let fill = Color::new(1.0 - v * 0.8, 0.2 + v * 0.7, 0.35, 1.0);
            draw_rectangle(bx, by, bw * v, bh, fill);
            draw_rectangle_lines(bx, by, bw, bh, 1.0, PANEL_EDGE);
            by += 26.0;
        }

        // job + state
        by += 8.0;
        let job = match &c.job {
            Some(j) => format!(
                "{}  {:02}:00–{:02}:00  ₢{:.0}/h",
                world.city.buildings[j.workplace as usize].kind.name(),
                j.shift_start, j.shift_end, j.wage_per_hour
            ),
            None => "Unemployed".to_string(),
        };
        draw_text("JOB", x + 14.0, by + 12.0, 15.0, Color::new(0.6, 0.75, 0.9, 0.9));
        draw_text(&job, x + 90.0, by + 12.0, 15.0, WHITE);
        by += 26.0;

        let state = match &c.state {
            CitizenState::Idle { .. } => "Idle — deciding".to_string(),
            CitizenState::Traveling { to, activity } => match to {
                Some(b) => format!("→ {} ({})", world.city.buildings[*b as usize].kind.name(), activity.label()),
                None => "Strolling".to_string(),
            },
            CitizenState::Performing { at, activity } => {
                format!("{} @ {}", activity.label(), world.city.buildings[*at as usize].kind.name())
            }
        };
        draw_text("NOW", x + 14.0, by + 12.0, 15.0, Color::new(0.6, 0.75, 0.9, 0.9));
        draw_text(&state, x + 90.0, by + 12.0, 15.0, activity_color(&c.state));
        by += 34.0;

        let mut ui_hit = hud.pointer_over_ui;
        if button(x + 14.0, by, 110.0, 30.0, if self.follow { "FOLLOWING" } else { "FOLLOW" }, self.follow, &mut ui_hit) {
            self.follow = !self.follow;
        }
        hud.pointer_over_ui = ui_hit;

        if self.follow {
            cam.center = c.pos;
        }

        // marker ring in-world
        let (sx, sy) = cam.to_screen(c.pos.0, c.pos.1);
        draw_circle_lines(sx, sy, cam.ppt * 0.34, 2.0, CYAN);
    }

    fn draw_building_panel(&mut self, world: &World, hud: &mut HudState, id: u16) {
        let b = &world.city.buildings[id as usize];
        let (x, y, w, h) = self.panel_rect();
        if mouse_position().0 >= x && mouse_position().1 <= y + h {
            hud.pointer_over_ui = true;
        }
        draw_rectangle(x, y, w, h, PANEL);
        draw_rectangle_lines(x, y, w, h, 1.5, PANEL_EDGE);

        draw_text(b.kind.name(), x + 14.0, y + 30.0, 26.0, crate::render::buildings::trim_color(b.kind));
        draw_text(&format!("#{:03}", b.id), x + w - 60.0, y + 30.0, 18.0, Color::new(0.6, 0.75, 0.9, 0.8));

        let mut by = y + 64.0;
        let mut line = |label: &str, value: String, by: &mut f32| {
            draw_text(label, x + 14.0, *by, 15.0, Color::new(0.6, 0.75, 0.9, 0.9));
            draw_text(&value, x + 110.0, *by, 15.0, WHITE);
            *by += 24.0;
        };
        if b.kind.is_food() {
            line("STOCK", format!("{:.0} meals", b.stock), &mut by);
            line("PRICE", format!("₢ {:.0}", crate::sim::economy::meal_price(b.kind)), &mut by);
        }
        if b.kind.is_workplace() {
            line("WORKERS", format!("{}", b.workers.len()), &mut by);
        }
        line("INSIDE", format!("{}", b.occupants.len()), &mut by);

        by += 6.0;
        for &cid in b.occupants.iter().take(8) {
            draw_text(&format!("· {}", world.citizens[cid].name), x + 14.0, by, 15.0, Color::new(0.8, 0.9, 1.0, 0.85));
            by += 20.0;
        }
    }
}
```

- [ ] **Step 2: Wire into main.rs**

Add `let mut inspector = ui::inspector::Inspector::new();` after `hud`.
Replace render/UI section of the loop:
```rust
        cam.update(hud.pointer_over_ui);
        inspector.handle_click(&world, &cam, &hud);
        let sel_building = match inspector.selection {
            ui::inspector::Selection::Building(b) => Some(b),
            _ => None,
        };
        render::draw_world(&world, &cam, t, sel_building, &traffic);
        ui::hud::draw_hud(&world, &mut hud);
        inspector.draw(&world, &mut cam, &mut hud);
```

- [ ] **Step 3: Verify**

Run: `cargo test` → pass. `cargo run` → click a walking citizen: live needs panel; FOLLOW tracks them; click a noodle bar: stock/price/occupants. Click empty road: deselect.

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "feat: inspector panels for citizens and buildings"
```

---

### Task 15: WASM build + website shell

**Files:**
- Create: `web/index.html`, `build_web.sh`
- Download: `web/mq_js_bundle.js` (vendored macroquad JS loader)

- [ ] **Step 1: Add the wasm target**

Run: `rustup target add wasm32-unknown-unknown`

- [ ] **Step 2: Vendor the macroquad JS loader**

Run: `curl -fsSL https://raw.githubusercontent.com/not-fl3/macroquad/master/js/mq_js_bundle.js -o web/mq_js_bundle.js`

- [ ] **Step 3: Create the page**

`web/index.html`:
```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>NEON CITY // 2161</title>
  <style>
    html, body { margin: 0; padding: 0; height: 100%; background: #05070f; overflow: hidden; }
    #glcanvas { width: 100vw; height: 100vh; display: block; outline: none; }
    #loading {
      position: absolute; inset: 0; display: flex; flex-direction: column;
      align-items: center; justify-content: center; color: #35e0ff;
      font-family: ui-monospace, monospace; letter-spacing: 0.4em; pointer-events: none;
      text-shadow: 0 0 18px #35e0ff;
    }
    #loading small { color: #ff3df0; letter-spacing: 0.2em; margin-top: 1em; text-shadow: 0 0 14px #ff3df0; }
  </style>
</head>
<body>
  <div id="loading"><div>NEON CITY</div><small>establishing uplink…</small></div>
  <canvas id="glcanvas" tabindex="1"></canvas>
  <script src="mq_js_bundle.js"></script>
  <script>
    load("neon_city.wasm");
    const hide = () => { const el = document.getElementById("loading"); if (el) el.remove(); };
    setTimeout(hide, 2500);
  </script>
</body>
</html>
```

- [ ] **Step 4: Create the build script**

`build_web.sh`:
```bash
#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
cargo build --release --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/neon_city.wasm web/
echo "Built. Serve with:  python3 -m http.server 8080 -d web"
```
Run: `chmod +x build_web.sh`

- [ ] **Step 5: Build and smoke-test the website**

Run: `./build_web.sh`
Expected: compiles to wasm, copies into `web/`.
Run: `python3 -m http.server 8080 -d web` (background) then `curl -sI http://localhost:8080/ | head -1` and `curl -sI http://localhost:8080/neon_city.wasm | head -1`
Expected: both `HTTP/1.0 200 OK`. Open `http://localhost:8080` in a browser to confirm the city runs.

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat: wasm build + neon website shell"
```

---

### Task 16: README + final verification

**Files:**
- Create: `README.md`

- [ ] **Step 1: Write the README**

`README.md`:
```markdown
# NEON CITY // 2161

A self-running sci-fi city simulation written entirely in Rust and compiled to
WebAssembly. AI citizens manage hunger, energy, hygiene, fun and money in a
procedurally generated neon city with jobs, food production, and a day/night
cycle.

## Run it

**Native (development):**
    cargo run

**Website (WASM):**
    ./build_web.sh
    python3 -m http.server 8080 -d web
    # open http://localhost:8080

## Controls

- **Drag** to pan, **scroll** to zoom
- **Click** a citizen or building to inspect it; FOLLOW tracks a citizen
- **Space** pause · **1/2/3** = 1×/4×/16× speed

## How it works

- `src/sim/` — pure deterministic simulation (no rendering imports): procedural
  city, A* pathfinding, utility-based AI (needs scream louder as they empty;
  personalities re-weight them), wages, food production chain.
- `src/render/` — procedural neon visuals; swap this layer for sprites without
  touching the sim.
- `src/ui/` — camera, HUD, inspector.

Tests: `cargo test` (28 tests over the sim core).
```

- [ ] **Step 2: Full verification sweep**

Run: `cargo test` → all pass.
Run: `cargo build --release --target wasm32-unknown-unknown` → clean.
Run: `cargo run` → observe ≥1 full day cycle at 16×: citizens commute at 08:00, eat at venues, sleep at night; lights/neon respond to time of day.

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "docs: README with run instructions"
```

---

## Self-review notes

- **Spec coverage:** needs+money (T6/T9), utility AI + personalities (T8), jobs/wages/shifts (T9), food production chain (T7), procedural zoned city (T4), A* (T5), day/night (T3/T11), neon look (T11/T12), inspect+follow (T14), speed controls (T13), WASM website (T15), determinism + tests (T2/T9). Out-of-scope items from spec remain out.
- **Type consistency check:** `Citizen.path: VecDeque<(i32,i32)>` matches `find_path` return converted via `VecDeque::from`; `choose_action(c, city, hour: u32, is_night: bool)` matches call in `tick_citizen`; `draw_world(world, cam, t, Option<u16>, &Traffic)` matches main.rs call after T12 amendment; `button(...)` shared by hud and inspector.
- **Known simplifications (deliberate):** citizens inside buildings are hidden (occupancy visible in inspector); traffic is ambient-only (not sim state); economy is source/sink rather than fully closed. All documented in code comments or README.
