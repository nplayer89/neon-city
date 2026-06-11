# Money Visibility — Design

**Date:** 2026-06-11
**Status:** Approved

## Context

The economy core already exists and is tested: citizens carry `money` (spawn
with ₢40–80), jobs pay `wage_per_hour` per tick worked, and food/leisure
venues charge per visit. The gap is purely presentational: the wallet balance
is easy to miss (small top-right figure in the inspector panel) and absent
from the roster sidebar.

## Goals

1. Make a citizen's current wallet balance obvious in the inspector panel.
2. Show each citizen's balance in the roster sidebar so "who's broke" is
   scannable the same way "who's hungry" already is.

Non-goals: any change to sim behavior, prices, wages, or money flow.

## Design

### 1. Inspector panel (`src/ui/inspector.rs`)

- Remove the small `₢ 47` readout at the top-right of the citizen panel.
- Add a `WALLET` line item in the existing label/value list, directly above
  `JOB`, using the same layout as JOB/NOW rows. Value text `₢ 47` (whole
  credits) in the existing yellow accent `Color::new(0.95, 0.85, 0.3, 1.0)`,
  drawn at 18 px (labels are 15 px) so it reads as the headline figure.
- Subsequent rows (JOB, NOW, FOLLOW button) shift down one row height
  (26 px); panel height already has slack.

### 2. Roster sidebar (`src/ui/roster.rs`)

- Widen `SIDEBAR_W` from 200 → 240 px.
- Each row: name (left, clipped) · money (right-aligned, compact `₢123`,
  rounded to whole credits) · 4 need icons (unchanged, rightmost).
- Money column sits between the name and the icons with a fixed reserved
  width of 44 px; name clipping width shrinks accordingly.
- Money is color-coded with the existing `Band` palette via a new pure
  function:

  ```rust
  /// Discrete status band for a wallet balance.
  pub fn money_band(balance: f32) -> Band {
      if balance >= 40.0 { Band::High }      // comfortable
      else if balance >= 15.0 { Band::Medium } // one noodle meal + slack
      else { Band::Low }                       // can't afford a ₢12 meal soon
  }
  ```

  Thresholds rationale: ₢15 ≈ price of the most expensive meal (₢12) plus
  slack; ₢40 ≈ spawn-range floor / several meals of cushion.

## Testing

- Unit test `money_band` boundaries (0, 14.9, 15, 39.9, 40, large), same
  style as the existing `band_boundaries` test.
- Rendering changes follow the existing pattern of untested draw code;
  verify visually by running the app.

## Files touched

- `src/ui/inspector.rs` — wallet line item, remove top-right readout.
- `src/ui/roster.rs` — sidebar width, money column, `money_band` + test.
