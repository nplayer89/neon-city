# Citizen Roster Sidebar Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A left sidebar listing every citizen (name + four color-banded need icons) with hover showing the full citizen panel and click selecting the citizen.

**Architecture:** New `src/ui/roster.rs` module draws the sidebar and reports hover/click; `src/ui/inspector.rs` gains a `preview` parameter that reuses the existing citizen panel for the hovered citizen; `src/main.rs` wires them together. `src/sim/` is untouched.

**Tech Stack:** Rust, macroquad 0.4 (immediate-mode rendering). Spec: `docs/superpowers/specs/2026-06-11-citizen-roster-sidebar-design.md`.

**Conventions:** This is a binary crate; `cargo test` compiles everything including `ui/`. UI modules may not call windowing functions (`screen_width`, `draw_*`, `measure_text`) from unit tests or constructors that tests exercise — pure logic only in tested code paths. Run all commands from the repo root.

---

### Task 1: Status band function

**Files:**
- Create: `src/ui/roster.rs`
- Modify: `src/ui/mod.rs`

- [ ] **Step 1: Create the module with a failing test**

Create `src/ui/roster.rs` containing ONLY the test (the `band` function does not exist yet):

```rust
use macroquad::prelude::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn band_boundaries() {
        assert_eq!(band(0.0), Band::Low);
        assert_eq!(band(0.29), Band::Low);
        assert_eq!(band(0.3), Band::Medium);
        assert_eq!(band(0.59), Band::Medium);
        assert_eq!(band(0.6), Band::High);
        assert_eq!(band(1.0), Band::High);
    }
}
```

Register the module — in `src/ui/mod.rs` replace the whole file with:

```rust
pub mod camera;
pub mod hud;
pub mod inspector;
pub mod roster;
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test band_boundaries`
Expected: compile error — `cannot find function 'band' in this scope` / `cannot find type 'Band'`. (In Rust, a test referencing missing items fails at compile time; that is the red state.)

- [ ] **Step 3: Implement the band function**

Add above the `#[cfg(test)]` block in `src/ui/roster.rs`:

```rust
/// Discrete status band for a need value in [0, 1].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Band {
    High,
    Medium,
    Low,
}

/// Pure (no macroquad) so it stays unit-testable.
pub fn band(value: f32) -> Band {
    if value >= 0.6 {
        Band::High
    } else if value >= 0.3 {
        Band::Medium
    } else {
        Band::Low
    }
}

fn band_color(b: Band) -> Color {
    match b {
        Band::High => Color::new(0.3, 0.95, 0.5, 1.0),
        Band::Medium => Color::new(0.95, 0.8, 0.25, 1.0),
        Band::Low => Color::new(1.0, 0.25, 0.4, 1.0),
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test band_boundaries`
Expected: `test ui::roster::tests::band_boundaries ... ok` — 1 passed. A `band_color is never used` dead-code warning is expected until Task 3 and is fine.

- [ ] **Step 5: Commit**

```bash
git add src/ui/roster.rs src/ui/mod.rs
git commit -m "feat: add need status band function for roster sidebar"
```

---

### Task 2: Roster struct with alphabetical order

**Files:**
- Modify: `src/ui/roster.rs`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `src/ui/roster.rs`:

```rust
    #[test]
    fn roster_order_is_alphabetical_and_complete() {
        let world = crate::sim::world::World::new(2161, 48);
        let r = Roster::new(&world);
        assert_eq!(r.order.len(), world.citizens.len());
        for pair in r.order.windows(2) {
            assert!(
                world.citizens[pair[0]].name <= world.citizens[pair[1]].name,
                "roster not sorted: {} before {}",
                world.citizens[pair[0]].name,
                world.citizens[pair[1]].name
            );
        }
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test roster_order`
Expected: compile error — `cannot find struct, variant or union type 'Roster'`.

- [ ] **Step 3: Implement Roster::new**

Add above the `#[cfg(test)]` block in `src/ui/roster.rs` (note: `new` must not call any macroquad function — the test runs without a window):

```rust
use crate::sim::world::World;

pub struct Roster {
    /// Citizen ids sorted alphabetically by name; the population is fixed
    /// after world creation, so this is computed once.
    order: Vec<usize>,
    scroll: f32,
}

impl Roster {
    pub fn new(world: &World) -> Roster {
        let mut order: Vec<usize> = (0..world.citizens.len()).collect();
        order.sort_by(|&a, &b| world.citizens[a].name.cmp(&world.citizens[b].name));
        Roster { order, scroll: 0.0 }
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test roster`
Expected: 2 passed (`band_boundaries`, `roster_order_is_alphabetical_and_complete`). Dead-code warnings for `Roster`/`scroll` are expected until Task 3.

