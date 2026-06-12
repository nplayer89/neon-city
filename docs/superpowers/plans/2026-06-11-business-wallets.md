# Business Wallets & Closed Money Loop — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Businesses hold bank balances; money flows citizen → venue → farm → farmhand and is conserved end to end, with insolvency when a farm can't make payroll.

**Architecture:** The sim layer (`src/sim/`) stays deterministic and dependency-free. `Building` gains `balance`/`insolvent`; retail payments credit venues; the hourly food distribution becomes a wholesale purchase; wages settle hourly (farms pay from balance, industry wages are minted and tracked in `World.minted`). Conservation invariant: `Σ wallets + Σ balances == initial + minted`. UI gets two new ticker events and a BALANCE line in the building inspector.

**Tech Stack:** Rust, macroquad (rendering/UI only), custom deterministic `Rng`. All sim behavior unit-tested; rendering code untested by convention (verify visually).

**Spec:** `docs/superpowers/specs/2026-06-11-business-wallets-design.md` — read it first.

**Conventions:**
- Run tests with `cargo test` (or `cargo test <name>` for one). The crate is a single binary; tests live in `#[cfg(test)]` modules per file.
- Commit style: `feat:` / `fix:` / `docs:` / `refactor:` prefixes, imperative mood.
- Money renders with `$` (ASCII only — the bundled font lacks `₢`).
- Some constants are added one task before the code that consumes them; `cargo build` may emit transient `dead_code` warnings in between. That's expected — don't "fix" it.
- Branch: work on `living-world-phase-2` (already created; spec committed there).

---

### Task 1: Building balance + insolvency fields

**Files:**
- Modify: `src/sim/city.rs` (Building struct ~line 61, `place_building` ~line 200, BuildingKind impl ~line 27, tests ~line 217)

- [ ] **Step 1: Write the failing tests** — append to the `tests` module in `src/sim/city.rs`:

```rust
#[test]
fn balances_seeded_by_kind() {
    let city = City::generate(&mut Rng::new(4));
    for b in &city.buildings {
        let expected = if b.kind.is_food() {
            100.0
        } else if b.kind == BuildingKind::HydroFarm {
            300.0
        } else {
            0.0
        };
        assert_eq!(b.balance, expected, "{:?}", b.kind);
        assert!(!b.insolvent, "{:?} starts insolvent", b.kind);
    }
}

#[test]
fn money_loop_participation_by_kind() {
    assert!(BuildingKind::NoodleBar.has_balance());
    assert!(BuildingKind::VendingPlaza.has_balance());
    assert!(BuildingKind::Arcade.has_balance());
    assert!(BuildingKind::HydroFarm.has_balance());
    assert!(!BuildingKind::Apartment.has_balance());
    assert!(!BuildingKind::FusionPlant.has_balance());
    assert!(BuildingKind::HydroFarm.wages_from_balance());
    assert!(!BuildingKind::DataCenter.wages_from_balance());
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test balances_seeded_by_kind money_loop_participation`
Expected: compile error — `balance`, `insolvent`, `has_balance`, `wages_from_balance` don't exist.

- [ ] **Step 3: Implement** — three edits in `src/sim/city.rs`:

(a) Add to the `impl BuildingKind` block (after `is_leisure`):

```rust
    /// Participates in the Phase 2 money loop (holds a balance the UI shows).
    pub fn has_balance(&self) -> bool {
        self.is_food() || matches!(self, BuildingKind::Arcade | BuildingKind::HydroFarm)
    }

    /// Employers that pay wages from their own balance. Everyone else's
    /// wages are minted (industry revenue is deferred — see Phase 2 spec).
    pub fn wages_from_balance(&self) -> bool {
        matches!(self, BuildingKind::HydroFarm)
    }
```

(b) Add fields to `struct Building` (after `stock`):

```rust
    /// Money the business holds (Phase 2). Stays 0 for kinds outside the loop.
    pub balance: f32,
    /// Latch so EmployerInsolvent events edge-trigger.
    pub insolvent: bool,
```

(c) In `place_building`, replace the `let stock = ...` line and the struct literal:

```rust
        let stock = if kind.is_food() { 20.0 } else { 0.0 };
        // Day-one float: venues can buy the first deliveries, farms can cover
        // roughly half a day of payroll before wholesale revenue arrives.
        let balance = if kind.is_food() {
            100.0
        } else if kind == BuildingKind::HydroFarm {
            300.0
        } else {
            0.0
        };
        self.buildings.push(Building {
            id,
            kind,
            x,
            y,
            w: bw,
            h: bh,
            door,
            stock,
            balance,
            insolvent: false,
            occupants: vec![],
            workers: vec![],
            vis_seed: rng.next_u32(),
        });
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test`
Expected: all tests pass (the two new ones included).

- [ ] **Step 5: Commit**

```bash
git add src/sim/city.rs
git commit -m "feat: building balance and insolvency fields with seeds"
```

---

