# Delivery Trucks & Supply Chain — Design (Roadmap Phase 3)

**Date:** 2026-06-16
**Status:** Approved
**Source:** Roadmap Phase 3 (`docs/superpowers/roadmap/2026-06-11-living-world-roadmap.md`) + brainstorm resolving its open decisions.

## Context

Phase 2 closed the food money loop but goods still teleport: `distribute_food`
is an instant hourly *push* — each venue buys its even share of pooled farm
output at a fixed wholesale price, payment transfers immediately, and any
undelivered output is lost. Its own comment marks the seam: "farms holding
stock arrives with trucks in Phase 3."

Phase 3 makes goods physically move. Farms accumulate inventory, venues order
when they run low, per-farm trucks drive the meals over the road grid, and
payment settles on delivery at a price that floats with supply and demand. A
venue chronically starved by the chain — a late truck at lunch rush, repeated —
can now go broke and close. This is the first phase where logistics failure is
visible on the map.

## Decisions resolved during brainstorm

| Decision | Outcome |
|---|---|
| Truck ownership | **Per-farm.** Each Hydro Farm owns one truck and employs its driver as a farm job. No depot building, no new money node; reuses the farm-employer + `wages_from_balance` model. The venue pays the owning farm on delivery. |
| Dispatch | **Decentralized, per-farm greedy.** Each farm with a parked truck, an on-shift driver, and stock independently claims the neediest open venue (lowest stock; ties by id), resolved in farm-id order. Two farms partition the city emergently — no central dispatcher. |
| Business closure | **Close, no reopen.** A venue insolvent past a grace period closes permanently this phase: dark, workers laid off, occupants ejected, dropped from orders/AI. Reopening and new-business formation stay deferred (roadmap). |
| Wholesale pricing | **Dynamic, city-wide supply/demand** (model A below). Spot price set at delivery, bounded $4.20–$11.20 around the $7 base. |
| Driver realism | **Driver rides the truck.** While on a run the driver leaves the farm, their position follows the truck, wages keep settling, and they render as the truck (not a pedestrian). Visible occupation per the legibility principle. |
| Trucks per farm | **One truck + one driver per farm** to start. The driver is one of the farm's existing worker slots (payroll unchanged from Phase 2). |
| Production vs. headcount | **Unchanged** — farm output stays a flat hourly rate, independent of worker count (as in Phase 2). Farmhands remain payroll participants; the driver additionally operates the truck. |
| Ambient traffic | **Kept** as cheap background flavor. Trucks are real sim agents drawn distinctly (larger hauler + cargo pip), so they read as "the real ones." |
| Truck selection | **Out of scope.** The `Selection` enum is untouched; the driver citizen is selectable and shows driving status. |

## Design

### Entities

New module **`src/sim/logistics.rs`** owns the truck/order types and the pure
helpers (pricing, neediest-venue choice, load math); `world.rs` orchestrates
per tick, mirroring `tick_citizen`.

```
Truck {
    id: usize,
    home_farm: u16,
    driver: Option<usize>,        // citizen id; None when unassigned
    pos: (f32, f32),
    path: VecDeque<(i32, i32)>,
    speed: f32,                   // tiles/tick, ~2–3× a citizen
    cargo: f32,                   // meals aboard
    state: TruckState,
}

TruckState =
    | Parked                      // at farm door, available
    | Outbound { venue: u16 }     // loaded, driving to the venue door
    | Returning                   // driving back to the farm door
```