- [ ] **Step 5: Commit**

```bash
git add src/ui/roster.rs
git commit -m "feat: add Roster with alphabetical citizen order"
```

---

### Task 3: Roster::draw rendering

Rendering is immediate-mode macroquad and not unit-testable; this task is verified by compilation now and manually in Task 6.

**Files:**
- Modify: `src/ui/roster.rs`

- [ ] **Step 1: Add imports and layout constants**

Replace the existing `use` lines at the top of `src/ui/roster.rs` with:

```rust
use crate::sim::citizen::NEED_KINDS;
use crate::sim::world::World;
use crate::ui::hud::{over, HudState, CYAN, PANEL, PANEL_EDGE};
use macroquad::prelude::*;

const SIDEBAR_W: f32 = 200.0;
/// Below the 52 px top bar.
const TOP: f32 = 52.0;
/// Stops above the bottom-left population strip.
const BOTTOM_MARGIN: f32 = 32.0;
const HEADER_H: f32 = 28.0;
const ROW_H: f32 = 15.0;
/// 4 icons, 7 px each with 4 px gaps, 6 px right padding.
const ICON_STRIDE: f32 = 11.0;
const ICON_SIZE: f32 = 7.0;
```

- [ ] **Step 2: Implement draw and the name-clipping helper**

Add inside `impl Roster` (after `new`):

```rust
    /// Draws the sidebar. Returns (hovered citizen id, clicked this frame).
    /// Sets `hud.pointer_over_ui` when the pointer is over the sidebar.
    pub fn draw(&mut self, world: &World, hud: &mut HudState) -> (Option<usize>, bool) {
        let (x, y, w) = (0.0, TOP, SIDEBAR_W);
        let h = screen_height() - TOP - BOTTOM_MARGIN;
        let hovering_panel = over(x, y, w, h);
        if hovering_panel {
            hud.pointer_over_ui = true;
        }

        draw_rectangle(x, y, w, h, PANEL);
        draw_rectangle_lines(x, y, w, h, 1.5, PANEL_EDGE);
        draw_text("CITIZENS", x + 10.0, y + 19.0, 18.0, CYAN);

        let list_top = y + HEADER_H;
        let list_h = h - HEADER_H;
        let max_scroll = (self.order.len() as f32 * ROW_H - list_h).max(0.0);
        if hovering_panel {
            let wheel = mouse_wheel().1;
            if wheel.abs() > 0.0 {
                self.scroll -= wheel.signum() * ROW_H * 3.0;
            }
        }
        self.scroll = self.scroll.clamp(0.0, max_scroll);

        let (_, my) = mouse_position();
        let icons_x = x + w - 6.0 - NEED_KINDS.len() as f32 * ICON_STRIDE;
        let name_max_w = icons_x - (x + 10.0) - 4.0;
        let mut hovered: Option<usize> = None;

        for (i, &id) in self.order.iter().enumerate() {
            let ry = list_top + i as f32 * ROW_H - self.scroll;
            // Partially clipped rows are skipped (no scissor in macroquad 2D).
            if ry < list_top || ry + ROW_H > y + h {
                continue;
            }
            if hovering_panel && my >= ry && my < ry + ROW_H {
                hovered = Some(id);
                draw_rectangle(x, ry, w, ROW_H, Color::new(0.2, 0.9, 1.0, 0.12));
            }
            let c = &world.citizens[id];
            draw_clipped_text(&c.name, x + 10.0, ry + 11.5, 15, name_max_w, Color::new(0.8, 0.9, 1.0, 0.9));
            for (j, k) in NEED_KINDS.iter().enumerate() {
                let color = band_color(band(c.needs.get(*k)));
                draw_rectangle(icons_x + j as f32 * ICON_STRIDE, ry + 4.0, ICON_SIZE, ICON_SIZE, color);
            }
        }

        let clicked = hovered.is_some() && is_mouse_button_pressed(MouseButton::Left);
        (hovered, clicked)
    }
```

Add as a free function (below `band_color`):