### Task 2: Economy constants — prices, wholesale, wage ranges

**Files:**
- Modify: `src/sim/economy.rs` (`meal_price` ~line 4, new constants, tests)

- [ ] **Step 1: Write the failing tests** — append to the `tests` module in `src/sim/economy.rs`:

```rust
#[test]
fn retail_covers_wholesale() {
    for k in [BuildingKind::NoodleBar, BuildingKind::VendingPlaza] {
        assert!(
            meal_price(k) > WHOLESALE_PRICE,
            "{k:?} sells below wholesale"
        );
    }
}

#[test]
fn farm_wages_below_other_wages() {
    assert_eq!(wage_range(BuildingKind::HydroFarm), (9.0, 11.0));
    assert_eq!(wage_range(BuildingKind::DataCenter), (11.0, 18.0));
    assert_eq!(wage_range(BuildingKind::FusionPlant), (11.0, 18.0));
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test retail_covers_wholesale farm_wages_below`
Expected: compile error — `WHOLESALE_PRICE` and `wage_range` don't exist.

- [ ] **Step 3: Implement** — in `src/sim/economy.rs`:

(a) Reprice meals (spec: rebalance so farm books almost close):

```rust
pub fn meal_price(kind: BuildingKind) -> f32 {
    match kind {
        BuildingKind::NoodleBar => 15.0,
        BuildingKind::VendingPlaza => 10.0,
        _ => 0.0,
    }
}
```

(b) Add below the existing `FARM_OUTPUT_PER_HOUR` constant:

```rust
/// Farms charge venues this per meal at distribution time.
pub const WHOLESALE_PRICE: f32 = 7.0;
/// Cap on farmhands per farm; keeps farm payroll under wholesale income
/// (~72 meals/day x $7 ≈ $504 vs 6 farmhands x ~$10/h x 8h ≈ $480).
pub const FARM_MAX_WORKERS: usize = 3;
/// A full shift of missed pay makes a worker quit.
pub const UNPAID_HOURS_TO_QUIT: u32 = 8;

/// Hourly wage range (lo, hi) for jobs at this building kind.
pub fn wage_range(kind: BuildingKind) -> (f32, f32) {
    match kind {
        BuildingKind::HydroFarm => (9.0, 11.0),
        _ => (11.0, 18.0),
    }
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test`
Expected: all pass. (No existing test asserts the old meal prices; `ai::` tests use wallets of $0/$100, which straddle the new prices the same way.)

- [ ] **Step 5: Commit**

```bash
git add src/sim/economy.rs
git commit -m "feat: wholesale price, per-kind wage ranges, repriced meals"
```

---

### Task 3: New event kinds + ticker formatting

`format_event` and `event_color` in `src/ui/ticker.rs` match exhaustively on `EventKind`, so the enum variants and the ticker arms must land together to keep the build green.

**Files:**
- Modify: `src/sim/event.rs` (EventKind enum ~line 8)
- Modify: `src/ui/ticker.rs` (`format_event` ~line 33, `event_color` ~line 75, tests ~line 156)

- [ ] **Step 1: Write the failing tests** — append to the `tests` module in `src/ui/ticker.rs`:

```rust
#[test]
fn insolvent_line_names_employer_and_targets_it() {
    let w = World::new(3, 4);
    let farm = w
        .city
        .buildings
        .iter()
        .find(|b| b.kind == crate::sim::city::BuildingKind::HydroFarm)
        .unwrap();
    let ev = SimEvent { tick: 30, kind: EventKind::EmployerInsolvent { building: farm.id } };
    let (text, target) = format_event(&w, &ev);
    assert!(text.contains("payroll"), "{text}");
    assert!(text.contains(farm.kind.name()), "{text}");
    assert_eq!(target, Selection::Building(farm.id));
}

#[test]
fn worker_quit_line_names_both_and_targets_citizen() {
    let w = World::new(3, 4);
    let farm = w
        .city
        .buildings
        .iter()
        .find(|b| b.kind == crate::sim::city::BuildingKind::HydroFarm)
        .unwrap();
    let ev = SimEvent { tick: 30, kind: EventKind::WorkerQuit { citizen: 1, building: farm.id } };
    let (text, target) = format_event(&w, &ev);
    assert!(text.contains(&w.citizens[1].name), "{text}");
    assert!(text.contains("quit"), "{text}");
    assert!(text.contains(farm.kind.name()), "{text}");
    assert_eq!(target, Selection::Citizen(1));
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test insolvent_line worker_quit_line`
Expected: compile error — no such `EventKind` variants.

- [ ] **Step 3: Implement**

(a) `src/sim/event.rs` — add variants to `EventKind` (after `DailyWages`):

```rust
    /// An employer couldn't cover a wage payment (edge-triggered per episode).
    EmployerInsolvent { building: u16 },
    /// A worker quit after a full shift of missed pay.
    WorkerQuit { citizen: usize, building: u16 },
```

