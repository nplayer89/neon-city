# Business Wallets & Closed Money Loop — Design (Roadmap Phase 2)

**Date:** 2026-06-11
**Status:** Approved
**Source:** Roadmap Phase 2 (`docs/superpowers/roadmap/2026-06-11-living-world-roadmap.md`) + brainstorm resolving its open decisions.

## Context

Today money is open at both ends: wages are minted from nothing per tick
worked, meal/arcade payments evaporate, and farms push free stock to food
venues hourly. Phase 2 closes the loop where real revenue exists — the food
economy — so money flows citizen → venue → farm → farmhand → citizen and is
conserved end to end.

## Decisions resolved during brainstorm

| Decision | Outcome |
|---|---|
| Industry revenue (Fusion Plant, Robotics Fab, Data Center) | **Deferred to a later phase.** Their wages keep being minted, but every minted dollar is tracked so conservation stays testable. |
| Unemployment stipend | **None.** Broke unemployed citizens visibly run dry (CantAffordMeal); Phase 4 mortality gives this teeth later. |
| Farm books vs. city food budget | **Rebalance both sides (approach A).** Farm payroll shrinks (fewer, cheaper farmhands) and food prices rise so farms and venues run thin-but-positive margins. Background: the city eats ~72 meals/day (48 citizens × ~1.5), so farm income is capped at 72 × wholesale; today's ~$990/day farm payroll could never be covered. |
| Dynamic supply/demand pricing | **Deferred to Phase 3**, where purchase orders give an honest demand signal. Phase 2 keeps prices as constants, looked up through `economy.rs` so a later swap to per-building dynamic prices is local. Noted in the roadmap. |
| Rehiring after quits | **None this phase** — quit workers join the unemployed pool; Phase 4 adds rehiring (per roadmap). |

## Design

### Money model

