# Delivery Trucks & Supply Chain — Implementation Plan (Roadmap Phase 3)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the instant hourly food *push* (`distribute_food`) with physical per-farm trucks that carry meals over the road grid, paid on delivery at a dynamic supply/demand price, and let chronically-starved venues close.

**Architecture:** A new `src/sim/logistics.rs` holds the `Truck`/`TruckState` types and the pure helpers (neediest-venue, load math); `world.rs` runs a per-tick logistics step (produce → dispatch → drive → deliver) after the citizen loop, mirroring `tick_citizen`. Trucks are sim agents (`pos`/`path`/`speed`), one per Hydro Farm; the driver is whichever farm worker is on-shift at dispatch time, pulled into a new `CitizenState::Driving` whose position follows the truck. Dynamic wholesale price lives in `economy.rs`. All sim logic is deterministic and money-conserving; rendering is untested by convention.

**Tech Stack:** Rust, macroquad. No new dependencies.

**Spec:** `docs/superpowers/specs/2026-06-16-delivery-trucks-design.md` — read it first.

**Conventions:**
- Branch: `living-world-phase-3` (already created, off `living-world-phase-2`). Phase 2's `balance`/`insolvent`/`stock`/`minted` are dependencies.
- NO rustfmt — match the surrounding compact style by hand; never run `cargo fmt`.
- `cargo build` may show transient `dead_code` warnings for types/consts added before their consumer task lands; leave them alone.
- Baseline test count: **63**. Per-task expected counts are noted (some tasks remove obsolete `distribute_food` tests).
- TICKS_PER_HOUR = 600; TICKS_PER_DAY = 14400. Citizen speed ≈ 0.05 tiles/tick.

---

### Task 1: Dynamic pricing + logistics constants (economy.rs)

Rename `WHOLESALE_PRICE` → `WHOLESALE_BASE` (it becomes the base, not the price), add the logistics constants, and add the pure `wholesale_price` function. `distribute_food` stays for now (deleted in Task 5) and keeps working at the base price.

**Files:**
- Modify: `src/sim/economy.rs`

- [ ] **Step 1: Write the failing tests** — append to the `tests` module in `src/sim/economy.rs`:

```rust
    #[test]
    fn wholesale_price_floors_and_ceils() {
        // Glut: huge supply, no demand -> floor.
        let lo = wholesale_price(1000.0, 0.0);
        assert!((lo - WHOLESALE_BASE * PRICE_LO_MULT).abs() < 1e-4, "got {lo}");
        // Famine: no supply, big demand -> ceiling.
        let hi = wholesale_price(0.0, 1000.0);
        assert!((hi - WHOLESALE_BASE * PRICE_HI_MULT).abs() < 1e-4, "got {hi}");
    }

    #[test]
    fn wholesale_price_balanced_is_near_base() {
        // demand ≈ supply -> ≈ base.
        let p = wholesale_price(100.0, 100.0);
        assert!((p - WHOLESALE_BASE).abs() < 0.2, "got {p}");
    }

    #[test]
    fn wholesale_price_rises_with_demand() {
        assert!(wholesale_price(50.0, 80.0) > wholesale_price(50.0, 40.0));
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test wholesale_price`
Expected: compile error — `wholesale_price`, `WHOLESALE_BASE`, `PRICE_LO_MULT`, `PRICE_HI_MULT` don't exist.

- [ ] **Step 3: Implement** — in `src/sim/economy.rs`:

(a) Rename the constant and add the new ones. Replace:

```rust
/// Farms charge venues this per meal at distribution time.
pub const WHOLESALE_PRICE: f32 = 7.0;
```
with:
```rust
/// Base wholesale price per meal; the dynamic price floats around it.
pub const WHOLESALE_BASE: f32 = 7.0;
/// Dynamic wholesale band: price = WHOLESALE_BASE * clamp(demand/(supply+1), LO, HI).
pub const PRICE_LO_MULT: f32 = 0.6;
pub const PRICE_HI_MULT: f32 = 1.6;
/// Farms hold inventory up to this (larger than the venue STOCK_CAP).
pub const FARM_STOCK_CAP: f32 = 120.0;
/// A venue is an open purchase order when stock drops below this.
pub const ORDER_THRESHOLD: f32 = 20.0;
/// Meals a truck carries per run.
pub const TRUCK_CAPACITY: f32 = 30.0;
/// Truck travel speed, tiles/tick (~2.4× a citizen).
pub const TRUCK_SPEED: f32 = 0.12;
/// Consecutive broke hours before a food venue closes for good.
pub const CLOSURE_GRACE_HOURS: u32 = 24;
```

(b) Add the pricing function after `wage_range` (before `distribute_food`):

```rust
/// Dynamic wholesale spot price from current city-wide supply and demand.
/// supply = meals waiting on farms; demand = unmet venue room. Bounded so a
/// glut floors the price and a famine ceilings it; the `+1` avoids a blow-up
/// at zero supply.
pub fn wholesale_price(supply: f32, demand: f32) -> f32 {
    WHOLESALE_BASE * (demand / (supply + 1.0)).clamp(PRICE_LO_MULT, PRICE_HI_MULT)
}
```

(c) Update the two existing references to the old name. In `distribute_food`, replace both `WHOLESALE_PRICE` occurrences with `WHOLESALE_BASE`. In the test `retail_covers_wholesale`, replace `WHOLESALE_PRICE` with `WHOLESALE_BASE` (two occurrences: the `assert!` and the message). In the test `distribution_charges_venues_and_pays_farms`, replace the `WHOLESALE_PRICE` occurrence with `WHOLESALE_BASE`.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test`
Expected: 66 passed (was 63; +3 pricing tests). `cargo build` — clean aside from transient `dead_code` on the new unused consts.

- [ ] **Step 5: Commit**

```bash
git add src/sim/economy.rs
git commit -m "feat: dynamic wholesale price + logistics constants"
```

---

### Task 2: logistics module — types + pure helpers

Create the module with the `Truck`/`TruckState` types and the two pure dispatch helpers. No world wiring yet.

**Files:**
- Create: `src/sim/logistics.rs`
- Modify: `src/sim/mod.rs`

- [ ] **Step 1: Create the module with types and helpers** — write `src/sim/logistics.rs`:

```rust
use crate::sim::city::{BuildingKind, City};
use crate::sim::economy;
use std::collections::VecDeque;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TruckState {
    /// Parked at the home farm, available to dispatch.
    Parked,
    /// Loaded, driving to a venue's door.
    Outbound { venue: u16 },
    /// Driving back to the home farm's door.
    Returning,
}