(b) `src/ui/ticker.rs` — add arms to the `match ev.kind` in `format_event` (after the `DailyWages` arm):

```rust
        EventKind::EmployerInsolvent { building } => (
            format!(
                "[{stamp}] {} #{:03} can't make payroll",
                world.city.buildings[building as usize].kind.name(),
                building
            ),
            Selection::Building(building),
        ),
        EventKind::WorkerQuit { citizen, building } => (
            format!(
                "[{stamp}] {} quit {} over unpaid wages",
                world.citizens[citizen].name,
                world.city.buildings[building as usize].kind.name()
            ),
            Selection::Citizen(citizen),
        ),
```

(c) `src/ui/ticker.rs` — add arms to `event_color`:

```rust
        EventKind::EmployerInsolvent { .. } => Color::new(1.0, 0.55, 0.15, 1.0),
        EventKind::WorkerQuit { .. } => Color::new(0.95, 0.4, 0.3, 1.0),
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src/sim/event.rs src/ui/ticker.rs
git commit -m "feat: insolvency and worker-quit events with ticker lines"
```

---

### Task 4: Wholesale distribution (venues buy stock from farms)

`produce_food` becomes `distribute_food`: same hourly cadence, but venues now *buy* their share at `WHOLESALE_PRICE`, limited by stock cap **and balance**; payment splits evenly across farms. A broke venue silently gets nothing (the existing `VenueSoldOut` event chain covers the consequence).

**Files:**
- Modify: `src/sim/economy.rs` (`produce_food` ~line 44 and its three existing tests)
- Modify: `src/sim/world.rs` (call site, ~line 90)

- [ ] **Step 1: Write the failing tests** — append to the `tests` module in `src/sim/economy.rs`:

```rust
#[test]
fn distribution_charges_venues_and_pays_farms() {
    let mut city = City::generate(&mut Rng::new(2));
    for b in city.buildings.iter_mut().filter(|b| b.kind.is_food()) {
        b.stock = 0.0;
    }
    let venues_before: f32 =
        city.buildings.iter().filter(|b| b.kind.is_food()).map(|b| b.balance).sum();
    let farms_before: f32 = city
        .buildings
        .iter()
        .filter(|b| b.kind == BuildingKind::HydroFarm)
        .map(|b| b.balance)
        .sum();
    distribute_food(&mut city, 10);
    let venues_after: f32 =
        city.buildings.iter().filter(|b| b.kind.is_food()).map(|b| b.balance).sum();
    let farms_after: f32 = city
        .buildings
        .iter()
        .filter(|b| b.kind == BuildingKind::HydroFarm)
        .map(|b| b.balance)
        .sum();
    assert!(venues_after < venues_before, "venues paid nothing");
    let paid = venues_before - venues_after;
    let received = farms_after - farms_before;
    assert!((paid - received).abs() < 1e-3, "leak: paid {paid}, received {received}");
}

#[test]
fn broke_venue_gets_no_stock() {
    let mut city = City::generate(&mut Rng::new(2));
    let id = city.buildings.iter().find(|b| b.kind.is_food()).unwrap().id as usize;
    city.buildings[id].stock = 0.0;
    city.buildings[id].balance = 0.0;
    distribute_food(&mut city, 10);
    assert_eq!(city.buildings[id].stock, 0.0);
    assert_eq!(city.buildings[id].balance, 0.0);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test distribution_charges broke_venue`
Expected: compile error — `distribute_food` doesn't exist.

- [ ] **Step 3: Implement** — replace `produce_food` in `src/sim/economy.rs` entirely with:

```rust
/// Hourly tick: farms grow food and sell it to venues at WHOLESALE_PRICE.
/// Each venue buys its even share, limited by the stock cap and its balance;
/// payment splits evenly across farms (equal output). Undelivered output is
/// lost — farms holding stock arrives with trucks in Phase 3.
pub fn distribute_food(city: &mut City, hour: u32) {
    if !(6..22).contains(&hour) {
        return;
    }
    let farms: Vec<usize> = city
        .buildings
        .iter()
        .filter(|b| b.kind == BuildingKind::HydroFarm)
        .map(|b| b.id as usize)
        .collect();
    let venues: Vec<usize> = city
        .buildings
        .iter()
        .filter(|b| b.kind.is_food())
        .map(|b| b.id as usize)
        .collect();
    if venues.is_empty() || farms.is_empty() {
        return;
    }
    let share = farms.len() as f32 * FARM_OUTPUT_PER_HOUR / venues.len() as f32;
    let mut wholesale_total = 0.0;
    for id in venues {
        let b = &mut city.buildings[id];
        let take = share
            .min((STOCK_CAP - b.stock).max(0.0))
            .min(b.balance / WHOLESALE_PRICE);
        if take <= 0.0 {
            continue;
        }
        b.stock += take;
        b.balance -= take * WHOLESALE_PRICE;
        wholesale_total += take * WHOLESALE_PRICE;
    }
    let per_farm = wholesale_total / farms.len() as f32;
    for id in farms {
        city.buildings[id].balance += per_farm;
    }
}
```

