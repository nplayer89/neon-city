# Sidebar BUSINESSES Tab — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A second tab in the left sidebar listing all workplaces and venues with stock/headcount and balance, with the same hover-preview and click-select behavior as the citizens tab.

**Architecture:** Tabs live inside `Roster` (a `Tab` enum, per-tab scroll, a precomputed `businesses: Vec<u16>` list). `Roster::draw` returns `Option<Selection>` (the inspector's existing enum) so one type covers hovered citizens and buildings; `Inspector::draw`'s preview parameter widens to match. The sim layer is untouched — all new logic is pure presentational helpers in `roster.rs`, unit-tested in the existing style.

**Tech Stack:** Rust, macroquad (immediate-mode UI). Rendering code untested by convention; pure helpers get unit tests.

**Spec:** `docs/superpowers/specs/2026-06-12-business-tab-design.md` — read it first.

**Conventions:**
- Branch: `business-tab` (already created, stacked on `living-world-phase-2` — Phase 2's `balance`/`insolvent`/`WHOLESALE_PRICE` are dependencies).
- NO rustfmt — match the surrounding compact style by hand; never run `cargo fmt`.
- `cargo build` may show transient dead_code warnings for Task 1's helpers until Task 3 consumes them; leave them alone.
- Baseline test count: 63. After Task 1: 66. After Task 3: 67.

---

### Task 1: Pure business-list helpers

**Files:**
- Modify: `src/ui/roster.rs` (imports ~line 1, helpers after `band_color` ~line 65, tests module ~line 154)

- [ ] **Step 1: Write the failing tests** — append to the `tests` module in `src/ui/roster.rs`:

```rust
    #[test]
    fn business_detail_per_kind() {
        use crate::sim::city::BuildingKind;
        assert_eq!(business_detail(BuildingKind::NoodleBar, 17.9, 0), "17m");
        assert_eq!(business_detail(BuildingKind::VendingPlaza, 0.4, 9), "0m");
        assert_eq!(business_detail(BuildingKind::HydroFarm, 5.0, 3), "3w");
        assert_eq!(business_detail(BuildingKind::DataCenter, 0.0, 5), "5w");
        assert_eq!(business_detail(BuildingKind::Arcade, 0.0, 0), "");
    }

    #[test]
    fn business_struggling_rules() {
        use crate::sim::city::BuildingKind::*;
        assert!(business_struggling(HydroFarm, 500.0, true), "insolvent employer");
        assert!(!business_struggling(HydroFarm, 0.0, false), "farms are judged by payroll, not restock");
        assert!(business_struggling(NoodleBar, crate::sim::economy::WHOLESALE_PRICE - 0.01, false), "venue below one meal");
        assert!(!business_struggling(NoodleBar, crate::sim::economy::WHOLESALE_PRICE, false), "boundary: exactly one meal");
        assert!(!business_struggling(Arcade, 0.0, false), "arcades never struggle");
        assert!(!business_struggling(DataCenter, 0.0, false), "industry balance is meaningless");
    }

    #[test]
    fn business_order_grouped_and_complete() {
        use crate::sim::city::BuildingKind;
        let world = crate::sim::world::World::new(2161, 48);
        let order = business_order(&world.city);
        assert_eq!(order.len(), 19, "4 noodle + 3 vending + 3 arcade + 2 farm + 2 fusion + 3 fab + 2 dc");
        let ranks: Vec<usize> = order
            .iter()
            .map(|&id| business_rank(world.city.buildings[id as usize].kind).unwrap())
            .collect();
        assert!(ranks.windows(2).all(|w| w[0] <= w[1]), "not grouped: {ranks:?}");
        for pair in order.windows(2) {
            let same_group = business_rank(world.city.buildings[pair[0] as usize].kind)
                == business_rank(world.city.buildings[pair[1] as usize].kind);
            if same_group {
                assert!(pair[0] < pair[1], "ids not ascending within a group");
            }
        }
        assert!(business_rank(BuildingKind::Apartment).is_none());
        assert!(business_rank(BuildingKind::HoloPark).is_none());
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test business_`
Expected: compile error — `business_detail`, `business_struggling`, `business_order`, `business_rank` don't exist.

- [ ] **Step 3: Implement**

(a) Extend the imports at the top of `src/ui/roster.rs`:

```rust
use crate::sim::citizen::NEED_KINDS;
use crate::sim::city::{BuildingKind, City};
use crate::sim::economy::WHOLESALE_PRICE;
use crate::sim::world::World;
use crate::ui::hud::{over, HudState, CYAN, PANEL, PANEL_EDGE};
use macroquad::prelude::*;
```

(b) Add after `band_color` (before `draw_clipped_text`):

```rust
/// Display order of the BUSINESSES tab: commercial first, then industry.
/// Doubles as the membership filter — kinds not listed don't appear
/// (equivalent to is_workplace() || has_balance(); the order test pins it).
const BUSINESS_KIND_ORDER: [BuildingKind; 7] = [
    BuildingKind::NoodleBar,
    BuildingKind::VendingPlaza,
    BuildingKind::Arcade,
    BuildingKind::HydroFarm,
    BuildingKind::FusionPlant,
    BuildingKind::RoboticsFab,
    BuildingKind::DataCenter,
];

/// Group rank of a kind in the businesses list; None = not a business.
pub fn business_rank(kind: BuildingKind) -> Option<usize> {
    BUSINESS_KIND_ORDER.iter().position(|k| *k == kind)
}

/// Building ids for the BUSINESSES tab: grouped by BUSINESS_KIND_ORDER,
/// id-ascending within a group. Buildings never spawn or despawn mid-run,
/// so this is computed once, like the citizen order.
pub fn business_order(city: &City) -> Vec<u16> {
    let mut ids: Vec<u16> = city
        .buildings
        .iter()
        .filter(|b| business_rank(b.kind).is_some())
        .map(|b| b.id)
        .collect();
    ids.sort_by_key(|&id| (business_rank(city.buildings[id as usize].kind), id));
    ids
}

/// Detail-column text: meals on hand for food venues, headcount for
/// employers, blank for arcades.
pub fn business_detail(kind: BuildingKind, stock: f32, workers: usize) -> String {
    if kind.is_food() {
        format!("{}m", stock.floor())
    } else if kind.is_workplace() {
        format!("{workers}w")
    } else {
        String::new()
    }
}

/// Red-balance rule: an employer that missed payroll, or a food venue that
/// can't afford its next wholesale meal. Industry (no books yet) only trips
/// via the insolvent flag; arcades never struggle.
pub fn business_struggling(kind: BuildingKind, balance: f32, insolvent: bool) -> bool {
    (kind.is_workplace() && insolvent) || (kind.is_food() && balance < WHOLESALE_PRICE)
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test`
Expected: 66 passed (was 63).

- [ ] **Step 5: Commit**

```bash
git add src/ui/roster.rs
git commit -m "feat: pure helpers for the sidebar businesses list"
```

---

### Task 2: Widen the inspector preview to any Selection (behavior-neutral)

`Inspector::draw` previews only citizens today. Widening `preview` to `Option<Selection>` now lets Task 3 plug business rows in without touching the inspector again. After this task the app behaves identically.

**Files:**
- Modify: `src/ui/inspector.rs` (`draw` ~line 67)
- Modify: `src/main.rs` (the `inspector.draw(...)` call, ~line 81)

- [ ] **Step 1: Implement**

(a) In `src/ui/inspector.rs`, replace `draw`'s doc comment, signature, and preview block. Current:

```rust
    /// `preview`: a citizen to show instead of the selection (roster hover).
    /// Previewing never alters the selection or follow state.
    pub fn draw(&mut self, world: &World, cam: &mut Camera, hud: &mut HudState, preview: Option<usize>) {
```
and inside, the preview block:
```rust
        if let Some(id) = preview {
            self.draw_citizen_panel(world, cam, hud, id, true);
            return;
        }
```

New:

```rust
    /// `preview`: an entity to show instead of the selection (roster hover).
    /// Previewing never alters the selection or follow state.
    pub fn draw(&mut self, world: &World, cam: &mut Camera, hud: &mut HudState, preview: Option<Selection>) {
```
and:
```rust
        match preview {
            Some(Selection::Citizen(id)) => {
                self.draw_citizen_panel(world, cam, hud, id, true);
                return;
            }
            Some(Selection::Building(id)) => {
                self.draw_building_panel(world, hud, id);
                return;
            }
            Some(Selection::None) | None => {}
        }
```

(The follow-centering block above it and the `match self.selection` below it are unchanged — follow stays tied to the selection, never the preview.)

(b) In `src/main.rs`, change only the inspector draw call:

```rust
        inspector.draw(&world, &mut cam, &mut hud, hovered.map(ui::inspector::Selection::Citizen));
```

(`hovered` is still `Option<usize>` from the roster until Task 3; `.map` shims it.)

- [ ] **Step 2: Verify**

Run: `cargo build` — clean (no new warnings beyond Task 1's transient ones), then `cargo test` — 66 passed.

- [ ] **Step 3: Commit**

```bash
git add src/ui/inspector.rs src/main.rs
git commit -m "refactor: inspector preview accepts any Selection"
```

---

### Task 3: Tab strip, business rows, widened roster return

**Files:**
- Modify: `src/ui/roster.rs` (consts ~line 17, `Roster` struct + `new` + `draw` ~lines 80–151, tests)
- Modify: `src/main.rs` (roster click wiring ~lines 70–76, inspector draw call)

- [ ] **Step 1: Write the failing test** — append to the `tests` module in `src/ui/roster.rs`:

```rust
    #[test]
    fn roster_carries_business_list_and_defaults_to_citizens() {
        let world = crate::sim::world::World::new(2161, 48);
        let r = Roster::new(&world);
        assert_eq!(r.businesses, business_order(&world.city));
        assert_eq!(r.tab, Tab::Citizens);
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test roster_carries`
Expected: compile error — `businesses`, `tab`, `Tab` don't exist.

- [ ] **Step 3: Implement** — in `src/ui/roster.rs`:

(a) Add an import for the selection type (mutual `ui` module imports are fine — `inspector.rs` already imports `roster::draw_clipped_text`):

```rust
use crate::ui::inspector::Selection;
```

(b) Add below `MONEY_COL_W`:

```rust
/// Reserved width for the business detail column ("17m" / "7w"), left of the balance.
const DETAIL_COL_W: f32 = 34.0;
```

(c) Replace the `Roster` struct and `new`:

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tab {
    Citizens,
    Businesses,
}

pub struct Roster {
    /// Citizen ids sorted alphabetically by name; the population is fixed
    /// after world creation, so this is computed once.
    order: Vec<usize>,
    /// Building ids for the BUSINESSES tab (see business_order).
    businesses: Vec<u16>,
    tab: Tab,
    /// Scroll offsets, kept per tab so switching back doesn't lose your place.
    scroll: f32,
    business_scroll: f32,
}

impl Roster {
    pub fn new(world: &World) -> Roster {
        let mut order: Vec<usize> = (0..world.citizens.len()).collect();
        order.sort_by(|&a, &b| world.citizens[a].name.cmp(&world.citizens[b].name));
        Roster {
            order,
            businesses: business_order(&world.city),
            tab: Tab::Citizens,
            scroll: 0.0,
            business_scroll: 0.0,
        }
    }
```

(d) Replace the entire `draw` method with the tab-aware version, and add the three private helpers below it (still inside `impl Roster`):

```rust
    /// Draws the sidebar. Returns (hovered row's selection, clicked this frame).
    /// Sets `hud.pointer_over_ui` when the pointer is over the sidebar.
    pub fn draw(&mut self, world: &World, hud: &mut HudState) -> (Option<Selection>, bool) {
        let (x, y, w) = (0.0, TOP, SIDEBAR_W);
        let h = screen_height() - TOP - BOTTOM_MARGIN;
        let hovering_panel = over(x, y, w, h);
        if hovering_panel {
            hud.pointer_over_ui = true;
        }

        draw_rectangle(x, y, w, h, PANEL);
        draw_rectangle_lines(x, y, w, h, 1.5, PANEL_EDGE);
        self.draw_tabs(x, y);

        let count = match self.tab {
            Tab::Citizens => self.order.len(),
            Tab::Businesses => self.businesses.len(),
        };
        let list_top = y + HEADER_H;
        let list_h = h - HEADER_H;
        let max_scroll = (count as f32 * ROW_H - list_h).max(0.0);
        let mut scroll = match self.tab {
            Tab::Citizens => self.scroll,
            Tab::Businesses => self.business_scroll,
        };
        if hovering_panel {
            let wheel = mouse_wheel().1;
            if wheel.abs() > 0.0 {
                scroll -= wheel.signum() * ROW_H * 3.0;
            }
        }
        scroll = scroll.clamp(0.0, max_scroll);
        match self.tab {
            Tab::Citizens => self.scroll = scroll,
            Tab::Businesses => self.business_scroll = scroll,
        }

        let (_, my) = mouse_position();
        let mut hovered: Option<Selection> = None;

        // No row hover while a left-drag is in progress (map pans sweeping
        // across the sidebar), except on the press frame so clicks register.
        let hover_enabled = !is_mouse_button_down(MouseButton::Left) || is_mouse_button_pressed(MouseButton::Left);

        for i in 0..count {
            let ry = list_top + i as f32 * ROW_H - scroll;
            // Partially clipped rows are skipped (no scissor in macroquad 2D).
            if ry < list_top || ry + ROW_H > y + h + 0.5 {
                continue;
            }
            let row_hovered = hover_enabled && hovering_panel && my >= ry && my < ry + ROW_H;
            if row_hovered {
                draw_rectangle(x, ry, w, ROW_H, Color::new(0.2, 0.9, 1.0, 0.12));
            }
            let sel = match self.tab {
                Tab::Citizens => {
                    self.draw_citizen_row(world, self.order[i], x, w, ry);
                    Selection::Citizen(self.order[i])
                }
                Tab::Businesses => {
                    self.draw_business_row(world, self.businesses[i], x, w, ry);
                    Selection::Building(self.businesses[i])
                }
            };
            if row_hovered {
                hovered = Some(sel);
            }
        }

        let clicked = hovered.is_some() && is_mouse_button_pressed(MouseButton::Left);
        (hovered, clicked)
    }

    /// Header tab strip: active label cyan with an underline, inactive dim.
    /// Tab clicks land in the header band, above list_top, so they can never
    /// double as row clicks.
    fn draw_tabs(&mut self, x: f32, y: f32) {
        let labels = [(Tab::Citizens, "CITIZENS"), (Tab::Businesses, "BUSINESSES")];
        let mut lx = x + 10.0;
        for (tab, label) in labels {
            let tw = measure_text(label, None, 18, 1.0).width;
            let active = self.tab == tab;
            let color = if active { CYAN } else { Color::new(0.45, 0.6, 0.75, 0.8) };
            draw_text(label, lx, y + 19.0, 18.0, color);
            if active {
                draw_line(lx, y + 23.0, lx + tw, y + 23.0, 2.0, CYAN);
            }
            if over(lx - 4.0, y + 4.0, tw + 8.0, HEADER_H - 6.0) && is_mouse_button_pressed(MouseButton::Left) {
                self.tab = tab;
            }
            lx += tw + 16.0;
        }
    }

    fn draw_citizen_row(&self, world: &World, id: usize, x: f32, w: f32, ry: f32) {
        let icons_x = x + w - 6.0 - NEED_KINDS.len() as f32 * ICON_STRIDE;
        let name_max_w = icons_x - MONEY_COL_W - (x + 10.0) - 4.0;
        let c = &world.citizens[id];
        draw_clipped_text(&c.name, x + 10.0, ry + 11.5, 15, name_max_w, Color::new(0.8, 0.9, 1.0, 0.9));
        let money_text = money_label(c.money);
        let mw = measure_text(&money_text, None, 13, 1.0).width;
        draw_text(&money_text, icons_x - 4.0 - mw, ry + 11.5, 13.0, band_color(money_band(c.money)));
        for (j, k) in NEED_KINDS.iter().enumerate() {
            let color = band_color(band(c.needs.get(*k)));
            draw_rectangle(icons_x + j as f32 * ICON_STRIDE, ry + 4.0, ICON_SIZE, ICON_SIZE, color);
        }
    }

    fn draw_business_row(&self, world: &World, id: u16, x: f32, w: f32, ry: f32) {
        let b = &world.city.buildings[id as usize];
        let balance_right = x + w - 6.0;
        let detail_right = balance_right - MONEY_COL_W;
        let name_max_w = detail_right - DETAIL_COL_W - (x + 10.0) - 4.0;

        let name = format!("{} #{}", b.kind.name(), b.id);
        draw_clipped_text(&name, x + 10.0, ry + 11.5, 15, name_max_w, Color::new(0.8, 0.9, 1.0, 0.9));

        let detail = business_detail(b.kind, b.stock, b.workers.len());
        if !detail.is_empty() {
            let dw = measure_text(&detail, None, 13, 1.0).width;
            draw_text(&detail, detail_right - dw, ry + 11.5, 13.0, Color::new(0.6, 0.75, 0.9, 0.9));
        }

        let balance_text = if b.kind.has_balance() { money_label(b.balance) } else { "-".to_string() };
        let bw = measure_text(&balance_text, None, 13, 1.0).width;
        let color = if business_struggling(b.kind, b.balance, b.insolvent) {
            band_color(Band::Low)
        } else {
            Color::new(0.8, 0.9, 1.0, 0.9)
        };
        draw_text(&balance_text, balance_right - bw, ry + 11.5, 13.0, color);
    }
```

(The old inline citizen row-loop body moves verbatim into `draw_citizen_row`; the only behavior change on the citizens tab is none.)

(e) In `src/main.rs`, the roster wiring becomes:

```rust
        let (hovered, roster_clicked) = roster.draw(&world, &mut hud);
        if roster_clicked {
            if let Some(sel) = hovered {
                inspector.selection = sel;
                inspector.follow = false;
            }
        }
```

and the inspector call drops the Task 2 shim:

```rust
        inspector.draw(&world, &mut cam, &mut hud, hovered);
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test`
Expected: 67 passed. `cargo build` — zero warnings (Task 1's helpers are now consumed).

- [ ] **Step 5: Commit**

```bash
git add src/ui/roster.rs src/main.rs
git commit -m "feat: sidebar BUSINESSES tab with hover preview and click-select"
```

---

### Task 4: Full verification

- [ ] **Step 1: Suite + build**

Run: `cargo test` (expect 67 passed) and `cargo build` (expect zero warnings).

- [ ] **Step 2: Visual verification** (controller does this via the headless web harness — `./build_web.sh`, serve `web/`, drive with playwright):
- Boot: sidebar shows CITIZENS active, BUSINESSES dim.
- Click BUSINESSES: 19 rows, grouped (4 noodle bars first, data centers last); food venues show `Nm` + balance, farms/industry show `Nw`, industry balance is `-`, arcades have a blank detail.
- Hover a business row: inspector previews the building panel without changing the selection.
- Click a business row: building selected (map outline + panel), matching a map click.
- Click CITIZENS: original list intact, scroll preserved per tab.

- [ ] **Step 3: Finish the branch** — superpowers:finishing-a-development-branch (PR base: `living-world-phase-2`, since this stacks on the open Phase 2 PR #3).

---

## Verification against spec (for the reviewer)

| Spec requirement | Task |
|---|---|
| 19 workplaces+venues, grouped order, Hab/Park excluded | 1 |
| Detail column (stock `m` / workers `w` / blank) | 1, 3 |
| Struggling-red rule (insolvent employer; venue < WHOLESALE_PRICE) | 1, 3 |
| Tab strip, per-tab scroll, default CITIZENS | 3 |
| Row layout: name `#id` \| detail \| balance, `-` for industry | 3 |
| Hover previews building panel; click selects (Selection widening) | 2, 3 |
| Sim layer untouched | all (no `src/sim` edits) |
| Unit tests for pure helpers; rendering verified visually | 1, 3, 4 |