- `Building` gains `balance: f32`. Seeds: food venues **$100** (float for
  day-one wholesale), hydro farms **$300** (≈ half a day's payroll), all
  other kinds $0.
- `World` gains `minted: f32`: a running total of money created from
  nothing. In this phase that is exactly industry payroll (Fusion Plant,
  Robotics Fab, Data Center).
- **Conservation invariant:** `Σ citizen wallets + Σ building balances ==
  initial total + minted`, within a small epsilon. Money stays `f32`;
  hourly payroll settlement (below) keeps float drift far below the
  test epsilon.
- Balances and wallets never go negative; every transfer is
  affordability-checked first.
- Known, accepted: venue and arcade profits accumulate with no outlet this
  phase (~$700/day parked across all venues). Phase 3 purchase orders and
  business closure put that money back into circulation.

### Flow 1 — Retail (redirected)

Meal and arcade payments credit the venue balance instead of evaporating:
`c.money -= price; building.balance += price`. Holo Park stays free.

### Flow 2 — Wholesale (new)

At the existing hourly distribution (06:00–22:00), each food venue buys its
even share of pooled farm output at the wholesale price, limited by stock
cap **and by its balance**: `take = min(share, cap_room, balance / WHOLESALE)`.
Payment (`take × WHOLESALE`) is split evenly across farms (equal output).
A broke venue stops restocking and the existing `VenueSoldOut` chain takes
over — no new event needed. Undelivered output is lost, as today.

### Flow 3 — Payroll (changed: hourly settlement)

Wages settle **on the hour** instead of accruing per tick: a worker in the
`Performing { Work }` state at an hour boundary receives `wage_per_hour` in
one transfer. Farm workers are paid from the farm balance; industry workers
are paid the same way except the money is minted (`minted += wage`).
Consequences, accepted: paydays are chunky and legible in the wallet; a
partial hour (arrived late, quit early) pays nothing for that hour.
`wages_today` / the `DailyWages` event keep their meaning (sum of all wages
actually paid).

### Flow 4 — Insolvency (new)

- A settlement that the employer balance cannot cover pays nothing.
- `Building` gains an `insolvent: bool` flag. On a failed settlement with
  the flag clear → set it and emit `EmployerInsolvent { building }`
  (edge-triggered). On a successful settlement with the flag set → clear it
  silently. Partial-payroll hours may oscillate the flag; each false→true
  transition emits, which is acceptable and rare.
- `Job` gains `unpaid_hours: u32`. Each failed settlement increments it; a
  successful one resets it to 0. At **8 unpaid hours** (one full shift) the
  worker quits: `job = None`, removed from `Building.workers`, emit
  `WorkerQuit { citizen, building }`. No rehiring this phase.

### Rebalancing constants

| Constant | Value | Today | Rationale |
|---|---|---|---|
| Noodle Bar meal | **$15** | $12 | margin $8 over wholesale |
| Vending Plaza meal | **$10** | $5 | stays the cheap option; margin $3 |
| Arcade | $8 | $8 | unchanged |
| Wholesale per meal | **$7** | — (free) | 72 meals/day ⇒ ~$500/day farm income |
| Farmhand wage | **$9–11/h** | $11–18/h | per-kind wage range, new |
| Other wages | $11–18/h | $11–18/h | unchanged |
| Max workers per farm | **3** | ~4–5 (round-robin) | farm payroll ≈ $480/day < ~$500 income: thin structural profit |
| Unpaid hours to quit | 8 | — | one full shift |
| Venue / farm balance seed | $100 / $300 | — | day-one float |

Spawn assignment honors the farm cap: when round-robin lands on a full
farm, it advances to the next non-full workplace (surplus shifts to
industry, whose minted wages don't care). Employed citizens still afford
food easily (~$80–145/day income vs ~$25/day food at new prices).

### Events

New `EventKind` variants (sim layer, plain data as established):

- `EmployerInsolvent { building: u16 }`
- `WorkerQuit { citizen: usize, building: u16 }`

`ui/ticker.rs` gains wording and colors for both; both click-select the
building / citizen respectively, reusing the existing selection wiring.

### UI

Inspector building panel gains a **BALANCE** line item (existing label/value
layout, `$` prefix per the ASCII-font convention) for participating
buildings: food venues, arcades, hydro farms. Industry and Holo Park show no
balance line this phase. Citizen-side money display already exists
(money-visibility work).

### Determinism & tests

`World::fingerprint` extends to `building.balance` and `minted`. Sim-layer
tests (all deterministic, no rendering):

1. **Conservation:** over ≥3 game days, `|total − initial − minted| < 0.5`.
2. Retail: buying a meal credits the venue by exactly the price.
3. Wholesale: distribution transfers venue → farm; a venue never spends
   below $0; a broke venue receives no stock.
4. Payroll: a farm worker's wallet rises by `wage_per_hour` at the hour
   boundary and the farm balance falls by the same; an industry worker is
   paid while `minted` rises by the same.
5. Insolvency: drain a farm balance → `EmployerInsolvent` fires once
   (edge); after 8 unpaid hours the worker quits, `WorkerQuit` fires,
   worker leaves `workers` and becomes unemployed.
6. Soak: 3 game days at the shipped seed — food still sells, at least one
   farm solvent with workers, conservation holds.
7. Existing tests: wage tests assert over ≥3h windows and survive hourly
   settlement; determinism test unchanged in form.

## Out of scope (this phase)

Industry balances/contracts; stipend; dynamic pricing (Phase 3); trucks &
real logistics (Phase 3); business closure (Phase 3); rehiring (Phase 4);
rent/utility bills (deferred list).

## Files touched

- `src/sim/city.rs` — `balance`, `insolvent` fields + seeds.
- `src/sim/economy.rs` — new prices, `WHOLESALE`, per-kind wage ranges,
  wholesale-aware distribution.
- `src/sim/world.rs` — `minted`, hourly settlement, insolvency/quit logic,
  farm-capped spawn assignment, fingerprint additions.
- `src/sim/citizen.rs` — `Job.unpaid_hours`.
- `src/sim/event.rs` — two new event kinds.
- `src/ui/ticker.rs` — formatting for new events.
- `src/ui/inspector.rs` — BALANCE line item.
- `docs/superpowers/roadmap/2026-06-11-living-world-roadmap.md` — Phase 3
  note: dynamic pricing deferred from Phase 2.