pub struct Truck {
    pub id: usize,
    pub home_farm: u16,
    /// Citizen currently driving; Some only between dispatch and park.
    pub driver: Option<usize>,
    pub pos: (f32, f32),
    pub path: VecDeque<(i32, i32)>,
    pub speed: f32,
    pub cargo: f32,
    pub state: TruckState,
}

/// City-wide (supply, demand) for the dynamic price: supply = meals on farms,
/// demand = unmet room across open food venues.
pub fn supply_demand(city: &City) -> (f32, f32) {
    let supply = city
        .buildings
        .iter()
        .filter(|b| b.kind == BuildingKind::HydroFarm)
        .map(|b| b.stock)
        .sum();
    let demand = city
        .buildings
        .iter()
        .filter(|b| b.kind.is_food() && b.open())
        .map(|b| (economy::STOCK_CAP - b.stock).max(0.0))
        .sum();
    (supply, demand)
}

/// Lowest-stock open food venue below the order threshold that no truck is
/// already serving (`served` = venues currently Outbound-targeted). Ties by id.
pub fn neediest_venue(city: &City, served: &[u16]) -> Option<u16> {
    city.buildings
        .iter()
        .filter(|b| b.kind.is_food() && b.open() && b.stock < economy::ORDER_THRESHOLD)
        .filter(|b| !served.contains(&b.id))
        .min_by(|a, b| a.stock.partial_cmp(&b.stock).unwrap().then(a.id.cmp(&b.id)))
        .map(|b| b.id)
}