```rust
/// Draws text clipped to `max_w` so long names never run under the icons.
fn draw_clipped_text(text: &str, x: f32, y: f32, font_px: u16, max_w: f32, color: Color) {
    if measure_text(text, None, font_px, 1.0).width <= max_w {
        draw_text(text, x, y, font_px as f32, color);
        return;
    }
    let mut s = text.to_string();
    while !s.is_empty() && measure_text(&s, None, font_px, 1.0).width > max_w {
        s.pop();
    }
    draw_text(&s, x, y, font_px as f32, color);
}
```

- [ ] **Step 3: Verify it compiles and tests stay green**

Run: `cargo build && cargo test`
Expected: build succeeds (a `draw is never used` warning is expected until Task 5); all tests pass (33 total: 31 sim + 2 roster).

- [ ] **Step 4: Commit**

```bash
git add src/ui/roster.rs
git commit -m "feat: draw roster sidebar with banded need icons"
```

---

### Task 4: Inspector preview mode

Adds a `preview` parameter to `Inspector::draw` while keeping behavior identical (main passes `None` until Task 5).

**Files:**
- Modify: `src/ui/inspector.rs`
- Modify: `src/main.rs` (call site only)

- [ ] **Step 1: Change Inspector::draw to accept a preview citizen**

In `src/ui/inspector.rs`, replace the existing `draw` method:

```rust
    pub fn draw(&mut self, world: &World, cam: &mut Camera, hud: &mut HudState) {
        match self.selection {
            Selection::None => {}
            Selection::Citizen(id) => self.draw_citizen_panel(world, cam, hud, id),
            Selection::Building(id) => self.draw_building_panel(world, hud, id),
        }
    }
```

with:

```rust
    /// `preview`: a citizen to show instead of the selection (roster hover).
    /// Previewing never alters the selection or follow state.
    pub fn draw(&mut self, world: &World, cam: &mut Camera, hud: &mut HudState, preview: Option<usize>) {
        // Follow-centering stays tied to the selection even while previewing,
        // so hovering a roster row never yanks a followed camera.
        if self.follow {
            if let Selection::Citizen(id) = self.selection {
                cam.center = world.citizens[id].pos;
            }
        }
        if let Some(id) = preview {
            self.draw_citizen_panel(world, cam, hud, id, true);
            return;
        }
        match self.selection {
            Selection::None => {}
            Selection::Citizen(id) => self.draw_citizen_panel(world, cam, hud, id, false),
            Selection::Building(id) => self.draw_building_panel(world, hud, id),
        }
    }
```

- [ ] **Step 2: Add the preview flag to draw_citizen_panel**

In `src/ui/inspector.rs`, change the signature:

```rust
    fn draw_citizen_panel(&mut self, world: &World, cam: &mut Camera, hud: &mut HudState, id: usize) {
```

to:

```rust
    fn draw_citizen_panel(&mut self, world: &World, cam: &mut Camera, hud: &mut HudState, id: usize, preview: bool) {
```

Then replace the follow-button/centering block near the end of that method:

```rust
        let mut ui_hit = hud.pointer_over_ui;
        if button(x + 14.0, by, 110.0, 30.0, if self.follow { "FOLLOWING" } else { "FOLLOW" }, self.follow, &mut ui_hit) {
            self.follow = !self.follow;
        }
        hud.pointer_over_ui = ui_hit;

        if self.follow {
            cam.center = world.citizens[id].pos;
        }
```

with (FOLLOW is unclickable during preview since the pointer is on the roster, so it is hidden; centering moved to `draw`):

```rust
        if !preview {
            let mut ui_hit = hud.pointer_over_ui;
            if button(x + 14.0, by, 110.0, 30.0, if self.follow { "FOLLOWING" } else { "FOLLOW" }, self.follow, &mut ui_hit) {
                self.follow = !self.follow;
            }
            hud.pointer_over_ui = ui_hit;
        }
```

The in-world cyan marker ring at the end of the method stays as-is — it now marks the previewed citizen too, which is the spec's intent.

- [ ] **Step 3: Update the call site to pass None**

In `src/main.rs`, change:

```rust
        inspector.draw(&world, &mut cam, &mut hud);
```

to:

```rust
        inspector.draw(&world, &mut cam, &mut hud, None);
```

- [ ] **Step 4: Verify it compiles and tests stay green**

Run: `cargo build && cargo test`
Expected: build succeeds, 33 tests pass. Behavior is unchanged (preview is always `None` so far).

- [ ] **Step 5: Commit**

```bash
git add src/ui/inspector.rs src/main.rs
git commit -m "feat: add hover-preview mode to inspector citizen panel"
```

---

### Task 5: Wire the roster into the main loop

**Files:**
- Modify: `src/main.rs`
- Modify: `src/ui/camera.rs`

