# Citizen Roster Sidebar — Design

**Date:** 2026-06-11
**Status:** Approved

## Goal

A left sidebar listing every citizen in town: name plus four small color-coded
icons (one per need) showing at a glance whether each stat is high, medium, or
low. Hovering a row shows the citizen's full detail panel — the same panel that
opens today when clicking their dot in the world. Clicking a row selects the
citizen, exactly like clicking the dot.

## Architecture

New module `src/ui/roster.rs` owning the sidebar; the existing inspector gains
a small "preview" path that reuses `draw_citizen_panel` for the hovered
citizen. No changes to `src/sim/` — this is purely `ui/` plus `main.rs`
wiring, consistent with the one-concern-per-module pattern in `src/ui/`
(`camera`, `hud`, `inspector`).

## Components

### `Roster` (new, `src/ui/roster.rs`)

- **State:** sorted citizen id list (alphabetical by name, computed once in
  `Roster::new(&world)` since the population is fixed after world creation)
  and a scroll offset.
- **`draw(&mut self, world, hud) -> (Option<usize>, bool)`:** draws the
  sidebar and returns `(hovered citizen id, clicked)` — `clicked` is true when
  the hovered row was clicked this frame. Sets `hud.pointer_over_ui` when the
  pointer is over the sidebar. On click, the caller (main loop) sets
  `inspector.selection = Selection::Citizen(id)` and clears `follow` — the
  roster itself does not reach into the inspector.

### Layout

- Sidebar anchored at the left edge: `x = 0`, from below the top bar
  (`y = 52`) down to just above the bottom population strip
  (`height = screen_height() − 52 − 32`), width **200 px**, drawn in the
  existing `PANEL` fill with `PANEL_EDGE` border, with a 28 px "CITIZENS"
  header at the top.
- One row per citizen, **15 px** tall (48 rows × 15 px = 720 px fits in the
  748 px list area at the default 860 px window height; 16 px would not):
  name (font 15) left-aligned, four
  **7 px** squares right-aligned with 4 px gaps, one per need in `NEED_KINDS`
  order (hunger, energy, hygiene, fun).
- The hovered row gets a subtle highlight rectangle so the hover target is
  visible. Names wide enough to reach the icon block are clipped, never drawn
  under the icons (with the current name pool and font size none are, but the
  guard is cheap).

### Status bands

A pure function maps a need value to a band:

| Band   | Range            | Color                          |
|--------|------------------|--------------------------------|
| High   | value ≥ 0.6      | green `(0.3, 0.95, 0.5)`       |
| Medium | 0.3 ≤ value < 0.6| amber `(0.95, 0.8, 0.25)`      |
| Low    | value < 0.3      | magenta-red `(1.0, 0.25, 0.4)` |

The AI treats `min need < 0.15` as critical (`src/sim/ai.rs`), so the Low band
doubles as an early warning. The band function takes and returns plain values
(no macroquad types) so it is unit-testable; colors are looked up at draw time.

### Inspector preview (modified, `src/ui/inspector.rs`)

- `Inspector::draw` gains a `preview: Option<usize>` parameter. When `Some(id)`,
  the citizen panel is drawn for that citizen in its usual right-side position,
  visually replacing the selection's panel for the duration of the hover. When
  `None`, behavior is unchanged.
- In preview mode the FOLLOW button is not drawn (the pointer is over the
  roster, so it could never be clicked), and follow-centering remains tied to
  the *selected* citizen so hovering a row never yanks a followed camera.
- The cyan in-world marker ring is drawn around the previewed citizen so they
  can be located on the map. Citizens currently `Performing` (inside a
  building) are still listed and previewable; the ring marks their position at
  the building.

### Scrolling

If rows overflow the sidebar height (only when the window is shorter than the
default 860 px — at 1360×860 all 48 rows fit), the mouse wheel scrolls the
list while the pointer is over the sidebar, clamped to the content range.
Camera zoom is already suppressed over UI via `hud.pointer_over_ui`, with the
same one-frame lag the rest of the UI accepts.

## Data flow (per frame)

1. `draw_hud` resets `pointer_over_ui` (existing).
2. `roster.draw(&world, &mut hud)` → hovered id + clicked flag; sets
   `pointer_over_ui` when over the sidebar.
3. Main loop applies a row click to `inspector.selection`.
4. `inspector.draw(&world, &mut cam, &mut hud, hovered)` draws preview or
   selection panel.

## Error handling

No new failure modes: the citizen list is non-empty by construction, the
scroll offset is clamped, and out-of-range ids cannot occur because row ids
come from the same `world.citizens` indices used everywhere else.

## Testing

- Unit test the band function boundaries (0.3 / 0.6, and edge values 0.0,
  1.0) in `roster.rs`.
- `cargo test` — all 31 existing sim tests must stay green (sim untouched).
- Manual verification: `cargo run --release`; confirm hover preview, click
  select, follow interaction, no click-through to the map, and wheel scroll in
  a shrunken window.
