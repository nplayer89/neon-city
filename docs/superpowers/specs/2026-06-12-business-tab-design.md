# Sidebar BUSINESSES Tab — Design

**Date:** 2026-06-12
**Status:** Approved
**Source:** User request after Phase 2 (business wallets): a second sidebar tab
showing business info the way the citizens roster shows citizens.

## Context

The left sidebar (`src/ui/roster.rs`, 240 px) lists all citizens: clipped name,
band-colored wallet, four need icons; hovering previews the citizen in the
inspector and clicking selects them. Phase 2 gave buildings balances, stock,
workers, and an insolvency flag — but the only way to see them is clicking
buildings one at a time on the map. This feature adds a BUSINESSES tab to the
same sidebar for an at-a-glance economic scan.

## Decisions (resolved during brainstorm)

| Decision | Outcome |
|---|---|
| Which buildings | **All workplaces + venues** (19): Noodle Bar, Vending Plaza, Holo Arcade, Hydro Farm, Fusion Plant, Robotics Fab, Data Center. Excludes Hab Blocks and Holo Parks (no business info). |
| Row layout | **Name `#id` \| detail \| balance.** Detail = stock (`17m`) for food venues, workers (`3w`) for employers, blank for arcades. Balance right-aligned via the existing `money_label`; industry (no books yet) shows `-`. |
| Struggling signal | Balance text turns **red** when an employer is `insolvent` or a food venue can't afford one wholesale meal (`balance < WHOLESALE_PRICE`); neutral otherwise. No yellow band — businesses don't have a meaningful "medium". |
| Architecture | Tabs live **inside `Roster`** (Tab enum + per-tab scroll). Rejected: a separate BusinessList struct (splits sidebar ownership), stacked sections (user asked for tabs). |

## Design

### Tab strip

The sidebar header row becomes two clickable labels: `CITIZENS` and
`BUSINESSES`. The active tab draws in `CYAN`, the inactive one dim
(`Color::new(0.45, 0.6, 0.75, 0.8)`); clicking a label switches tabs.
The header already sits inside the panel, so `pointer_over_ui` handling is
unchanged. Scroll position is kept per tab (switching back returns to where
you were). Default tab: CITIZENS.

### Business list

`Roster::new` precomputes `business_order: Vec<u16>` alongside the citizen
order: every building where `kind.is_workplace() || kind.has_balance()`,
sorted by a fixed display-group order — Noodle Bar, Vending Plaza, Holo
Arcade, Hydro Farm, Fusion Plant, Robotics Fab, Data Center — then by id
ascending within a group. Buildings are never added or removed mid-run in
this phase, so computing once matches the citizen-order convention.

### Rows

Same `ROW_H` (15 px), same clipping helper, same hover/click handling as the
citizens tab:

- **Name:** `"{kind name} #{id}"` (e.g. `Noodle Bar #1`), clipped to the
  space left of the detail column.
- **Detail column** (right-aligned just left of the balance column):
  - food venues → `format!("{}m", stock.floor())` — meals on hand;
  - employers (`is_workplace`) → `format!("{}w", workers.len())`;
  - arcades → empty string.
- **Balance column** (right-aligned, 6 px from the panel edge, like the
  citizens money column): `money_label(balance)` for `has_balance()` kinds,
  `-` for industry. Red when struggling (rule above), otherwise the roster's
  normal text color `Color::new(0.8, 0.9, 1.0, 0.9)`.

### Interaction

Hover and click mirror the citizens tab via one widened type:

- `Roster::draw` returns `(Option<Selection>, bool)` instead of
  `(Option<usize>, bool)` — `Selection::Citizen(id)` rows on the citizens
  tab, `Selection::Building(id)` rows on the businesses tab.
- `Inspector::draw`'s `preview` parameter widens from `Option<usize>` to
  `Option<Selection>`: previewing a citizen behaves exactly as today;
  previewing a building draws the existing building panel. Follow/camera
  centering stays tied to the *selection* and remains citizen-only.
- Clicking a row sets `inspector.selection` to the row's `Selection` (and
  clears `follow`), exactly like clicking the entity on the map. The
  existing map outline for a selected building comes along for free via
  `main.rs`'s `sel_building`.

### Sim layer

Zero changes. All new logic is presentational.

### New pure helpers (unit-testable, no macroquad)

In `roster.rs`, following the `band`/`money_label` pattern:

- `business_detail(kind, stock, workers) -> String` — the detail-column text.
- `business_struggling(kind, balance, insolvent) -> bool` — the red rule:
  `(is_workplace && insolvent) || (is_food && balance < WHOLESALE_PRICE)`.
- `business_order(city) -> Vec<u16>` — the grouped, id-sorted list (free
  function so the ordering is testable without constructing a `Roster`).

## Testing

- Unit tests: `business_detail` per kind (food/employer/arcade), the
  floor behavior on fractional stock; `business_struggling` for insolvent
  farm, broke venue (boundary at `WHOLESALE_PRICE`), healthy cases, and
  industry-never-struggling-via-balance; `business_order` covers exactly
  the 7 kinds, grouped order, ids ascending, Hab Block/Holo Park excluded.
- Existing roster tests unchanged.
- Rendering verified visually (headless web build), per convention: tab
  switch, row contents, hover preview of a building, click-select, red
  balance on a struggling venue if observable.

## Files touched

- `src/ui/roster.rs` — Tab enum, business order + helpers + tests, header
  tab strip, second row renderer, widened return type.
- `src/ui/inspector.rs` — `preview: Option<Selection>`; building preview
  path.
- `src/main.rs` — adapt to the widened hover/click types.

## Out of scope

Sorting/filtering controls; live re-sorting; business detail beyond the two
columns; apartments/parks rows; any sim-layer change; keyboard tab
shortcuts.
