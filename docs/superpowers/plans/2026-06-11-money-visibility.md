# Money Visibility Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make citizens' wallet balances visible in the roster sidebar and prominent in the inspector panel.

**Architecture:** Pure presentation change — no sim code is touched. A new pure `money_band()` function in `src/ui/roster.rs` maps a balance to the existing `Band` enum for color-coding; the roster gains a money column and the inspector gains a `WALLET` line item replacing the small top-right readout.

**Tech Stack:** Rust, macroquad 0.4 (immediate-mode drawing). Tests via `cargo test`.

**Spec:** `docs/superpowers/specs/2026-06-11-money-visibility-design.md`

---

## File Structure

- Modify: `src/ui/roster.rs` — `money_band()` + boundary test, sidebar width 200→240, per-row money readout.
- Modify: `src/ui/inspector.rs` — remove top-right `₢` readout, add `WALLET` line item above `JOB`.

No new files. Draw code stays untested by convention in this codebase (see `roster.rs` header comment on `band`: "Pure (no macroquad) so it stays unit-testable" — only pure helpers get tests).

---

### Task 1: `money_band()` pure function (TDD)

**Files:**
- Modify: `src/ui/roster.rs` (function near `band()` at ~line 26; test in the existing `mod tests` at ~line 128)

- [ ] **Step 1: Write the failing test**

Add to the existing `mod tests` block at the bottom of `src/ui/roster.rs`, after `band_boundaries`:

```rust
    #[test]
    fn money_band_boundaries() {
        assert_eq!(money_band(0.0), Band::Low);
        assert_eq!(money_band(14.9), Band::Low);
        assert_eq!(money_band(15.0), Band::Medium);
        assert_eq!(money_band(39.9), Band::Medium);
        assert_eq!(money_band(40.0), Band::High);
        assert_eq!(money_band(500.0), Band::High);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test money_band_boundaries`
Expected: compile error — `cannot find function money_band in this scope`

- [ ] **Step 3: Write minimal implementation**

Add to `src/ui/roster.rs` directly below the existing `band()` function (after its closing brace, ~line 34):

```rust
/// Discrete status band for a wallet balance. ₢15 ≈ the priciest meal (₢12)
/// plus slack; ₢40 ≈ several meals of cushion (spawn-range floor).
pub fn money_band(balance: f32) -> Band {
    if balance >= 40.0 {
        Band::High
    } else if balance >= 15.0 {
        Band::Medium
    } else {
        Band::Low
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test money_band_boundaries`
Expected: `test ui::roster::tests::money_band_boundaries ... ok`

- [ ] **Step 5: Commit**

```bash
git add src/ui/roster.rs
git commit -m "feat: add money band function for roster sidebar"
```

---

### Task 2: Roster sidebar money column

**Files:**
- Modify: `src/ui/roster.rs:6` (`SIDEBAR_W`), `:14-15` (const block), draw loop (~lines 96-121)

No unit tests — this is macroquad draw code (codebase convention). Verified by `cargo build` plus the visual check in Task 4.

- [ ] **Step 1: Widen the sidebar and reserve the money column**

In `src/ui/roster.rs`, change:

```rust
const SIDEBAR_W: f32 = 200.0;
```

to:

```rust
const SIDEBAR_W: f32 = 240.0;
```

and add below the `ICON_SIZE` const (~line 15):

```rust
/// Reserved width for the right-aligned wallet readout, left of the icons.
const MONEY_COL_W: f32 = 44.0;
```

- [ ] **Step 2: Shrink the name clip width to make room**

In `Roster::draw`, change:

```rust
        let icons_x = x + w - 6.0 - NEED_KINDS.len() as f32 * ICON_STRIDE;
        let name_max_w = icons_x - (x + 10.0) - 4.0;
```

to:

```rust
        let icons_x = x + w - 6.0 - NEED_KINDS.len() as f32 * ICON_STRIDE;
        let name_max_w = icons_x - MONEY_COL_W - (x + 10.0) - 4.0;
```

(Net name space is ~132 px vs ~136 px before — the 40 px widening absorbs the column.)

- [ ] **Step 3: Draw the money readout per row**

In the row loop, after the `draw_clipped_text(&c.name, ...)` call and before the need-icon `for` loop, insert:

```rust
            let money_text = format!("₢{:.0}", c.money);
            let mw = measure_text(&money_text, None, 13, 1.0).width;
            draw_text(&money_text, icons_x - 4.0 - mw, ry + 11.5, 13.0, band_color(money_band(c.money)));
```

(`measure_text` is already in scope via `macroquad::prelude::*`.)

- [ ] **Step 4: Build and run the full test suite**

Run: `cargo test`
Expected: all tests pass (34 after Task 1), no warnings about unused `MONEY_COL_W`.

- [ ] **Step 5: Commit**

```bash
git add src/ui/roster.rs
git commit -m "feat: show wallet balance in roster sidebar rows"
```

---

### Task 3: Inspector WALLET line item

**Files:**
- Modify: `src/ui/inspector.rs:100` (remove top-right readout), `:115-127` (insert WALLET row above JOB)

No unit tests — macroquad draw code.

- [ ] **Step 1: Remove the top-right money readout**

In `draw_citizen_panel` in `src/ui/inspector.rs`, delete this line (the `money` binding above it stays — it's reused in Step 2):

```rust
        draw_text(&format!("₢ {:.0}", money), x + w - 80.0, y + 30.0, 22.0, Color::new(0.95, 0.85, 0.3, 1.0));
```

- [ ] **Step 2: Insert the WALLET line item above JOB**

In the same function, find the job section:

```rust
        // job + state
        by += 8.0;
        let job = match &world.citizens[id].job {
```

and insert the wallet row between `by += 8.0;` and `let job = ...`:

```rust
        draw_text("WALLET", x + 14.0, by + 12.0, 15.0, Color::new(0.6, 0.75, 0.9, 0.9));
        draw_text(&format!("₢ {:.0}", money), x + 90.0, by + 14.0, 18.0, Color::new(0.95, 0.85, 0.3, 1.0));
        by += 26.0;
```

(Value is 18 px vs the 15 px labels so it reads as the headline figure; `+ 14.0` baseline centers the larger text on the label. Rows below shift down 26 px; the 330 px panel has ~24 px of slack left after the FOLLOW button.)

- [ ] **Step 3: Build and run the full test suite**

Run: `cargo test`
Expected: all tests pass; `cargo build` emits no warnings (in particular no unused-variable warning for `money`).

- [ ] **Step 4: Commit**

```bash
git add src/ui/inspector.rs
git commit -m "feat: prominent WALLET line in inspector citizen panel"
```

---

### Task 4: Final verification

- [ ] **Step 1: Full test suite**

Run: `cargo test`
Expected: 34 tests pass, 0 failed.

- [ ] **Step 2: Release build**

Run: `cargo build --release`
Expected: clean build, no warnings.

- [ ] **Step 3: Visual check**

Run: `cargo run --release` and verify:
- Roster sidebar is 240 px wide; each row shows a color-coded `₢NN` between the name and the need icons (green ≥₢40, yellow ≥₢15, red below).
- Clicking a citizen shows `WALLET ₢ NN` in yellow above the JOB row; no money figure in the panel's top-right corner.
- Hovering a roster row previews the same panel with the WALLET line.