/// Meals to load: the min of truck capacity, farm inventory, and venue room.
pub fn load_amount(capacity: f32, farm_stock: f32, venue_room: f32) -> f32 {
    capacity.min(farm_stock).min(venue_room).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::city::City;
    use crate::sim::rng::Rng;

    #[test]
    fn neediest_picks_lowest_open_unserved() {
        let mut city = City::generate(&mut Rng::new(2));
        let food: Vec<u16> =
            city.buildings.iter().filter(|b| b.kind.is_food()).map(|b| b.id).collect();
        assert!(food.len() >= 3, "need ≥3 food venues for this test");
        for &id in &food {
            city.buildings[id as usize].stock = economy::STOCK_CAP; // full -> not needy
        }
        city.buildings[food[1] as usize].stock = 5.0; // neediest
        city.buildings[food[2] as usize].stock = 10.0; // needy but higher
        // food[1] is neediest; if it's already served, food[2] wins.
        assert_eq!(neediest_venue(&city, &[]), Some(food[1]));
        assert_eq!(neediest_venue(&city, &[food[1]]), Some(food[2]));
    }

    #[test]
    fn neediest_skips_full_venues() {
        let mut city = City::generate(&mut Rng::new(2));
        for b in city.buildings.iter_mut().filter(|b| b.kind.is_food()) {
            b.stock = economy::STOCK_CAP;
        }
        assert_eq!(neediest_venue(&city, &[]), None);
    }

    #[test]
    fn load_amount_is_the_min() {
        assert_eq!(load_amount(30.0, 100.0, 100.0), 30.0); // capacity binds
        assert_eq!(load_amount(30.0, 12.0, 100.0), 12.0); // farm stock binds
        assert_eq!(load_amount(30.0, 100.0, 8.0), 8.0); // venue room binds
        assert_eq!(load_amount(30.0, 100.0, -5.0), 0.0); // no negative loads
    }
}
```

(Note: `b.open()` is added in Task 3. This file will not compile until then, so the test run is in Task 3.)

- [ ] **Step 2: Register the module** — in `src/sim/mod.rs`, add `pub mod logistics;` in alphabetical order with the other `pub mod` lines.

- [ ] **Step 3: Commit** (build is intentionally red until Task 3 adds `open()`; commit anyway so the module exists, or fold into Task 3's commit — recommended to proceed straight to Task 3 and commit once both compile).

Skip the standalone commit; continue to Task 3, then commit Tasks 2+3 together at Task 3 Step 5.

---

### Task 3: Building closure fields + `open()` predicate (city.rs)

**Files:**
- Modify: `src/sim/city.rs`

- [ ] **Step 1: Write the failing test** — append to the `tests` module in `src/sim/city.rs`:

```rust
    #[test]
    fn buildings_start_open_with_no_broke_hours() {
        let city = City::generate(&mut Rng::new(4));
        for b in &city.buildings {
            assert!(b.open(), "{:?} starts closed", b.kind);
            assert_eq!(b.hours_broke, 0);
        }
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test buildings_start_open`
Expected: compile error — `open`, `hours_broke` don't exist.

- [ ] **Step 3: Implement** — in `src/sim/city.rs`:

(a) Add two fields to the `Building` struct, after `insolvent`:

```rust
    /// Venue has shut down (frozen, dark). Food venues only.
    pub closed: bool,
    /// Consecutive hours a venue couldn't afford one wholesale meal.
    pub hours_broke: u32,
```

(b) Add the predicate inside `impl BuildingKind`? No — it's per-building. Add an `impl Building` block right after the `Building` struct definition:

```rust
impl Building {
    /// Open for business (used by ordering, dispatch, and AI targeting).
    pub fn open(&self) -> bool {
        !self.closed
    }
}
```

(c) Initialize the fields in the `place_building` struct literal, after `insolvent: false,`:

```rust
            closed: false,
            hours_broke: 0,
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test`
Expected: 68 passed (66 + Task 2's `neediest`/`load` ... wait: Task 2's 3 tests + this 1 = +4 over 66 = 70). Expected: **70 passed**. `cargo build` — transient `dead_code` on `Truck`/`TruckState`/helpers until Task 4–5.

- [ ] **Step 5: Commit** (Tasks 2 + 3 together)

```bash
git add src/sim/logistics.rs src/sim/mod.rs src/sim/city.rs
git commit -m "feat: logistics types + pure helpers; venue closure fields"
```

---

### Task 4: Trucks on World + `CitizenState::Driving` plumbing (no behavior yet)

Add the `Driving` variant and the `trucks` field, create one parked truck per farm at world init, and satisfy every exhaustive match. No dispatch logic yet, so no truck ever moves — the new arms are inert but compile, and determinism still holds.

**Files:**
- Modify: `src/sim/citizen.rs` (CitizenState)
- Modify: `src/sim/world.rs` (struct, `new`, `tick_citizen` arm, fingerprint)
- Modify: `src/render/agents.rs` (`activity_color`, `draw_citizens`)
- Modify: `src/ui/inspector.rs` (citizen state line)

- [ ] **Step 1: Write the failing test** — append to the `tests` module in `src/sim/world.rs`:

```rust
    #[test]
    fn one_parked_truck_per_farm_at_init() {
        let w = World::new(2161, 48);
        let farms = w
            .city
            .buildings
            .iter()
            .filter(|b| b.kind == BuildingKind::HydroFarm)
            .count();
        assert_eq!(w.trucks.len(), farms, "expected one truck per farm");
        for t in &w.trucks {
            assert!(matches!(t.state, crate::sim::logistics::TruckState::Parked));
            assert!(t.driver.is_none());
            assert_eq!(t.cargo, 0.0);
            assert_eq!(w.city.buildings[t.home_farm as usize].kind, BuildingKind::HydroFarm);
        }
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test one_parked_truck_per_farm`
Expected: compile error — `w.trucks` doesn't exist.

- [ ] **Step 3: Implement**

(a) In `src/sim/citizen.rs`, add a variant to `CitizenState` (after `Performing`):

```rust
    /// Out driving a delivery truck (logistics owns the position).
    Driving { truck: usize },
```

(b) In `src/sim/world.rs`:

Add the import at the top, next to the other `use crate::sim::...` lines:
```rust
use crate::sim::logistics::{self, Truck, TruckState};
```

Add the field to `World` (after `minted`):
```rust
    /// Per-farm delivery trucks (one each, created at init).
    pub trucks: Vec<Truck>,
```

Initialize it in `World::new`. Change the struct literal to add `trucks: vec![]`:
```rust
        let mut world = World { rng, city, citizens: vec![], tick: 0, seed, events: VecDeque::new(), wages_today: 0.0, minted: 0.0, trucks: vec![] };
```
Then, just before `world` is returned at the end of `new` (after the citizen loop), build the trucks:
```rust
        let farms: Vec<u16> = world
            .city
            .buildings_of(|k| k == BuildingKind::HydroFarm)
            .map(|b| b.id)
            .collect();
        for (i, farm) in farms.into_iter().enumerate() {
            let door = world.city.buildings[farm as usize].door;
            world.trucks.push(Truck {
                id: i,
                home_farm: farm,
                driver: None,
                pos: (door.0 as f32 + 0.5, door.1 as f32 + 0.5),
                path: std::collections::VecDeque::new(),
                speed: economy::TRUCK_SPEED,
                cargo: 0.0,
                state: TruckState::Parked,
            });
        }
        world
```

Add a `Driving` arm to the `match c.state` in `tick_citizen` (after the `Performing` arm, before the closing `}` of the match). It settles wages like work but does not move — logistics moves the truck and the driver's `pos`:
```rust
        CitizenState::Driving { .. } => {
            // On the clock while driving; wages settle from the farm balance.
            let at = c.job.as_ref().map(|j| j.workplace);
            if let Some(at) = at {
                let (w, m) = settle_wage(c, city, at, tick, events);
                wages = w;
                minted = m;
            }
        }
```

Extend `fingerprint` (inside the `#[cfg(test)]` method) — after the buildings loop, before `h = mix(h, self.minted...)`, add a trucks loop and fold truck state in:
```rust
        for t in &self.trucks {
            h = mix(h, t.pos.0.to_bits() as u64);
            h = mix(h, t.pos.1.to_bits() as u64);
            h = mix(h, t.cargo.to_bits() as u64);
            h = mix(h, match t.state {
                TruckState::Parked => 0,
                TruckState::Outbound { venue } => 1_000 + venue as u64,
                TruckState::Returning => 2,
            });
            h = mix(h, t.driver.map_or(0, |d| d as u64 + 1));
        }
```
Also fold the new building fields into the existing buildings loop in `fingerprint` (after the `b.occupants.len()` line):
```rust
            h = mix(h, b.closed as u64);
            h = mix(h, b.hours_broke as u64);
```

(c) In `src/render/agents.rs`, handle `Driving` in both places.

In `activity_color`, add a match arm to the inner `match state` (after the `Idle` arm):
```rust
        CitizenState::Driving { .. } => None,
```

In `draw_citizens`, skip drivers (they render as the truck). Change the early-continue at the top of the loop:
```rust
        // citizens inside buildings or out driving aren't drawn as pedestrians
        if matches!(c.state, CitizenState::Performing { .. } | CitizenState::Driving { .. }) {
            continue;
        }
```

(d) In `src/ui/inspector.rs`, add a `Driving` arm to the state `match &c.state` in `draw_citizen_panel` (after the `Performing` arm):
```rust
                CitizenState::Driving { truck } => {
                    let farm = world.trucks[*truck].home_farm;
                    format!("Driving for {}", world.city.buildings[farm as usize].kind.name())
                }
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test`
Expected: 71 passed (70 + 1). `cargo build` — transient `dead_code` may remain on `logistics::supply_demand`/`neediest_venue`/`load_amount` until Task 5.

- [ ] **Step 5: Commit**

```bash
git add src/sim/citizen.rs src/sim/world.rs src/render/agents.rs src/ui/inspector.rs
git commit -m "feat: trucks on World + Driving citizen state (plumbing)"
```

---

### Task 5: The delivery loop — produce, dispatch, drive, deliver

Wire the logistics step into `world.tick()` (replacing `distribute_food`), implement the loop, add the `DeliveryCompleted` event, and remove the obsolete `distribute_food` and its five tests. This is the integration task: after it, food reaches venues only via trucks.

**Files:**
- Modify: `src/sim/logistics.rs` (the impure `tick` step + an `advance` helper)
- Modify: `src/sim/world.rs` (call site; drop `distribute_food`)
- Modify: `src/sim/economy.rs` (delete `distribute_food` + its 5 tests)
- Modify: `src/sim/event.rs` (`DeliveryCompleted`)
- Modify: `src/ui/ticker.rs` (format + color for `DeliveryCompleted`)

- [ ] **Step 1: Write the failing tests** — append to the `tests` module in `src/sim/world.rs`:

```rust
    #[test]
    fn truck_delivers_to_a_low_venue_and_money_is_conserved() {
        let mut w = World::new(2161, 48);
        // A farm with stock and an on-shift driver standing at it.
        let farm = w.city.buildings.iter().find(|b| b.kind == BuildingKind::HydroFarm).unwrap().id;
        let venue = w.city.buildings.iter().find(|b| b.kind.is_food()).unwrap().id;
        w.city.buildings[farm as usize].stock = 100.0;
        w.city.buildings[venue as usize].stock = 0.0; // wide-open order
        w.city.buildings[venue as usize].balance = 1000.0; // can pay
        // Put a farm worker on-site, on shift, so a driver is available.
        let driver = w.city.buildings[farm as usize].workers.first().copied()
            .unwrap_or_else(|| { w.citizens[0].job = Some(crate::sim::citizen::Job { workplace: farm, shift_start: 0, shift_end: 24, wage_per_hour: 10.0, unpaid_hours: 0 }); w.city.buildings[farm as usize].workers.push(0); 0 });
        w.citizens[driver].state = CitizenState::Performing { at: farm, activity: Activity::Work };
        w.city.buildings[farm as usize].occupants.push(driver);

        let total_before = w.total_money();
        let venue_stock_before = w.city.buildings[venue as usize].stock;
        // Run up to ~3 hours: dispatch, drive, deliver, return.
        let mut delivered = false;
        for _ in 0..(TICKS_PER_HOUR * 3) {
            w.tick();
            for ev in w.drain_events() {
                if matches!(ev.kind, EventKind::DeliveryCompleted { venue: v, .. } if v == venue) {
                    delivered = true;
                }
            }
            if delivered { break; }
        }
        assert!(delivered, "no delivery completed");
        assert!(w.city.buildings[venue as usize].stock > venue_stock_before, "venue not restocked");
        // Money only moved venue->farm: total (wallets + balances) unchanged by delivery,
        // up to whatever wages minted in these ticks.
        let drift = (w.total_money() - total_before - w.minted).abs();
        assert!(drift < 0.5, "conservation drift {drift}");
    }

    #[test]
    fn leftover_cargo_returns_to_the_farm() {
        let mut w = World::new(2161, 48);
        let farm = w.city.buildings.iter().find(|b| b.kind == BuildingKind::HydroFarm).unwrap().id;
        let venue = w.city.buildings.iter().find(|b| b.kind.is_food()).unwrap().id;
        w.city.buildings[farm as usize].stock = 100.0;
        // Venue is broke and nearly full: it can buy almost nothing, so the load returns.
        w.city.buildings[venue as usize].stock = 19.0; // below threshold (20) -> ordered
        w.city.buildings[venue as usize].balance = 0.0;
        let driver = w.city.buildings[farm as usize].workers.first().copied()
            .unwrap_or_else(|| { w.citizens[0].job = Some(crate::sim::citizen::Job { workplace: farm, shift_start: 0, shift_end: 24, wage_per_hour: 10.0, unpaid_hours: 0 }); w.city.buildings[farm as usize].workers.push(0); 0 });
        w.citizens[driver].state = CitizenState::Performing { at: farm, activity: Activity::Work };
        w.city.buildings[farm as usize].occupants.push(driver);

        let farm_meals_before = w.city.buildings[farm as usize].stock; // 100
        // Run long enough for a full round trip back to Parked.
        for _ in 0..(TICKS_PER_HOUR * 4) { w.tick(); }
        // Farm produced some during 06-22, and the unsold load came back; the farm's
        // meal total should not have permanently lost the dispatched cargo.
        let farm_now = w.city.buildings[farm as usize].stock;
        let venue_now = w.city.buildings[venue as usize].stock;
        assert!(farm_now + venue_now >= farm_meals_before - 1.0,
            "meals leaked: farm {farm_now} + venue {venue_now} < {farm_meals_before}");
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test truck_delivers_to_a_low_venue`
Expected: FAIL — no delivery ever fires (logistics not wired; `DeliveryCompleted` doesn't exist → also a compile error). Fix compile first via Step 3.

- [ ] **Step 3: Implement**

(a) In `src/sim/event.rs`, add a variant to `EventKind` (after `WorkerQuit`):
```rust
    /// A truck dropped meals at a venue.
    DeliveryCompleted { farm: u16, venue: u16, meals: u16 },
```

(b) In `src/ui/ticker.rs`, add a `format_event` arm (inside the `match ev.kind`, after `WorkerQuit`):
```rust
        EventKind::DeliveryCompleted { farm, venue, meals } => (
            format!(
                "[{stamp}] {} #{:03} delivered {meals} meals to {} #{:03}",
                world.city.buildings[farm as usize].kind.name(),
                farm,
                world.city.buildings[venue as usize].kind.name(),
                venue
            ),
            Selection::Building(venue),
        ),
```
and an `event_color` arm (after `WorkerQuit`):
```rust
        EventKind::DeliveryCompleted { .. } => Color::new(0.4, 0.9, 1.0, 1.0),
```

(c) In `src/sim/logistics.rs`, add the impure tick step and the `advance` helper (and the needed imports). At the top, extend the `use` block:
```rust
use crate::sim::citizen::{Activity, Citizen, CitizenState};
use crate::sim::event::{push_event, EventKind, SimEvent};
use crate::sim::path;
use crate::sim::time::TICKS_PER_HOUR;
```
(keep the existing `city`/`economy`/`VecDeque` imports). Then add:

```rust
/// One tick of the supply chain: hourly farm production, then per-truck
/// dispatch / drive / deliver. Runs after the citizen loop each tick.
pub fn tick(
    city: &mut City,
    trucks: &mut [Truck],
    citizens: &mut [Citizen],
    tick: u64,
    hour: u32,
    events: &mut VecDeque<SimEvent>,
) {
    // 1. Farm production (06:00–22:00), accumulating into farm stock.
    if tick % TICKS_PER_HOUR == 0 && (6..22).contains(&hour) {
        for b in city.buildings.iter_mut().filter(|b| b.kind == BuildingKind::HydroFarm) {
            b.stock = (b.stock + economy::FARM_OUTPUT_PER_HOUR).min(economy::FARM_STOCK_CAP);
        }
    }

    // Venues already being served this tick (so two trucks don't both grab one).
    let mut served: Vec<u16> = trucks
        .iter()
        .filter_map(|t| match t.state {
            TruckState::Outbound { venue } => Some(venue),
            _ => None,
        })
        .collect();

    for i in 0..trucks.len() {
        match trucks[i].state {
            TruckState::Parked => {
                let farm = trucks[i].home_farm;
                if city.buildings[farm as usize].stock <= 0.0 {
                    continue;
                }
                // Driver = lowest-id farm worker currently working at the farm.
                let driver = city.buildings[farm as usize]
                    .workers
                    .iter()
                    .copied()
                    .filter(|&d| matches!(citizens[d].state, CitizenState::Performing { at, activity: Activity::Work } if at == farm))
                    .min();
                let Some(d) = driver else { continue };
                let Some(venue) = neediest_venue(city, &served) else { continue };
                let room = economy::STOCK_CAP - city.buildings[venue as usize].stock;
                let cargo = load_amount(economy::TRUCK_CAPACITY, city.buildings[farm as usize].stock, room);
                if cargo <= 0.0 {
                    continue;
                }
                let from = city.buildings[farm as usize].door;
                let to = city.buildings[venue as usize].door;
                let Some(p) = path::find_path(city, from, to) else { continue };
                city.buildings[farm as usize].stock -= cargo;
                trucks[i].cargo = cargo;
                trucks[i].driver = Some(d);
                trucks[i].pos = (from.0 as f32 + 0.5, from.1 as f32 + 0.5);
                trucks[i].path = VecDeque::from(p);
                trucks[i].state = TruckState::Outbound { venue };
                citizens[d].state = CitizenState::Driving { truck: i };
                city.buildings[farm as usize].occupants.retain(|&o| o != d);
                served.push(venue);
            }
            TruckState::Outbound { venue } => {
                advance(&mut trucks[i]);
                if let Some(d) = trucks[i].driver {
                    citizens[d].pos = trucks[i].pos;
                }
                if trucks[i].path.is_empty() {
                    // Borrow discipline: end every `&mut city.buildings[..]` borrow before
                    // the next one (supply_demand needs `&city`; the farm write is a 2nd index).
                    let open_and_room = {
                        let v = &city.buildings[venue as usize];
                        v.open() && v.stock < economy::STOCK_CAP
                    };
                    if open_and_room {
                        let (supply, demand) = supply_demand(city);
                        let price = economy::wholesale_price(supply, demand);
                        let bought;
                        {
                            let v = &mut city.buildings[venue as usize];
                            let room = economy::STOCK_CAP - v.stock;
                            bought = trucks[i].cargo.min(v.balance / price).min(room).max(0.0);
                            v.stock += bought;
                            v.balance -= bought * price;
                        }
                        trucks[i].cargo -= bought;
                        let farm = trucks[i].home_farm;
                        city.buildings[farm as usize].balance += bought * price;
                        push_event(events, SimEvent { tick, kind: EventKind::DeliveryCompleted { farm, venue, meals: bought.round() as u16 } });
                    }
                    let farm = trucks[i].home_farm;
                    let from = city.buildings[venue as usize].door;
                    let to = city.buildings[farm as usize].door;
                    trucks[i].path = path::find_path(city, from, to).map(VecDeque::from).unwrap_or_default();
                    trucks[i].state = TruckState::Returning;
                }
            }
            TruckState::Returning => {
                advance(&mut trucks[i]);
                if let Some(d) = trucks[i].driver {
                    citizens[d].pos = trucks[i].pos;
                }
                if trucks[i].path.is_empty() {
                    let farm = trucks[i].home_farm;
                    let door;
                    {
                        let fb = &mut city.buildings[farm as usize];
                        fb.stock = (fb.stock + trucks[i].cargo).min(economy::FARM_STOCK_CAP);
                        door = fb.door;
                    }
                    trucks[i].cargo = 0.0;
                    if let Some(d) = trucks[i].driver.take() {
                        citizens[d].pos = (door.0 as f32 + 0.5, door.1 as f32 + 0.5);
                        let resume = matches!(&citizens[d].job, Some(j) if j.workplace == farm && j.in_shift(hour));
                        if resume {
                            citizens[d].state = CitizenState::Performing { at: farm, activity: Activity::Work };
                            city.buildings[farm as usize].occupants.push(d);
                        } else {
                            citizens[d].state = CitizenState::Idle { until: tick + 1 };
                        }
                    }
                    trucks[i].state = TruckState::Parked;
                    trucks[i].path.clear();
                }
            }
        }
    }
}

/// Move a truck one tick along its path (same vector math as citizen travel).
fn advance(t: &mut Truck) {
    if let Some(&(tx, ty)) = t.path.front() {
        let target = (tx as f32 + 0.5, ty as f32 + 0.5);
        let (dx, dy) = (target.0 - t.pos.0, target.1 - t.pos.1);
        let d = (dx * dx + dy * dy).sqrt();
        if d <= t.speed {
            t.pos = target;
            t.path.pop_front();
        } else {
            t.pos.0 += dx / d * t.speed;
            t.pos.1 += dy / d * t.speed;
        }
    }
}
```

(d) In `src/sim/world.rs`, replace the hourly `distribute_food` call with the logistics step at the end of `tick`. First, delete these lines from the top of `tick`:
```rust
        if self.tick % TICKS_PER_HOUR == 0 {
            economy::distribute_food(&mut self.city, hour);
        }
```
Then, in `tick`, after the citizen `for` loop and before the daily-summary `if`, add:
```rust
        logistics::tick(&mut self.city, &mut self.trucks, &mut self.citizens, tick, hour, &mut self.events);
```

(e) In `src/sim/economy.rs`, delete the entire `distribute_food` function and its doc comment, and delete the five tests that exercised it: `farms_stock_food_venues`, `no_production_at_night`, `stock_caps`, `distribution_charges_venues_and_pays_farms`, `broke_venue_gets_no_stock`. Keep `retail_covers_wholesale` and `farm_wages_below_other_wages`. (The `Rng` import in the test module may now be unused — remove it if the compiler warns.)

- [ ] **Step 4: Run to verify pass**

Run: `cargo test`
Expected: **70 passed** (71 − 5 removed economy tests − 1? recount: 71 after Task 4; remove 5 → 66; add 2 new world tests → 68). Expected: **68 passed**.
Run: `cargo build` — clean (logistics helpers now consumed; no `dead_code`).
Also run the integration regressions and confirm green: `cargo test full_day_cycle_behavior three_day_money_conservation_soak`. If `full_day_cycle_behavior` fails with "ran completely out of food," see the tuning note in Task 7 — but with `FARM_OUTPUT_PER_HOUR=6` × 2 farms × 16h ≈ 192 meals/day produced vs ~72 eaten and `TRUCK_CAPACITY=30`, it should pass at the shipped seed. The soak's comment is updated in Task 7.

- [ ] **Step 5: Commit**

```bash
git add src/sim/logistics.rs src/sim/world.rs src/sim/economy.rs src/sim/event.rs src/ui/ticker.rs
git commit -m "feat: per-farm delivery trucks replace instant food push"
```

---

### Task 6: Business closure on chronic insolvency

Add the hourly broke-hours tracking and closure, the `BusinessClosed` event, AI/arrival gating on `open()`, and occupant ejection.

**Files:**
- Modify: `src/sim/world.rs` (`process_closures` + hourly call; `arrive` guard; fingerprint already done in Task 4)
- Modify: `src/sim/event.rs` (`BusinessClosed`)
- Modify: `src/ui/ticker.rs` (format + color)
- Modify: `src/sim/ai.rs` (skip closed food venues)

- [ ] **Step 1: Write the failing test** — append to the `tests` module in `src/sim/world.rs`:

```rust
    #[test]
    fn chronically_broke_venue_closes_once_and_ejects() {
        let mut w = World::new(2161, 48);
        let venue = w.city.buildings.iter().find(|b| b.kind.is_food()).unwrap().id;
        // Strand the venue: broke and empty, and zero every farm so no truck can
        // ever recapitalize it.
        w.city.buildings[venue as usize].balance = 0.0;
        w.city.buildings[venue as usize].stock = 0.0;
        for b in w.city.buildings.iter_mut().filter(|b| b.kind == BuildingKind::HydroFarm) {
            b.stock = 0.0;
        }
        let mut closed_events = 0;
        let balance_at_close = std::cell::Cell::new(-1.0f32);
        // CLOSURE_GRACE_HOURS = 24; run ~26 hours.
        for _ in 0..(TICKS_PER_HOUR * 26) {
            w.tick();
            for ev in w.drain_events() {
                if matches!(ev.kind, EventKind::BusinessClosed { building } if building == venue) {
                    closed_events += 1;
                    balance_at_close.set(w.city.buildings[venue as usize].balance);
                }
            }
        }
        assert_eq!(closed_events, 1, "closure must fire exactly once");
        assert!(w.city.buildings[venue as usize].closed, "venue not marked closed");
        assert!(w.city.buildings[venue as usize].occupants.is_empty(), "occupants not ejected");
        // Balance is frozen after closing (conservation): unchanged from close onward.
        assert_eq!(w.city.buildings[venue as usize].balance, balance_at_close.get());
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test chronically_broke_venue_closes`
Expected: compile error — `EventKind::BusinessClosed` doesn't exist.

- [ ] **Step 3: Implement**

(a) In `src/sim/event.rs`, add the variant (after `DeliveryCompleted`):
```rust
    /// A food venue closed for good after chronic insolvency.
    BusinessClosed { building: u16 },
```

(b) In `src/ui/ticker.rs`, add a `format_event` arm (after `DeliveryCompleted`):
```rust
        EventKind::BusinessClosed { building } => (
            format!(
                "[{stamp}] {} #{:03} shut down — out of money",
                world.city.buildings[building as usize].kind.name(),
                building
            ),
            Selection::Building(building),
        ),
```
and an `event_color` arm:
```rust
        EventKind::BusinessClosed { .. } => Color::new(0.85, 0.2, 0.25, 1.0),
```

(c) In `src/sim/world.rs`, add the closure pass. Add this free function near `settle_wage`:
```rust
/// Hourly: a food venue that can't afford one wholesale meal accrues broke
/// hours; past the grace period it closes for good — occupants ejected, frozen
/// balance, dropped from orders/AI. Food venues are unstaffed, so the defensive
/// worker layoff is a no-op here.
fn process_closures(city: &mut City, citizens: &mut [Citizen], tick: u64, events: &mut VecDeque<SimEvent>) {
    let (supply, demand) = logistics::supply_demand(city);
    let price = economy::wholesale_price(supply, demand);
    let venues: Vec<u16> = city
        .buildings
        .iter()
        .filter(|b| b.kind.is_food() && b.open())
        .map(|b| b.id)
        .collect();
    for id in venues {
        let b = &mut city.buildings[id as usize];
        if b.balance < price {
            b.hours_broke += 1;
        } else {
            b.hours_broke = 0;
        }
        if b.hours_broke >= economy::CLOSURE_GRACE_HOURS {
            b.closed = true;
            let door = b.door;
            let occ = std::mem::take(&mut b.occupants);
            let workers = std::mem::take(&mut b.workers); // empty for venues; defensive
            for o in occ {
                citizens[o].pos = (door.0 as f32 + 0.5, door.1 as f32 + 0.5);
                citizens[o].state = CitizenState::Idle { until: tick + 1 };
            }
            for wkr in workers {
                citizens[wkr].job = None;
            }
            push_event(events, SimEvent { tick, kind: EventKind::BusinessClosed { building: id } });
        }
    }
}
```
Call it from `tick`, in the hourly block. Since the old hourly `distribute_food` block was removed in Task 5, add a fresh hourly block near the top of `tick` (after the `let (tick, hour, night) = ...;` line):
```rust
        if self.tick % TICKS_PER_HOUR == 0 {
            process_closures(&mut self.city, &mut self.citizens, tick, &mut self.events);
        }
```

Guard `arrive` so a citizen can't eat at a closed venue. In `arrive`, change the `Activity::Eat` stock check:
```rust
            if !building.open() || building.stock < 1.0 {
                c.state = CitizenState::Idle { until: tick + 60 };
                return;
            }
```

(d) In `src/sim/ai.rs`, skip closed food venues. In `choose_action`, change the food loop guard:
```rust
            if !b.open() || b.stock < 1.0 || c.money < economy::meal_price(b.kind) {
                continue;
            }
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test`
Expected: **69 passed** (68 + 1). `cargo build` — clean.

- [ ] **Step 5: Commit**

```bash
git add src/sim/world.rs src/sim/event.rs src/ui/ticker.rs src/sim/ai.rs
git commit -m "feat: venues close for good after chronic insolvency"
```

---

### Task 7: Update integration tests + tune against the soak

The conservation soak and `full_day_cycle_behavior` now run through trucks and dynamic pricing. Update the soak's stale comment, add a coverage guard, and confirm both pass at the shipped seed; tune constants only if they fail.

**Files:**
- Modify: `src/sim/world.rs` (`three_day_money_conservation_soak`)

- [ ] **Step 1: Update the soak test** — in `three_day_money_conservation_soak`:

Replace the doc comment's calibration line:
```rust
    /// (~0.3s in debug. If "every farm collapsed" ever fires after retuning,
    /// the calibration knob is WHOLESALE_PRICE — raise it toward 8.0.)
```
with:
```rust
    /// (~0.3s in debug. Food now arrives by truck at a dynamic price. If venues
    /// starve, the levers are economy::TRUCK_CAPACITY (raise), ORDER_THRESHOLD
    /// (raise), or PRICE_LO_MULT (raise, to protect farm solvency).)
```

Add a coverage assertion at the end of the test (after the "every farm collapsed" assert), guarding that the supply chain keeps venues alive:
```rust
        let open_venues = w
            .city
            .buildings
            .iter()
            .filter(|b| b.kind.is_food() && b.open())
            .count();
        assert!(open_venues >= 2, "supply chain starved the city: only {open_venues} venues open");
```

- [ ] **Step 2: Run the integration tests**

Run: `cargo test three_day_money_conservation_soak full_day_cycle_behavior -- --nocapture`
Expected: both PASS. The soak still asserts conservation drift < 0.5, `minted > 0`, `venue_total > 100`, day-3 eating, a solvent farm, and now ≥2 venues open.

- [ ] **Step 3 (contingency): tune only if a test fails**

If `full_day_cycle_behavior` or the soak fails because venues run dry:
- Raise `economy::TRUCK_CAPACITY` (30 → 45) so each run restocks more.
- Raise `economy::ORDER_THRESHOLD` (20 → 30) so venues order earlier.
If a farm goes insolvent (drivers quit, deliveries stop):
- Raise `economy::PRICE_LO_MULT` (0.6 → 0.8) so wholesale income holds the floor higher.
Re-run Step 2 after each change. Change one lever at a time.

- [ ] **Step 4: Run the full suite + determinism**

Run: `cargo test`
Expected: **69 passed** (count unchanged; this task edits existing tests). `deterministic_across_runs` must pass (trucks are in the fingerprint).

- [ ] **Step 5: Commit**

```bash
git add src/sim/world.rs
git commit -m "test: soak/full-day cover trucked delivery + dynamic price"
```

---

### Task 8: Render trucks and dark closed venues

Rendering only — untested by convention.

**Files:**
- Modify: `src/render/agents.rs` (`draw_trucks`)
- Modify: `src/render/mod.rs` (call `draw_trucks`)
- Modify: `src/render/buildings.rs` (dark trim for closed venues)

- [ ] **Step 1: Add `draw_trucks`** — in `src/render/agents.rs`, after `draw_citizens`:

```rust
/// Delivery trucks — larger than ambient cars, amber hauler with a cargo pip.
pub fn draw_trucks(world: &World, cam: &Camera, t: f32) {
    let night = crate::sim::time::is_night(world.tick);
    for truck in &world.trucks {
        if matches!(truck.state, crate::sim::logistics::TruckState::Parked) {
            continue; // parked at the farm; skip to keep the depot uncluttered
        }
        let (sx, sy) = cam.to_screen(truck.pos.0, truck.pos.1);
        if sx < -40.0 || sy < -40.0 || sx > screen_width() + 40.0 || sy > screen_height() + 40.0 {
            continue;
        }
        // direction from the next waypoint, for orientation
        let (mut dx, mut dy) = (1.0f32, 0.0f32);
        if let Some(&(nx, ny)) = truck.path.front() {
            let (vx, vy) = (nx as f32 + 0.5 - truck.pos.0, ny as f32 + 0.5 - truck.pos.1);
            let d = (vx * vx + vy * vy).sqrt().max(0.001);
            dx = vx / d;
            dy = vy / d;
        }
        let body = Color::new(1.0, 0.72, 0.24, 1.0); // amber hauler
        let (l, w) = (cam.ppt * 0.7, cam.ppt * 0.34);
        let horizontal = dx.abs() >= dy.abs();
        let (rw, rh) = if horizontal { (l, w) } else { (w, l) };
        draw_circle(sx, sy, cam.ppt * 0.55, Color::new(body.r, body.g, body.b, 0.12)); // glow
        draw_rectangle(sx - rw / 2.0, sy - rh / 2.0, rw, rh, body);
        // cargo pip when loaded
        if truck.cargo > 0.0 {
            draw_circle(sx, sy, cam.ppt * 0.12, Color::new(0.4, 1.0, 0.6, 0.95));
        }
        let _ = t;
        if night {
            let (hx, hy) = (sx + dx * l * 0.6, sy + dy * l * 0.6);
            draw_circle(hx, hy, cam.ppt * 0.1, Color::new(1.0, 1.0, 0.9, 0.95));
        }
    }
}
```

- [ ] **Step 2: Call it** — in `src/render/mod.rs`, in `draw_world`, after `traffic.draw(cam, world.tick);` and before `agents::draw_citizens(...)`:
```rust
    agents::draw_trucks(world, cam, t);
```

- [ ] **Step 3: Dark trim for closed venues** — in `src/render/buildings.rs`, in `draw_neon`, replace:
```rust
        let mut c = trim_color(b.kind);
        c.a = glow;
```
with:
```rust
        let mut c = if b.closed { Color::new(0.3, 0.32, 0.38, 1.0) } else { trim_color(b.kind) };
        c.a = if b.closed { 0.5 } else { glow };
```

- [ ] **Step 4: Verify build**

Run: `cargo build`
Expected: clean. Run `cargo test` — still **69 passed** (no test changes).

- [ ] **Step 5: Commit**

```bash
git add src/render/agents.rs src/render/mod.rs src/render/buildings.rs
git commit -m "feat: draw delivery trucks and dark closed venues"
```

---

### Task 9: Inspector — venue CLOSED marker + farm STOCK line

The driver "Driving for …" line landed in Task 4. Add a CLOSED marker for closed venues and a STOCK line for farms (which now hold inventory).

**Files:**
- Modify: `src/ui/inspector.rs` (`draw_building_panel`)

- [ ] **Step 1: Implement** — in `src/ui/inspector.rs`, in `draw_building_panel`:

(a) Show a CLOSED marker next to the `#id`. After the `#{:03}` `draw_text` line:
```rust
        if b.closed {
            draw_text("CLOSED", x + 14.0, y + 52.0, 18.0, Color::new(0.85, 0.2, 0.25, 1.0));
        }
```

(b) Add a STOCK line for farms (they hold inventory now). Change the food-venue STOCK block to also cover farms:
```rust
        if b.kind.is_food() || b.kind == crate::sim::city::BuildingKind::HydroFarm {
            draw_line_item("STOCK", &format!("{:.0} meals", b.stock), x, by);
            by += 24.0;
        }
        if b.kind.is_food() {
            draw_line_item("PRICE", &format!("${:.0}", crate::sim::economy::meal_price(b.kind)), x, by);
            by += 24.0;
        }
```
(Replace the original `if b.kind.is_food() { STOCK; PRICE }` block with the two blocks above.)

- [ ] **Step 2: Verify build**

Run: `cargo build` — clean. `cargo test` — **69 passed**.

- [ ] **Step 3: Commit**

```bash
git add src/ui/inspector.rs
git commit -m "feat: inspector shows CLOSED venues and farm stock"
```

---

### Task 10: Roadmap status + full verification + finish branch

- [ ] **Step 1: Mark Phase 3 in the roadmap** — in `docs/superpowers/roadmap/2026-06-11-living-world-roadmap.md`, change the Phase 3 status row from `pending` to `done`:
```
| 3 | Delivery trucks & supply chain | done |
```

- [ ] **Step 2: Commit**
```bash
git add docs/superpowers/roadmap/2026-06-11-living-world-roadmap.md
git commit -m "docs: roadmap phase 3 done"
```

- [ ] **Step 3: Suite + build**

Run: `cargo test` (expect **69 passed**) and `cargo build` (expect zero warnings).

- [ ] **Step 4: Visual verification** (headed browser — headless renders blank because of the app's MSAA; see the Phase-2 verification notes):
- `./build_web.sh`, serve `web/` (`python3 -m http.server 8080 -d web`), drive with a headed Playwright browser on a real GPU.
- Boot and let the sim run a few game-minutes: amber trucks drive the roads between farms and venues (distinct from the smaller ambient cars), cargo pip visible while loaded.
- Click a farm: STOCK line present and changing; click a venue: STOCK rises after a delivery.
- Watch the ticker for "delivered N meals" lines; clicking one selects the venue.
- Force a closure (optional): a venue with no income eventually shows CLOSED, goes dark, and a "shut down" ticker line appears.

- [ ] **Step 5: Finish the branch** — superpowers:finishing-a-development-branch (PR base: `living-world-phase-2`, since this stacks on the open Phase 2 PR #3; it retargets to `main` once #3 merges).

---

## Verification against spec (for the reviewer)

| Spec requirement | Task |
|---|---|
| Farms accumulate stock; production into farm inventory | 5 |
| Venues order below threshold; per-farm decentralized dispatch | 2, 5 |
| Trucks as sim agents (pos/path/speed); drive door-to-door | 4, 5 |
| Payment on delivery; leftover returns; conservation | 5 |
| Dynamic city-wide supply/demand wholesale price | 1, 5 |
| Driver occupation rides the truck (`Driving`), wages settle | 4, 5 |
| Venue closure after grace; eject occupants; frozen balance | 6 |
| AI/arrival skip closed venues | 6 |
| Events: DeliveryCompleted, BusinessClosed | 5, 6 |
| Determinism (trucks in fingerprint); conservation soak | 4, 7 |
| Coverage guard (venues keep selling) | 7 |
| Render trucks + dark closed venues; driver not a pedestrian | 4, 8 |
| Inspector: driving status, CLOSED marker, farm stock | 4, 9 |
| Replace `distribute_food`; remove its tests | 5 |
| Roadmap status updated | 10 |
