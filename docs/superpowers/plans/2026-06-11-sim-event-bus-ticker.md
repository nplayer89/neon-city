# Sim Event Bus & News Ticker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A typed event queue in the sim layer plus a clickable bottom-of-screen news ticker, so the world's happenings (sold-out venues, starving citizens, daily wage totals) are visible and selectable.

**Architecture:** New `sim::event` module defines `SimEvent`/`EventKind` as plain `Copy` data — no strings, no colors; formatting belongs to the UI. `World` accumulates events in a capped `VecDeque` during `tick()`; the UI drains once per frame into a new `ui::ticker` that renders the last 5 entries above the population strip and returns a `Selection` when a row is clicked (reusing the inspector's selection enum). Event emission is edge-triggered (fires on crossings, not every tick) so the sim stays deterministic and the ticker stays quiet.

**Tech Stack:** Rust 2021, macroquad 0.4 (already present — no new dependencies). Phase 1 of `docs/superpowers/roadmap/2026-06-11-living-world-roadmap.md`.

**Event types (4):**
| Kind | Fires when | Click target |
|------|-----------|--------------|
| `VenueSoldOut { building }` | a meal purchase drops a venue's stock below 1.0 | the building |
| `CriticalNeed { citizen, need }` | a need decays across `ai::CRITICAL` (0.15) from above | the citizen |
| `CantAffordMeal { citizen, building }` | a citizen arrives to eat but `money < price` | the citizen |
| `DailyWages { day, total }` | midnight rollover; total wages paid that day | none |

**File structure:**
- Create: `src/sim/event.rs` — event types + capped push helper (sole responsibility: event data)
- Create: `src/ui/ticker.rs` — formatting + ticker widget (sole responsibility: event display)
- Modify: `src/sim/mod.rs`, `src/ui/mod.rs` — register modules
- Modify: `src/sim/world.rs` — queue field, drain, emission points, wage accounting
- Modify: `src/sim/ai.rs` — make `CRITICAL` public
- Modify: `src/ui/inspector.rs` — add `Debug` to `Selection`
- Modify: `src/main.rs` — wiring

---

### Task 1: `sim::event` module

**Files:**
- Create: `src/sim/event.rs`
- Modify: `src/sim/mod.rs`

- [ ] **Step 1: Create the module with types, helper, and a failing-by-construction test**

Create `src/sim/event.rs`:

```rust
use crate::sim::citizen::NeedKind;
use std::collections::VecDeque;

/// Sim-layer happenings surfaced by the UI news ticker.
/// Plain data only — wording and colors are the UI's job.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum EventKind {
    /// A food venue's last meal was just sold.
    VenueSoldOut { building: u16 },
    /// A need decayed across the critical threshold (ai::CRITICAL).
    CriticalNeed { citizen: usize, need: NeedKind },
    /// A citizen arrived hungry but couldn't pay for the meal.
    CantAffordMeal { citizen: usize, building: u16 },
    /// Total wages paid over the day that just ended.
    DailyWages { day: u64, total: f32 },
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SimEvent {
    pub tick: u64,
    pub kind: EventKind,
}

/// Pending-event cap so an undrained world (headless tests) can't grow unbounded.
pub const MAX_PENDING: usize = 256;

/// Push with cap: oldest events drop first.
pub fn push_event(events: &mut VecDeque<SimEvent>, ev: SimEvent) {
    if events.len() == MAX_PENDING {
        events.pop_front();
    }
    events.push_back(ev);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_caps_pending_events_dropping_oldest() {
        let mut q = VecDeque::new();
        for i in 0..(MAX_PENDING as u64 + 40) {
            push_event(&mut q, SimEvent { tick: i, kind: EventKind::VenueSoldOut { building: 0 } });
        }
        assert_eq!(q.len(), MAX_PENDING);
        assert_eq!(q.front().unwrap().tick, 40, "oldest events should drop first");
        assert_eq!(q.back().unwrap().tick, MAX_PENDING as u64 + 39);
    }
}
```

- [ ] **Step 2: Run the test — it fails because the module isn't registered**

Run: `cargo test sim::event`
Expected: 0 tests run (module not compiled) — or compile error if referenced. Confirms registration is needed.

- [ ] **Step 3: Register the module**

In `src/sim/mod.rs`, add one line after `pub mod economy;`:

```rust
pub mod rng;
pub mod time;
pub mod city;
pub mod path;
pub mod citizen;
pub mod economy;
pub mod event;
pub mod ai;
pub mod world;
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test sim::event`
Expected: `test sim::event::tests::push_caps_pending_events_dropping_oldest ... ok` — 1 passed.

- [ ] **Step 5: Commit**

```bash
git add src/sim/event.rs src/sim/mod.rs
git commit -m "feat: sim event types with capped pending queue"
```

---

### Task 2: World event queue, drain, and DailyWages emission

`World` gains the queue and a daily wage accumulator. `tick_citizen` returns the wage it paid this tick (previously implicit in `c.money += …`), and `arrive` gains an events parameter (unused until Task 4).

**Files:**
- Modify: `src/sim/world.rs` (struct ~line 10, `new` ~line 26, `tick` ~line 79, `tick_citizen` ~line 115, `arrive` ~line 216, tests module ~line 242)

- [ ] **Step 1: Write the failing test**

Add to the `tests` module at the bottom of `src/sim/world.rs`:

```rust
    #[test]
    fn daily_wage_summary_emitted_at_midnight() {
        let mut w = World::new(99, 40);
        let mut summaries = vec![];
        for _ in 0..crate::sim::time::TICKS_PER_DAY {
            w.tick();
            for ev in w.drain_events() {
                if let EventKind::DailyWages { day, total } = ev.kind {
                    summaries.push((day, total, ev.tick));
                }
            }
        }
        assert_eq!(summaries.len(), 1, "expected exactly one daily summary");
        let (day, total, tick) = summaries[0];
        assert_eq!(day, 1);
        assert!(total > 0.0, "no wages accumulated");
        assert_eq!(tick, crate::sim::time::TICKS_PER_DAY);
    }
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test daily_wage_summary`
Expected: COMPILE ERROR — `no method named drain_events`, `cannot find EventKind`.

- [ ] **Step 3: Implement the plumbing**

In `src/sim/world.rs`, update the imports at the top:

```rust
use crate::sim::ai;
use crate::sim::citizen::{Activity, Citizen, CitizenState, Job, NeedKind};
use crate::sim::city::{BuildingKind, City};
use crate::sim::economy;
use crate::sim::event::{push_event, EventKind, SimEvent};
use crate::sim::path;
use crate::sim::rng::Rng;
use crate::sim::time::{self, TICKS_PER_DAY, TICKS_PER_HOUR};
use std::collections::VecDeque;
```

Extend the `World` struct:

```rust
pub struct World {
    pub rng: Rng,
    pub city: City,
    pub citizens: Vec<Citizen>,
    pub tick: u64,
    pub seed: u64,
    /// Events since the last drain (capped; see event::MAX_PENDING).
    pub events: VecDeque<SimEvent>,
    /// Wages paid since the last midnight rollover.
    wages_today: f32,
}
```

In `World::new`, initialize the new fields:

```rust
        let mut world = World {
            rng,
            city,
            citizens: vec![],
            tick: 0,
            seed,
            events: VecDeque::new(),
            wages_today: 0.0,
        };
```

Replace `World::tick` with:

```rust
    pub fn tick(&mut self) {
        self.tick += 1;
        let (tick, hour, night) = (self.tick, self.hour(), time::is_night(self.tick));
        if self.tick % TICKS_PER_HOUR == 0 {
            economy::produce_food(&mut self.city, hour);
        }
        let city = &mut self.city;
        let rng = &mut self.rng;
        let events = &mut self.events;
        let mut wages = 0.0;
        for c in self.citizens.iter_mut() {
            c.decay_needs();
            wages += tick_citizen(c, city, rng, tick, hour, night, events);
        }
        self.wages_today += wages;
        if self.tick % TICKS_PER_DAY == 0 {
            let summary = EventKind::DailyWages { day: time::day(self.tick) - 1, total: self.wages_today };
            push_event(&mut self.events, SimEvent { tick: self.tick, kind: summary });
            self.wages_today = 0.0;
        }
    }

    /// Hand pending events to the UI (or tests); empties the queue.
    pub fn drain_events(&mut self) -> Vec<SimEvent> {
        self.events.drain(..).collect()
    }
```

Change `tick_citizen` to thread events through and return the wage paid this tick. Signature and the two changed arms (the rest of the body is unchanged):

```rust
fn tick_citizen(
    c: &mut Citizen,
    city: &mut City,
    rng: &mut Rng,
    tick: u64,
    hour: u32,
    night: bool,
    events: &mut VecDeque<SimEvent>,
) -> f32 {
    let mut wages = 0.0;
    match c.state {
        // ... Idle arm unchanged ...
        CitizenState::Traveling { to, activity } => {
            // ... movement unchanged; the arrival call becomes:
            if c.path.is_empty() {
                match to {
                    Some(b) => arrive(c, city, b, activity, tick, events),
                    None => c.state = CitizenState::Idle { until: tick + rng.gen_range(60, 240) as u64 },
                }
            }
        }
        CitizenState::Performing { at, activity } => {
            // ... only the Work arm changes:
                Activity::Work => {
                    if let Some(job) = &c.job {
                        let pay = job.wage_per_hour / TICKS_PER_HOUR as f32;
                        c.money += pay;
                        wages = pay;
                        !job.in_shift(hour) || c.needs.min_value() < 0.08
                    } else {
                        true
                    }
                }
            // ... rest of Performing arm unchanged ...
        }
    }
    wages
}
```

Note: `match c.state { … }` was previously the tail expression; it now becomes a statement followed by `wages`. No arm returns a value, so no other changes are needed.

Change `arrive`'s signature (parameter is unused until Task 4 — underscore it for now):

```rust
fn arrive(c: &mut Citizen, city: &mut City, b: u16, act: Activity, tick: u64, _events: &mut VecDeque<SimEvent>) {
```

- [ ] **Step 4: Run the tests**

Run: `cargo test`
Expected: all tests pass, including `daily_wage_summary_emitted_at_midnight` and the untouched `deterministic_across_runs` (events never touch the RNG).

- [ ] **Step 5: Commit**

```bash
git add src/sim/world.rs
git commit -m "feat: world event queue with daily wage summary event"
```

---

### Task 3: CriticalNeed edge-trigger

**Files:**
- Modify: `src/sim/ai.rs:11` (make `CRITICAL` public)
- Modify: `src/sim/world.rs` (tick loop + imports + test)

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `src/sim/world.rs`:

```rust
    #[test]
    fn critical_need_event_fires_once_on_crossing() {
        let mut w = World::new(31, 10);
        for c in w.citizens.iter_mut() {
            c.needs = Needs::full();
            c.job = None;
        }
        w.citizens[0].needs.hunger = ai::CRITICAL + 0.005;
        w.citizens[0].money = 0.0; // can't buy food, so hunger keeps falling
        let mut crossings = 0;
        for _ in 0..(TICKS_PER_HOUR * 4) {
            w.tick();
            for ev in w.drain_events() {
                if let EventKind::CriticalNeed { citizen: 0, need: NeedKind::Hunger } = ev.kind {
                    crossings += 1;
                }
            }
        }
        assert_eq!(crossings, 1, "edge trigger fired {crossings} times");
    }
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test critical_need_event`
Expected: COMPILE ERROR — `constant CRITICAL is private`. (After making it public it would fail with `crossings == 0`.)

- [ ] **Step 3: Implement**

In `src/sim/ai.rs`, change line 11:

```rust
/// Needs below this hijack the citizen's agenda (and emit a ticker event).
pub const CRITICAL: f32 = 0.15;
```

In `src/sim/world.rs`, extend the citizen import to include `NEED_KINDS`:

```rust
use crate::sim::citizen::{Activity, Citizen, CitizenState, Job, NeedKind, NEED_KINDS};
```

In `World::tick`, wrap the decay with before/after crossing detection (`Needs` is `Copy`):

```rust
        for c in self.citizens.iter_mut() {
            let before = c.needs;
            c.decay_needs();
            for k in NEED_KINDS {
                if before.get(k) >= ai::CRITICAL && c.needs.get(k) < ai::CRITICAL {
                    push_event(events, SimEvent { tick, kind: EventKind::CriticalNeed { citizen: c.id, need: k } });
                }
            }
            wages += tick_citizen(c, city, rng, tick, hour, night, events);
        }
```

- [ ] **Step 4: Run the tests**

Run: `cargo test`
Expected: all pass. The new test sees exactly 1 crossing (hunger falls past 0.15 within ~50–100 ticks and stays below; full-bar needs can't reach 0.15 in 4 game hours).

- [ ] **Step 5: Commit**

```bash
git add src/sim/ai.rs src/sim/world.rs
git commit -m "feat: edge-triggered critical-need events"
```

---

### Task 4: VenueSoldOut and CantAffordMeal emission in `arrive`

The AI pre-filters unaffordable/empty venues (`ai.rs:62`), so `arrive` rejections only happen when stock or money changed mid-walk. Tests therefore steer a citizen manually: `Traveling` state with an empty path means `arrive` runs on the next tick.

**Files:**
- Modify: `src/sim/world.rs` (`arrive` + two tests)

- [ ] **Step 1: Write the two failing tests**

Add to the `tests` module in `src/sim/world.rs`:

```rust
    #[test]
    fn selling_last_meal_emits_sold_out_once() {
        let mut w = World::new(17, 6);
        let venue = w.city.buildings.iter().find(|b| b.kind.is_food()).unwrap().id;
        for b in w.city.buildings.iter_mut().filter(|b| b.kind.is_food()) {
            b.stock = 0.0;
        }
        w.city.buildings[venue as usize].stock = 1.0;
        for c in w.citizens.iter_mut() {
            c.needs = Needs::full();
            c.job = None;
        }
        // Walk citizen 0 straight in: empty path + Traveling = arrive on next tick.
        w.citizens[0].money = 100.0;
        w.citizens[0].path.clear();
        w.citizens[0].state = CitizenState::Traveling { to: Some(venue), activity: Activity::Eat };

        let mut sold_out = 0;
        for _ in 0..200 {
            w.tick();
            for ev in w.drain_events() {
                if ev.kind == (EventKind::VenueSoldOut { building: venue }) {
                    sold_out += 1;
                }
            }
        }
        assert_eq!(sold_out, 1, "sold-out fired {sold_out} times");
        assert!(w.city.buildings[venue as usize].stock < 1.0);
    }

    #[test]
    fn arriving_broke_emits_cant_afford() {
        let mut w = World::new(17, 6);
        let venue = w.city.buildings.iter().find(|b| b.kind.is_food()).unwrap().id;
        w.city.buildings[venue as usize].stock = 10.0;
        for c in w.citizens.iter_mut() {
            c.needs = Needs::full();
            c.job = None;
        }
        w.citizens[0].money = 0.0;
        w.citizens[0].path.clear();
        w.citizens[0].state = CitizenState::Traveling { to: Some(venue), activity: Activity::Eat };

        w.tick();
        let events = w.drain_events();
        assert!(
            events.iter().any(|e| e.kind == (EventKind::CantAffordMeal { citizen: 0, building: venue })),
            "no cant-afford event in {events:?}"
        );
        assert!(matches!(w.citizens[0].state, CitizenState::Idle { .. }));
    }
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -- selling_last_meal arriving_broke`
Expected: FAIL — `sold-out fired 0 times` and `no cant-afford event in []`.

- [ ] **Step 3: Implement**

In `src/sim/world.rs`, replace the `Activity::Eat` arm of `arrive` (and rename `_events` back to `events` in the signature):

```rust
fn arrive(c: &mut Citizen, city: &mut City, b: u16, act: Activity, tick: u64, events: &mut VecDeque<SimEvent>) {
    let building = &mut city.buildings[b as usize];
    match act {
        Activity::Eat => {
            if building.stock < 1.0 {
                c.state = CitizenState::Idle { until: tick + 60 };
                return;
            }
            let price = economy::meal_price(building.kind);
            if c.money < price {
                push_event(events, SimEvent { tick, kind: EventKind::CantAffordMeal { citizen: c.id, building: b } });
                c.state = CitizenState::Idle { until: tick + 60 };
                return;
            }
            building.stock -= 1.0;
            c.money -= price;
            if building.stock < 1.0 {
                push_event(events, SimEvent { tick, kind: EventKind::VenueSoldOut { building: b } });
            }
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

(The combined `stock < 1.0 || money < price` rejection is split so the two cases emit differently; sold-out arrivals don't emit — the crossing already did.)

- [ ] **Step 4: Run the tests**

Run: `cargo test`
Expected: all pass. The sold-out test sees exactly one event (test runs 200 ticks at game hour 0, before any hourly food production at tick 600 could restock).

- [ ] **Step 5: Commit**

```bash
git add src/sim/world.rs
git commit -m "feat: sold-out and cant-afford-meal events on venue arrival"
```

---

### Task 5: `ui::ticker` widget

Formatting is a pure, testable function; drawing follows the existing immediate-mode panel conventions (`hud::over` for hover, `pointer_over_ui` to block world click-through — the carried-to-next-frame pattern the roster already uses).

**Files:**
- Create: `src/ui/ticker.rs`
- Modify: `src/ui/mod.rs`
- Modify: `src/ui/inspector.rs:9` (add `Debug` to `Selection` so tests can `assert_eq!` it)

- [ ] **Step 1: Add `Debug` to `Selection`**

In `src/ui/inspector.rs` line 9:

```rust
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Selection {
```

- [ ] **Step 2: Create the ticker with formatting tests included**

Create `src/ui/ticker.rs`:

```rust
use crate::sim::citizen::NeedKind;
use crate::sim::event::{EventKind, SimEvent};
use crate::sim::time::{self, TICKS_PER_HOUR};
use crate::sim::world::World;
use crate::ui::hud::{over, HudState, CYAN, PANEL, PANEL_EDGE};
use crate::ui::inspector::Selection;
use macroquad::prelude::*;
use std::collections::VecDeque;

/// Visible rows; older entries scroll off.
const MAX_ENTRIES: usize = 5;
const ROW_H: f32 = 20.0;
const TICKER_W: f32 = 470.0;
/// Clear of the roster sidebar (240px) on the left.
const TICKER_X: f32 = 254.0;

struct Entry {
    text: String,
    target: Selection,
    color: Color,
}

pub struct Ticker {
    entries: VecDeque<Entry>,
}

/// Ticker line + click-to-select target for a sim event.
pub fn format_event(world: &World, ev: &SimEvent) -> (String, Selection) {
    let mins = (ev.tick % TICKS_PER_HOUR) * 60 / TICKS_PER_HOUR;
    let stamp = format!("D{} {:02}:{:02}", time::day(ev.tick), time::hour(ev.tick), mins);
    match ev.kind {
        EventKind::VenueSoldOut { building } => (
            format!(
                "[{stamp}] {} #{:03} sold out of meals",
                world.city.buildings[building as usize].kind.name(),
                building
            ),
            Selection::Building(building),
        ),
        EventKind::CriticalNeed { citizen, need } => {
            let verb = match need {
                NeedKind::Hunger => "is starving",
                NeedKind::Energy => "is exhausted",
                NeedKind::Hygiene => "desperately needs a shower",
                NeedKind::Fun => "is bored stiff",
            };
            (
                format!("[{stamp}] {} {verb}", world.citizens[citizen].name),
                Selection::Citizen(citizen),
            )
        }
        EventKind::CantAffordMeal { citizen, building } => (
            format!(
                "[{stamp}] {} can't afford a meal at {}",
                world.citizens[citizen].name,
                world.city.buildings[building as usize].kind.name()
            ),
            Selection::Citizen(citizen),
        ),
        EventKind::DailyWages { day, total } => (
            format!("[{stamp}] Day {day} wrap-up: ₢ {total:.0} paid in wages"),
            Selection::None,
        ),
    }
}

fn event_color(kind: &EventKind) -> Color {
    match kind {
        EventKind::VenueSoldOut { .. } => Color::new(1.0, 0.75, 0.25, 1.0),
        EventKind::CriticalNeed { .. } => Color::new(1.0, 0.35, 0.35, 1.0),
        EventKind::CantAffordMeal { .. } => Color::new(1.0, 0.3, 0.85, 1.0),
        EventKind::DailyWages { .. } => CYAN,
    }
}

impl Ticker {
    pub fn new() -> Ticker {
        Ticker { entries: VecDeque::new() }
    }

    pub fn push(&mut self, world: &World, ev: &SimEvent) {
        let (text, target) = format_event(world, ev);
        if self.entries.len() == MAX_ENTRIES {
            self.entries.pop_front();
        }
        self.entries.push_back(Entry { text, target, color: event_color(&ev.kind) });
    }

    /// Bottom strip, newest row last. Returns a clicked row's target.
    pub fn draw(&self, hud: &mut HudState) -> Option<Selection> {
        if self.entries.is_empty() {
            return None;
        }
        let h = ROW_H * self.entries.len() as f32;
        let y = screen_height() - 36.0 - h;
        draw_rectangle(TICKER_X, y, TICKER_W, h, PANEL);
        draw_rectangle_lines(TICKER_X, y, TICKER_W, h, 1.0, PANEL_EDGE);
        let mut clicked = None;
        for (i, e) in self.entries.iter().enumerate() {
            let ry = y + i as f32 * ROW_H;
            if over(TICKER_X, ry, TICKER_W, ROW_H) {
                hud.pointer_over_ui = true;
                if e.target != Selection::None {
                    draw_rectangle(TICKER_X, ry, TICKER_W, ROW_H, Color::new(0.1, 0.2, 0.32, 0.6));
                    if is_mouse_button_pressed(MouseButton::Left) {
                        clicked = Some(e.target);
                    }
                }
            }
            draw_text(&e.text, TICKER_X + 10.0, ry + 15.0, 16.0, e.color);
        }
        clicked
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sold_out_line_names_venue_and_targets_it() {
        let w = World::new(3, 4);
        let venue = w.city.buildings.iter().find(|b| b.kind.is_food()).unwrap();
        let ev = SimEvent { tick: TICKS_PER_HOUR * 13, kind: EventKind::VenueSoldOut { building: venue.id } };
        let (text, target) = format_event(&w, &ev);
        assert!(text.contains("sold out"), "{text}");
        assert!(text.contains(venue.kind.name()), "{text}");
        assert!(text.contains("D1 13:00"), "{text}");
        assert_eq!(target, Selection::Building(venue.id));
    }

    #[test]
    fn critical_need_line_names_citizen_and_targets_them() {
        let w = World::new(3, 4);
        let ev = SimEvent { tick: 30, kind: EventKind::CriticalNeed { citizen: 2, need: NeedKind::Hunger } };
        let (text, target) = format_event(&w, &ev);
        assert!(text.contains(&w.citizens[2].name), "{text}");
        assert!(text.contains("starving"), "{text}");
        assert_eq!(target, Selection::Citizen(2));
    }

    #[test]
    fn daily_wages_line_has_no_target() {
        let w = World::new(3, 4);
        let ev = SimEvent { tick: crate::sim::time::TICKS_PER_DAY, kind: EventKind::DailyWages { day: 1, total: 1240.0 } };
        let (text, target) = format_event(&w, &ev);
        assert!(text.contains("1240"), "{text}");
        assert!(text.contains("Day 1"), "{text}");
        assert_eq!(target, Selection::None);
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail (module unregistered)**

Run: `cargo test ui::ticker`
Expected: 0 tests run.

- [ ] **Step 4: Register the module**

In `src/ui/mod.rs`:

```rust
pub mod camera;
pub mod hud;
pub mod inspector;
pub mod roster;
pub mod ticker;
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test ui::ticker`
Expected: 3 passed.

- [ ] **Step 6: Commit**

```bash
git add src/ui/ticker.rs src/ui/mod.rs src/ui/inspector.rs
git commit -m "feat: news ticker widget with click-to-select"
```

---

### Task 6: Wire into the main loop and verify end-to-end

**Files:**
- Modify: `src/main.rs:30` (init), `src/main.rs:43-65` (drain + draw)
- Modify: `docs/superpowers/roadmap/2026-06-11-living-world-roadmap.md` (status table)

- [ ] **Step 1: Wire the ticker**

In `src/main.rs`, after the roster init (line 30):

```rust
    let mut ticker = ui::ticker::Ticker::new();
```

After the tick loop (right after the `if steps == 240 { acc = 0.0; }` block), drain once per frame:

```rust
        for ev in world.drain_events() {
            ticker.push(&world, &ev);
        }
```

In the draw section, after the roster click block and before `inspector.draw(...)`:

```rust
        if let Some(sel) = ticker.draw(&mut hud) {
            inspector.selection = sel;
            inspector.follow = false;
        }
```

Draw order matters: `draw_hud` resets `pointer_over_ui` each frame, so the ticker must draw after it (same contract the roster relies on; `handle_click` reads the flag the next frame).

- [ ] **Step 2: Run the full test suite and build**

Run: `cargo test`
Expected: all tests pass — the full pre-existing suite plus the 8 added by this plan.

Run: `cargo build --release`
Expected: clean compile, no warnings about unused `events`.

- [ ] **Step 3: Manual smoke check**

Run: `cargo run --release`
Verify, ideally at 16x speed:
- Ticker rows appear above the bottom-left population strip, right of the roster (critical-need events appear within the first game day; a wage wrap-up at midnight).
- Hovering a row highlights it and does not click through to the world.
- Clicking a citizen row opens that citizen in the inspector; a building row opens the building panel.
- Pausing freezes the ticker.

- [ ] **Step 4: Mark Phase 1 done in the roadmap**

In `docs/superpowers/roadmap/2026-06-11-living-world-roadmap.md`, change the Phase 1 status row:

```markdown
| 1 | Event feed & sim event bus | done |
```

- [ ] **Step 5: Commit**

```bash
git add src/main.rs docs/superpowers/roadmap/2026-06-11-living-world-roadmap.md
git commit -m "feat: wire news ticker into main loop; roadmap phase 1 done"
```