`World` gains `trucks: Vec<Truck>` (one per farm, created at world init and
assigned a driver from the farm's workers). Buildings already carry `stock`,
`balance`, `workers`, `occupants`; Phase 3 adds two fields to `Building`:

- `closed: bool` — venue has shut down (frozen, dark).
- `hours_broke: u32` — consecutive hours unable to afford one wholesale meal.

A new predicate `Building::open(&self) -> bool` returns `!self.closed`; it gates
ordering, dispatch targeting, and AI targeting.

### Driver occupation

Each farm designates one of its existing worker slots (≤ `FARM_MAX_WORKERS`) as
the **truck driver**; the rest stay farmhands. Payroll is unchanged from Phase 2
— the driver is paid the same farmhand wage from the farm balance.

`CitizenState` gains `Driving { truck: usize }`. A farm dispatches only when its
driver is currently `Performing { Work }` at the farm (on-shift, present). On
dispatch the driver transitions to `Driving`, leaves the farm's `occupants`, and
from then on the truck-drive step sets the driver's `pos` to the truck's. While
`Driving`, the driver's wages settle hourly exactly as `Work` does (still on the
clock) and needs decay normally; the driver picks no actions. When the truck
parks — or at shift end after finishing the current round trip — the driver is
placed at the farm door and returns to `Idle`. The render layer skips drawing a
`Driving` citizen as a pedestrian (the truck represents them); the inspector
shows "Driving for Hydro Farm #NN."

If the driver quits (unpaid, per Phase 2) or none is on shift, the truck stays
`Parked` and the farm reassigns a driver from its remaining workers on a later
tick. No driver ⇒ no deliveries from that farm until a worker is on shift —
deliveries naturally pause overnight and during gaps, an accepted emergent gap.

### The delivery loop

A new logistics step runs in `world.tick()`, **after the citizen loop**, and
**replaces the `distribute_food` call**. Order of operations each tick:

1. **Produce** (on the hour, 06:00–22:00): every open farm does
   `farm.stock = (farm.stock + FARM_OUTPUT_PER_HOUR).min(FARM_STOCK_CAP)`.
   Farms now hold inventory instead of pushing it.
2. **Dispatch** (each tick): for each farm in id order with a `Parked` truck, an
   on-shift driver present, and `stock > 0`, find the neediest open venue —
   lowest `stock` among open food venues with `stock < ORDER_THRESHOLD` that has
   no truck already `Outbound` to it (ties broken by id). If one exists, load
   `cargo = min(TRUCK_CAPACITY, farm.stock, STOCK_CAP − venue.stock)`, do
   `farm.stock −= cargo`, transition the driver to `Driving`, route the truck
   from the farm door to the venue door (A* `find_path`), set `Outbound`.
3. **Drive** (each tick): advance every non-`Parked` truck along its path by
   `speed` using the same vector math as citizen travel; pop reached waypoints;
   copy the truck's `pos` to its driver.
4. **Arrive** (path empty):
   - **Outbound → at venue:** compute the spot `price` (below). The venue buys
     `bought = min(cargo, balance / price, STOCK_CAP − stock)`; then
     `venue.stock += bought`, `venue.balance −= bought·price`,
     `farm.balance += bought·price`, `cargo −= bought`. Emit
     `DeliveryCompleted { farm, venue, meals: bought.round() }`. Switch to
     `Returning`, routing back to the farm door. (A broke venue simply buys
     less; the unbought remainder rides back.)
   - **Returning → at farm:** `farm.stock += cargo` (leftover returns — no
     waste, no leak), `cargo = 0`, driver released to the farm door, truck
     `Parked`.

Money is conserved: every delivery is a venue→farm transfer of exactly
`bought·price`, and unsold meals return to the farm rather than vanishing.
Meals are conserved too: created only by farm production, destroyed only by
citizens eating (unchanged Phase 2 path).

### Dynamic wholesale pricing (model A)

Spot price computed at each delivery from current city-wide conditions:

```
supply  S = Σ open farm.stock
demand  D = Σ max(0, STOCK_CAP − venue.stock)   over open food venues
price = WHOLESALE_BASE · clamp( D / (S + 1.0), PRICE_LO_MULT, PRICE_HI_MULT )
```

Balanced city (`D ≈ S`) ⇒ price ≈ base. Venues starving while farms are empty ⇒
price pegs to the ceiling; farms glutted while venues are full ⇒ price floors.
The `+1.0` avoids a divide-by-zero and a blow-up at `S = 0`. One bounded number,
easy to display ("WHOLESALE $9") and to test at the extremes.