- [ ] **Step 1: Create the roster at startup**

In `src/main.rs`, after `let mut inspector = ui::inspector::Inspector::new();` add:

```rust
    let mut roster = ui::roster::Roster::new(&world);
```

- [ ] **Step 2: Draw the roster and feed hover/click to the inspector**

Replace the end of the frame (after `render::draw_world(...)`):

```rust
        ui::hud::draw_hud(&world, &mut hud);
        inspector.draw(&world, &mut cam, &mut hud, None);
        next_frame().await
```

with (order matters: `draw_hud` resets `pointer_over_ui`, the roster sets it, the inspector draws last so the preview panel sits on top):

```rust
        ui::hud::draw_hud(&world, &mut hud);
        let (hovered, roster_clicked) = roster.draw(&world, &mut hud);
        if roster_clicked {
            if let Some(id) = hovered {
                inspector.selection = ui::inspector::Selection::Citizen(id);
                inspector.follow = false;
            }
        }
        inspector.draw(&world, &mut cam, &mut hud, hovered);
        next_frame().await
```

No change to `inspector.handle_click` is needed for the common case: it already ignores clicks when `hud.pointer_over_ui` was set (by the roster) on the previous frame — the same one-frame-lag convention the rest of the UI uses.

- [ ] **Step 3: Guard world-click detection against presses that began over UI**

Code review found one gesture that slips through: press on a roster row (selects the citizen), hold, drag onto the map, release over empty ground. On the release frame `pointer_over_ui` is false and `cam.dragged` is stale from the previous gesture, so `inspector.handle_click` runs and may set `Selection::None`, undoing the selection the roster just made.

In `src/ui/camera.rs`, replace:

```rust
        if is_mouse_button_pressed(MouseButton::Left) && !ui_hover {
            self.drag_anchor = Some((mx, my));
            self.dragged = false;
        }
```

with:

```rust
        if is_mouse_button_pressed(MouseButton::Left) {
            if ui_hover {
                // Press began over UI: poison the gesture so release-frame
                // readers (inspector click detection) ignore it even if the
                // pointer leaves the UI before release.
                self.drag_anchor = None;
                self.dragged = true;
            } else {
                self.drag_anchor = Some((mx, my));
                self.dragged = false;
            }
        }
```

Also extend the doc comment on the `dragged` field:

```rust
    /// True if the most recent press turned into a drag. Also set when a
    /// press begins over UI, so that gesture never reads as a world click.
    /// Deliberately NOT cleared on release: release-frame readers
    /// (click-vs-drag detection in the inspector, which runs after
    /// Camera::update) rely on it. It resets on the next press. Do not read
    /// it outside a release-frame context.
    pub dragged: bool,
```

- [ ] **Step 4: Verify it compiles and tests stay green**

Run: `cargo build && cargo test`
Expected: build succeeds with no dead-code warnings from `roster.rs`; 33 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs src/ui/camera.rs
git commit -m "feat: wire citizen roster sidebar into main loop"
```

---

### Task 6: Full verification

**Files:** none (verification only)

- [ ] **Step 1: Run the full test suite**

Run: `cargo test`
Expected: 33 passed, 0 failed.

- [ ] **Step 2: Manual verification in the running app**

Run: `cargo run --release` and check each item:

1. Left sidebar lists all 48 citizens alphabetically with 4 colored squares per row; colors change over time as needs drain (green → amber → magenta-red).
2. All 48 rows are visible at the default window size (no scrolling needed).
3. Hovering a row highlights it, shows that citizen's full panel on the right (no FOLLOW button), and draws a cyan ring at their map position; moving the mouse off restores the previous state (selected citizen's panel, or none).
4. Clicking a row pins that citizen's panel (FOLLOW button present); FOLLOW then tracks them. While following, hovering other rows previews them without moving the camera.
5. With the pointer over the sidebar: map does not pan when dragging, does not zoom when scrolling, and clicks do not select dots/buildings through the sidebar.
6. Shrink the window vertically: the wheel scrolls the list over the sidebar, clamped at both ends.
7. The bottom-left `POP/EMPLOYED/SEED` strip is not covered by the sidebar.
8. Press a roster row, hold the button, drag onto the map, release over empty ground: the selection made on press survives (no deselect), and the map does not pan during that gesture.

- [ ] **Step 3: Web build sanity check (optional but cheap)**

Run: `./build_web.sh`
Expected: builds without errors (`web/neon_city.wasm` updated).