Update the three existing economy tests:
- `farms_stock_food_venues` and `no_production_at_night`: rename the call `produce_food(...)` → `distribute_food(...)` (no other change — venues' $100 seed affords the first delivery).
- `stock_caps`: venues now stop buying when broke, which would stop exercising the cap — fund them first so the cap stays the binding limit:

```rust
    #[test]
    fn stock_caps() {
        let mut city = City::generate(&mut Rng::new(2));
        for b in city.buildings.iter_mut().filter(|b| b.kind.is_food()) {
            b.balance = 1_000_000.0;
        }
        for _ in 0..1000 {
            distribute_food(&mut city, 10);
        }
        for b in city.buildings.iter().filter(|b| b.kind.is_food()) {
            assert!(b.stock <= STOCK_CAP);
        }
    }
```

In `src/sim/world.rs` (~line 90), update the call site:

```rust
        if self.tick % TICKS_PER_HOUR == 0 {
            economy::distribute_food(&mut self.city, hour);
        }
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test`
Expected: all pass. Note: `full_day_cycle_behavior`'s "city ran completely out of food" assert still holds — venues earn retail revenue all day (next task strengthens this further).

- [ ] **Step 5: Commit**

```bash
git add src/sim/economy.rs src/sim/world.rs
git commit -m "feat: hourly food distribution becomes a wholesale purchase"
```

---

### Task 5: Retail payments credit the venue

**Files:**
- Modify: `src/sim/world.rs` (`arrive` ~line 245, tests)

- [ ] **Step 1: Write the failing test** — append to the `tests` module in `src/sim/world.rs`:

```rust
#[test]
fn meal_payment_credits_venue() {
    let mut w = World::new(17, 6);
    let venue = w.city.buildings.iter().find(|b| b.kind.is_food()).unwrap().id;
    w.city.buildings[venue as usize].stock = 10.0;
    for c in w.citizens.iter_mut() {
        c.needs = Needs::full();
        c.job = None;
    }
    w.citizens[0].money = 100.0;
    w.citizens[0].path.clear();
    w.citizens[0].state = CitizenState::Traveling { to: Some(venue), activity: Activity::Eat };
    let balance_before = w.city.buildings[venue as usize].balance;
    let price = crate::sim::economy::meal_price(w.city.buildings[venue as usize].kind);
    w.tick();
    let gained = w.city.buildings[venue as usize].balance - balance_before;
    assert!((gained - price).abs() < 1e-3, "venue gained {gained}, price {price}");
    assert!((w.citizens[0].money - (100.0 - price)).abs() < 1e-3);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test meal_payment_credits_venue`
Expected: FAIL — `venue gained 0, price 15` (money still evaporates).

- [ ] **Step 3: Implement** — in `arrive()` in `src/sim/world.rs`:

The `Activity::Eat` arm:

```rust
            building.stock -= 1.0;
            c.money -= price;
            building.balance += price;
```

The `Activity::Fun` arm:

```rust
            c.money -= price;
            building.balance += price;
```

(Holo Park's `fun_price` is 0, so the free venue stays free.)

- [ ] **Step 4: Run to verify pass**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src/sim/world.rs
git commit -m "feat: meal and arcade payments credit the venue balance"
```

---

### Task 6: Hourly payroll from employer balance, minted industry wages, insolvency & quitting

The core task. Wages stop accruing per tick; instead, a worker in `Performing { Work }` at an hour boundary receives `wage_per_hour` in one transfer. Farms pay from balance; industry wages are minted into existence but tracked in `World.minted`. A failed farm payment edge-fires `EmployerInsolvent`; 8 consecutive-or-not unpaid hours (counter resets on any successful payment) make the worker quit with `WorkerQuit`.

**Files:**
- Modify: `src/sim/citizen.rs` (Job struct ~line 97)
- Modify: `src/sim/world.rs` (World struct ~line 11, `new` ~line 32/62, `tick` ~line 103, `tick_citizen` ~line 139, new `settle_wage`, tests)
- Modify: `src/sim/ai.rs` (two `Job { ... }` literals in tests, ~lines 148/158)

- [ ] **Step 1: Write the failing tests (paid paths)** — append to the `tests` module in `src/sim/world.rs`. Also extend the test-module imports: add `Job` to the existing `use crate::sim::citizen::{...}` line and add `use crate::sim::city::BuildingKind;`.

```rust
#[test]
fn farm_wages_come_from_farm_balance() {
    let mut w = World::new(21, 4);
    let farm =
        w.city.buildings.iter().find(|b| b.kind == BuildingKind::HydroFarm).unwrap().id;
    // Zero venue balances so no wholesale income muddies the farm's books.
    for b in w.city.buildings.iter_mut().filter(|b| b.kind.is_food()) {
        b.balance = 0.0;
    }
    w.city.buildings[farm as usize].balance = 500.0;
    for c in w.citizens.iter_mut() {
        c.needs = Needs::full();
        c.job = None;
    }
    w.citizens[0].job = Some(Job {
        workplace: farm,
        shift_start: 0,
        shift_end: 24,
        wage_per_hour: 10.0,
        unpaid_hours: 0,
    });
    w.city.buildings[farm as usize].workers.push(0);
    w.citizens[0].path.clear();
    w.citizens[0].state = CitizenState::Traveling { to: Some(farm), activity: Activity::Work };
    let money_before = w.citizens[0].money;
    for _ in 0..(TICKS_PER_HOUR * 2) {
        w.tick();
    }
    let earned = w.citizens[0].money - money_before;
    assert!(earned >= 10.0, "no hourly wage landed: {earned}");
    assert!(
        (earned / 10.0).fract().abs() < 1e-4,
        "wages not in whole-hour chunks: {earned}"
    );
    let farm_spent = 500.0 - w.city.buildings[farm as usize].balance;
    assert!((farm_spent - earned).abs() < 1e-3, "spent {farm_spent} != earned {earned}");
    assert_eq!(w.minted, 0.0, "farm wages must not mint");
}

#[test]
fn industry_wages_are_minted_and_tracked() {
    let mut w = World::new(21, 4);
    let dc =
        w.city.buildings.iter().find(|b| b.kind == BuildingKind::DataCenter).unwrap().id;
    for c in w.citizens.iter_mut() {
        c.needs = Needs::full();
        c.job = None;
    }
    w.citizens[0].job = Some(Job {
        workplace: dc,
        shift_start: 0,
        shift_end: 24,
        wage_per_hour: 14.0,
        unpaid_hours: 0,
    });
    w.citizens[0].path.clear();
    w.citizens[0].state = CitizenState::Traveling { to: Some(dc), activity: Activity::Work };
    let money_before = w.citizens[0].money;
    for _ in 0..(TICKS_PER_HOUR * 2) {
        w.tick();
    }
    let earned = w.citizens[0].money - money_before;
    assert!(earned >= 14.0, "no wage landed: {earned}");
    assert!((w.minted - earned).abs() < 1e-3, "minted {} != earned {earned}", w.minted);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test farm_wages_come_from industry_wages_are_minted`
Expected: compile error — `unpaid_hours` and `minted` don't exist.

- [ ] **Step 3: Implement the paid paths**

(a) `src/sim/citizen.rs` — add to `struct Job`:

```rust
    /// Hours worked without pay (employer broke). Resets on any paid hour;
    /// at economy::UNPAID_HOURS_TO_QUIT the worker quits.
    pub unpaid_hours: u32,
```

(b) Fix the now-broken `Job { ... }` literals by adding `unpaid_hours: 0,` to each: `src/sim/world.rs` `World::new` (~line 62) and test `working_pays_wages` (~line 349); `src/sim/ai.rs` tests `employed_citizen_works_during_shift` and `critical_need_overrides_work`.

(c) `src/sim/world.rs` — add to `struct World` (after `wages_today`):

```rust
    /// Money created from nothing since world start (industry payroll —
    /// their revenue is deferred; see the Phase 2 spec). The conservation
    /// invariant is: Σ wallets + Σ balances == initial total + minted.
    pub minted: f32,
```

Initialize `minted: 0.0` in the `World { ... }` literal in `new()`.

(d) `src/sim/world.rs` — add `settle_wage` (above `tick_citizen`). This version only handles paid hours; a missed payment does nothing yet (the next test cycle adds insolvency):

```rust
/// Hourly wage settlement while working. Returns (wages_paid, minted):
/// farm wages move from the employer balance; industry wages are minted.
fn settle_wage(
    c: &mut Citizen,
    city: &mut City,
    at: u16,
    tick: u64,
    events: &mut VecDeque<SimEvent>,
) -> (f32, f32) {
    if tick % TICKS_PER_HOUR != 0 {
        return (0.0, 0.0);
    }
    let Some(job) = c.job.as_mut() else { return (0.0, 0.0) };
    let wage = job.wage_per_hour;
    let employer = &mut city.buildings[at as usize];
    if !employer.kind.wages_from_balance() {
        c.money += wage;
        return (wage, wage);
    }
    if employer.balance >= wage {
        employer.balance -= wage;
        c.money += wage;
        job.unpaid_hours = 0;
        employer.insolvent = false;
        return (wage, 0.0);
    }
    let _ = events; // used by the insolvency cycle below
    (0.0, 0.0)
}
```

(e) `src/sim/world.rs` — rewire `tick_citizen`:
- Signature: `) -> f32 {` becomes `) -> (f32, f32) {`
- Body top: `let mut wages = 0.0;` becomes `let mut wages = 0.0; let mut minted = 0.0;`
- The early return in the Idle arm: `return wages;` becomes `return (wages, minted);`
- The final line: `wages` becomes `(wages, minted)`
- Replace the whole `Activity::Work` arm with:

```rust
                Activity::Work => {
                    let (w, m) = settle_wage(c, city, at, tick, events);
                    wages = w;
                    minted = m;
                    match &c.job {
                        Some(job) => !job.in_shift(hour) || c.needs.min_value() < 0.08,
                        None => true,
                    }
                }
```

(f) `src/sim/world.rs` — in `tick()`, replace the accumulation line:

```rust
            let (w, m) = tick_citizen(c, city, rng, tick, hour, night, events);
            self.wages_today += w;
            self.minted += m;
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test`
Expected: all pass, including the existing `working_pays_wages` (3 hour-boundaries fall inside its window), `daily_wage_summary_emitted_at_midnight`, and `full_day_cycle_behavior`.

- [ ] **Step 5: Commit**

```bash
git add src/sim/citizen.rs src/sim/world.rs src/sim/ai.rs
git commit -m "feat: hourly wage settlement; farms pay from balance, industry mints"
```

- [ ] **Step 6: Write the failing tests (insolvency + quit + recovery)** — append to the `tests` module in `src/sim/world.rs`:

```rust
#[test]
fn missed_payroll_fires_insolvency_once_then_quit_after_full_shift() {
    let mut w = World::new(21, 4);
    let farm =
        w.city.buildings.iter().find(|b| b.kind == BuildingKind::HydroFarm).unwrap().id;
    for b in w.city.buildings.iter_mut().filter(|b| b.kind.is_food()) {
        b.balance = 0.0; // no wholesale income for the farm
    }
    w.city.buildings[farm as usize].balance = 0.0;
    for c in w.citizens.iter_mut() {
        c.needs = Needs::full();
        c.job = None;
    }
    w.citizens[0].job = Some(Job {
        workplace: farm,
        shift_start: 0,
        shift_end: 24,
        wage_per_hour: 10.0,
        unpaid_hours: 0,
    });
    w.city.buildings[farm as usize].workers.push(0);
    w.citizens[0].path.clear();
    w.citizens[0].state = CitizenState::Traveling { to: Some(farm), activity: Activity::Work };

    let mut insolvent_events = 0;
    let mut quit_events = 0;
    for _ in 0..(TICKS_PER_HOUR * 9) {
        w.tick();
        for ev in w.drain_events() {
            match ev.kind {
                EventKind::EmployerInsolvent { building } if building == farm => {
                    insolvent_events += 1
                }
                EventKind::WorkerQuit { citizen: 0, building } if building == farm => {
                    quit_events += 1
                }
                _ => {}
            }
        }
    }
    assert_eq!(insolvent_events, 1, "insolvency must edge-trigger once");
    assert_eq!(quit_events, 1, "expected exactly one quit");
    assert!(w.citizens[0].job.is_none(), "job not cleared");
    assert!(
        !w.city.buildings[farm as usize].workers.contains(&0),
        "still on the workers list"
    );
}

#[test]
fn payment_resets_unpaid_count_and_insolvency_flag() {
    let mut w = World::new(21, 4);
    let farm =
        w.city.buildings.iter().find(|b| b.kind == BuildingKind::HydroFarm).unwrap().id;
    for b in w.city.buildings.iter_mut().filter(|b| b.kind.is_food()) {
        b.balance = 0.0;
    }
    w.city.buildings[farm as usize].balance = 0.0;
    for c in w.citizens.iter_mut() {
        c.needs = Needs::full();
        c.job = None;
    }
    w.citizens[0].job = Some(Job {
        workplace: farm,
        shift_start: 0,
        shift_end: 24,
        wage_per_hour: 10.0,
        unpaid_hours: 0,
    });
    w.city.buildings[farm as usize].workers.push(0);
    w.citizens[0].path.clear();
    w.citizens[0].state = CitizenState::Traveling { to: Some(farm), activity: Activity::Work };

    for _ in 0..(TICKS_PER_HOUR * 3) {
        w.tick();
    }
    assert!(w.citizens[0].job.unwrap().unpaid_hours >= 2, "no missed hours recorded");
    assert!(w.city.buildings[farm as usize].insolvent, "flag not set");

    w.city.buildings[farm as usize].balance = 500.0;
    for _ in 0..=TICKS_PER_HOUR {
        w.tick();
    }
    assert_eq!(
        w.citizens[0].job.unwrap().unpaid_hours,
        0,
        "paid hour must reset the counter"
    );
    assert!(!w.city.buildings[farm as usize].insolvent, "flag must clear on payment");
}
```

- [ ] **Step 7: Run to verify failure**

Run: `cargo test missed_payroll payment_resets`
Expected: `missed_payroll...` FAILS (no events fire, job never cleared); `payment_resets...` may partially pass but the `insolvent` flag assert FAILS.

- [ ] **Step 8: Implement insolvency** — in `settle_wage`, replace the trailing miss path (`let _ = events; (0.0, 0.0)`) with:

```rust
    job.unpaid_hours += 1;
    if !employer.insolvent {
        employer.insolvent = true;
        push_event(events, SimEvent { tick, kind: EventKind::EmployerInsolvent { building: at } });
    }
    if job.unpaid_hours >= economy::UNPAID_HOURS_TO_QUIT {
        push_event(events, SimEvent { tick, kind: EventKind::WorkerQuit { citizen: c.id, building: at } });
        employer.workers.retain(|&id| id != c.id);
        c.job = None;
    }
    (0.0, 0.0)
```

(The quit leaves `c.job == None`, so the Work arm's `done` match returns `true` and the citizen walks out the same tick.)

- [ ] **Step 9: Run to verify pass**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 10: Commit**

```bash
git add src/sim/world.rs
git commit -m "feat: insolvency events and quit after a full unpaid shift"
```

---

### Task 7: Spawn staffing — farm cap and per-kind wages

**Files:**
- Modify: `src/sim/world.rs` (`World::new` job assignment ~line 52, tests)

- [ ] **Step 1: Write the failing test** — append to the `tests` module in `src/sim/world.rs` (add `use crate::sim::economy;` to the test-module imports):

```rust
#[test]
fn farm_staffing_capped_and_wages_match_kind() {
    for seed in [7u64, 21, 2161] {
        let w = World::new(seed, 48);
        for b in w.city.buildings.iter().filter(|b| b.kind == BuildingKind::HydroFarm) {
            assert!(
                b.workers.len() <= economy::FARM_MAX_WORKERS,
                "seed {seed}: farm has {} workers",
                b.workers.len()
            );
        }
        for c in &w.citizens {
            if let Some(job) = &c.job {
                let kind = w.city.buildings[job.workplace as usize].kind;
                let (lo, hi) = economy::wage_range(kind);
                assert!(
                    job.wage_per_hour >= lo && job.wage_per_hour <= hi,
                    "seed {seed}: {kind:?} wage {} outside [{lo}, {hi}]",
                    job.wage_per_hour
                );
            }
        }
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test farm_staffing_capped`
Expected: FAIL — farms get ~4–5 round-robin workers, and farm wages land in 11–18.

- [ ] **Step 3: Implement** — in `World::new`, replace the employment block inside `if world.rng.chance(0.8) { ... }`:

```rust
            if world.rng.chance(0.8) {
                // Round-robin, but skip farms already at capacity — surplus
                // shifts to industry, whose minted wages absorb anyone.
                let mut idx = i % workplaces.len();
                for _ in 0..workplaces.len() {
                    let b = &world.city.buildings[workplaces[idx] as usize];
                    if b.kind != BuildingKind::HydroFarm
                        || b.workers.len() < economy::FARM_MAX_WORKERS
                    {
                        break;
                    }
                    idx = (idx + 1) % workplaces.len();
                }
                let wp = workplaces[idx];
                let roll = world.rng.gen_f32();
                let shift = if roll < SHIFT_WEIGHTS[0] {
                    SHIFTS[0]
                } else if roll < SHIFT_WEIGHTS[0] + SHIFT_WEIGHTS[1] {
                    SHIFTS[1]
                } else {
                    SHIFTS[2]
                };
                let (lo, hi) = economy::wage_range(world.city.buildings[wp as usize].kind);
                c.job = Some(Job {
                    workplace: wp,
                    shift_start: shift.0,
                    shift_end: shift.1,
                    wage_per_hour: world.rng.gen_f32_range(lo, hi),
                    unpaid_hours: 0,
                });
                world.city.buildings[wp as usize].workers.push(i);
            }
```

(`Job` is already imported at the top of `world.rs`; `economy` and `BuildingKind` too.)

- [ ] **Step 4: Run to verify pass**

Run: `cargo test`
Expected: all pass (`citizens_have_homes_and_most_have_jobs` is staffing-agnostic).

- [ ] **Step 5: Commit**

```bash
git add src/sim/world.rs
git commit -m "feat: cap farm staffing and pay per-kind wage ranges"
```

---

### Task 8: Fingerprint coverage + three-day conservation soak

**Files:**
- Modify: `src/sim/world.rs` (`fingerprint` ~line 119, new `total_money`, tests)

- [ ] **Step 1: Write the failing test** — append to the `tests` module in `src/sim/world.rs`:

```rust
/// Phase 2 exit criterion: money is conserved end to end. Runs three game
/// days at the shipped seed and checks the economy is still alive after.
/// (Slow in debug — roughly 10–30s; it earns its keep.)
#[test]
fn three_day_money_conservation_soak() {
    let mut w = World::new(2161, 48);
    let initial = w.total_money();
    for _ in 0..(crate::sim::time::TICKS_PER_DAY * 3) {
        w.tick();
    }
    let drift = (w.total_money() - initial - w.minted).abs();
    assert!(drift < 0.5, "conservation drift {drift} (minted {})", w.minted);
    assert!(w.minted > 0.0, "industry never paid wages");
    let venue_total: f32 =
        w.city.buildings.iter().filter(|b| b.kind.is_food()).map(|b| b.balance).sum();
    assert!(venue_total > 0.0, "every venue is broke");
    assert!(
        w.city
            .buildings
            .iter()
            .any(|b| b.kind == BuildingKind::HydroFarm && !b.workers.is_empty() && !b.insolvent),
        "every farm collapsed"
    );
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test three_day_money_conservation_soak`
Expected: compile error — `total_money` doesn't exist.

- [ ] **Step 3: Implement**

(a) Add next to `fingerprint` in `impl World`:

```rust
    /// Σ wallets + Σ building balances. Conserved up to `minted` — see the
    /// conservation soak test.
    #[cfg(test)]
    pub fn total_money(&self) -> f32 {
        self.citizens.iter().map(|c| c.money).sum::<f32>()
            + self.city.buildings.iter().map(|b| b.balance).sum::<f32>()
    }
```

(b) Extend `fingerprint` so determinism tests cover the new state — the buildings loop gains `balance`, and `minted` mixes in at the end:

```rust
        for b in &self.city.buildings {
            h = mix(h, b.stock.to_bits() as u64);
            h = mix(h, b.balance.to_bits() as u64);
            h = mix(h, b.occupants.len() as u64);
        }
        h = mix(h, self.minted.to_bits() as u64);
        h
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test`
Expected: all pass. If the soak's "every farm collapsed" assert ever fails, the tuning knob is `WHOLESALE_PRICE` (raise toward 8.0) — but at 7.0 the average farm clears ~$10/day profit, so it should hold.

- [ ] **Step 5: Commit**

```bash
git add src/sim/world.rs
git commit -m "test: conservation soak; fingerprint covers balances and minted"
```

---

### Task 9: Inspector BALANCE line

Rendering code is untested by repo convention — verify visually.

**Files:**
- Modify: `src/ui/inspector.rs` (`draw_building_panel` ~line 188)

- [ ] **Step 1: Implement** — in `draw_building_panel`, insert between the `is_food()` block and the `is_workplace()` block:

```rust
        if b.kind.has_balance() {
            draw_line_item("BALANCE", &format!("${:.0}", b.balance), x, by);
            by += 24.0;
        }
```

- [ ] **Step 2: Build and verify visually**

Run: `cargo build` (expect: clean compile), then `cargo run`:
- Click a Noodle Bar → STOCK / PRICE / BALANCE rows; balance rises when someone eats, dips on the hourly delivery.
- Click a Hydro Farm → BALANCE / WORKERS rows; balance climbs hourly (wholesale), drops on the hour (payroll).
- Click a Fusion Plant → no BALANCE row (deferred from the loop).

- [ ] **Step 3: Run full suite, then commit**

Run: `cargo test`
Expected: all pass.

```bash
git add src/ui/inspector.rs
git commit -m "feat: building inspector shows balance for money-loop buildings"
```

---

### Task 10: Roadmap status + final verification

**Files:**
- Modify: `docs/superpowers/roadmap/2026-06-11-living-world-roadmap.md` (status table ~line 20)

- [ ] **Step 1: Update the roadmap status table** — Phase 2 row becomes:

```markdown
| 2 | Business wallets & closed money loop | done |
```

- [ ] **Step 2: Full verification**

Run: `cargo fmt && cargo test`
Expected: no formatting diff churn beyond the files touched; every test passes.

Run: `cargo run` for a couple of game days at high speed; sanity-watch the ticker for the occasional `can't make payroll` / `quit` lines (rare is correct) and `DailyWages` totals (~$4k–5k).

- [ ] **Step 3: Commit**

```bash
git add docs/superpowers/roadmap/2026-06-11-living-world-roadmap.md
git commit -m "docs: roadmap phase 2 done"
```

- [ ] **Step 4: Finish the branch** — use the superpowers:finishing-a-development-branch skill (PR per repo convention: phase 1 merged via PR).

---

## Verification against spec (for the reviewer)

| Spec requirement | Task |
|---|---|
| `Building.balance` + seeds ($100 venues / $300 farms) | 1 |
| Retail credits venue; Holo Park free | 5 |
| Wholesale $7 at distribution, cap- and balance-limited, even farm split | 2, 4 |
| Hourly payroll; farms from balance; industry minted + tracked | 6 |
| Insolvency edge event; quit after 8 unpaid hours; counter resets on pay | 6 |
| Farmhand wages $9–11/h; 3 workers/farm cap | 2, 7 |
| Events `EmployerInsolvent` / `WorkerQuit` + ticker lines | 3 |
| Inspector BALANCE line via `has_balance()` | 1, 9 |
| Conservation test (±0.5 over 3 days) + alive checks; fingerprint coverage | 8 |
| Roadmap status update | 10 |