This perturbs Phase 2's "farm books almost close" tuning: a sustained low price
could push a farm insolvent → farmhands quit → production stalls. The
`PRICE_LO_MULT` floor mitigates this; final `LO/HI` values are tuned against the
conservation soak so at least one farm stays solvent over several game days.

### Business closure (food venues)

Each hour, for every open food venue: if `balance < wholesale spot price`
(can't afford even one meal), `hours_broke += 1`, else `hours_broke = 0`. At
`hours_broke ≥ CLOSURE_GRACE_HOURS` the venue **closes**:

- `closed = true`; emit `BusinessClosed { building }`.
- Lay off workers: for each id in `workers`, set that citizen's `job = None`
  and clear their `Performing`/path back to `Idle` if they were on site; clear
  `workers`.
- Eject `occupants` to the door, `Idle`.
- It no longer appears as an order target, dispatch target, or AI target
  (`open()` is false), and trucks already `Outbound` to it abort to `Returning`
  on arrival (deliver nothing to a closed venue).
- Its `balance` stays frozen on the dark building, so money remains conserved.

Closure applies to food venues only — arcades only earn (can't go broke) and
industry has no books. It should be rare (retail > wholesale floor); it bites a
venue the chain chronically fails to stock, which is the visible failure the
phase is for.

### Events

New `EventKind` variants (sim layer, plain data):

- `DeliveryCompleted { farm: u16, venue: u16, meals: u16 }`
- `BusinessClosed { building: u16 }`

`VenueSoldOut` (stockout) already exists and stays. `ui/ticker.rs` gains wording
and colors for both new kinds; `DeliveryCompleted` click-selects the venue,
`BusinessClosed` the building, reusing the existing selection wiring.

### Rendering

- **Trucks** (`src/render/agents.rs`): drawn as a larger hauler rectangle than
  ambient cars, with a small cargo pip whose presence reflects `cargo > 0`, and
  night headlights like the ambient vehicles. Trucks are sim-driven, so they
  follow roads exactly. The existing render-only `Traffic` stays as background
  flavor.
- **Driver:** a `Driving` citizen is not drawn as a pedestrian (the truck is
  them). The inspector citizen panel shows "Driving for Hydro Farm #NN" when in
  that state.
- **Closed venues** (`src/render/buildings.rs`): dark/desaturated trim so a shut
  venue reads at a glance.

### UI / Inspector

- Citizen panel: `Driving` status line as above.
- Building panel: a food venue shows a **CLOSED** marker when `closed`; farms
  may show `STOCK` (they now hold inventory). Optional, low-cost additions that
  reuse the existing label/value layout.

### Constants (`economy.rs`)

| Constant | Value | Note |
|---|---|---|
| `WHOLESALE_BASE` | **$7** | renamed from `WHOLESALE_PRICE`; now the base, not the price |
| `PRICE_LO_MULT` / `PRICE_HI_MULT` | **0.6 / 1.6** | price band $4.20–$11.20 |
| `FARM_OUTPUT_PER_HOUR` | 6.0 | unchanged; now accumulates into farm `stock` |
| `FARM_STOCK_CAP` | **120** | farms buffer ~a couple days; > venue `STOCK_CAP` |
| `STOCK_CAP` | 60 | unchanged (venue cap) |
| `ORDER_THRESHOLD` | **20** | venue is "open order" below this |
| `TRUCK_CAPACITY` | **30** | meals per load |
| `TRUCK_SPEED` | **0.12** | tiles/tick, ~2.5× a citizen |
| `CLOSURE_GRACE_HOURS` | **24** | one game day broke ⇒ close |

These are tuning starting points; `LO/HI`, capacity, and grace are finalized
against the soak test.

**Coverage risk to tune:** deliveries only run while a farm's driver is
on-shift, but venues consume meals all day. Two trucks must keep seven venues
above the closure line across the daily cycle — buffered overnight by
`FARM_STOCK_CAP` inventory and venue stock. If the soak shows venues starving in
the off-shift window, the levers are `TRUCK_CAPACITY`, `TRUCK_SPEED`,
`ORDER_THRESHOLD`, and driver shift hours; a second truck/driver per farm is the
fallback. Test 8 guards this by asserting venues keep selling, not just that one
farm stays solvent.

### Determinism & tests

`World::fingerprint` extends to per-truck `pos`, `state` discriminant, and
`cargo`, plus venue `closed` and `hours_broke`. (Farm `stock` and balances are
already hashed across all buildings; driver `pos` is hashed via the citizen
loop.) Trucks tick in id order; dispatch resolves in farm-id then venue-id
order — fully deterministic.

Pure unit tests in `logistics.rs`:

1. **Pricing:** `wholesale_price` hits `PRICE_HI_MULT·BASE` when `D≫S`,
   `PRICE_LO_MULT·BASE` when `S≫D`, ≈ base when `D≈S`, and is monotonic in `D`.
2. **Neediest venue:** lowest-stock open venue chosen; closed and
   already-served venues skipped; ties by id.
3. **Load math:** `cargo` is the min of capacity, farm stock, and venue room.
4. **Delivery accounting:** a broke venue buys only what it can afford;
   `bought·price` leaves the venue and reaches the farm exactly; leftover stays
   aboard.

World-level tests (deterministic, no rendering):

5. **Dispatch & deliver:** a venue below `ORDER_THRESHOLD` gets a truck; on
   arrival its `stock` rises, money transfers farm↔venue with no leak, and the
   truck returns and parks.
6. **Leftover returns:** delivering to a near-full / broke venue returns the
   remainder to farm `stock` (meal conservation).
7. **Closure:** a venue held broke for `CLOSURE_GRACE_HOURS` closes, fires
   `BusinessClosed` once, lays off its workers (`job = None`, `workers` empty),
   and stops receiving deliveries; its balance is unchanged afterward.
8. **Conservation soak:** ≥3 game days at the shipped seed —
   `|total − initial − minted| < 0.5` still holds under lagged delivery and
   dynamic price; at least one farm stays solvent with workers; and venues keep
   selling (food sales accrue each day and not all venues close — the coverage
   guard above).
9. **Determinism:** `fingerprint` identical across two runs (existing test form,
   now covering trucks).

Existing `economy.rs` distribution tests are removed/rewritten (the instant
push is gone); existing wage and determinism tests are updated only where the
delivery-timing change affects them.

## Out of scope (this phase)

Dynamic *retail* pricing; multiple goods (power, materials); supply chains
beyond food; depot/shared fleets; truck selection in the inspector; venue
reopening and new-business formation (later phase); rehiring laid-off workers
(Phase 4); traffic congestion; tying farm output to headcount.

## Files touched

- `src/sim/logistics.rs` — **new**: `Truck`, `TruckState`, `wholesale_price`,
  neediest-venue/load helpers, dispatch/drive/arrive, unit tests.
- `src/sim/economy.rs` — remove `distribute_food`; rename `WHOLESALE_PRICE`
  → `WHOLESALE_BASE`; add price-band + logistics constants; `FARM_OUTPUT_PER_HOUR`
  now feeds farm stock.
- `src/sim/city.rs` — `Building.closed`, `Building.hours_broke`, `Building::open`.
- `src/sim/citizen.rs` — `CitizenState::Driving { truck }`.
- `src/sim/world.rs` — `World.trucks`, truck/driver init, logistics tick step,
  hourly production, closure logic, `Driving` handling in `tick_citizen`,
  fingerprint additions; drop the `distribute_food` call.
- `src/sim/event.rs` — `DeliveryCompleted`, `BusinessClosed`.
- `src/ui/ticker.rs` — formatting + selection wiring for the two new events.
- `src/render/agents.rs` — draw trucks; skip pedestrian draw for `Driving`.
- `src/render/buildings.rs` — dark trim for closed venues.
- `src/ui/inspector.rs` — driver "Driving for …" status; venue CLOSED marker.
- `docs/superpowers/roadmap/2026-06-11-living-world-roadmap.md` — mark Phase 3
  status.
