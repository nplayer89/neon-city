# Living World Roadmap

**Date:** 2026-06-11
**Status:** Approved direction, phases pending
**Source:** Brainstorm on making the simulation feel like a living world (lifecycle/mortality, supply chains, relationships, LLM-driven citizens).

## How to use this document

Each phase below is scoped to fit a single implementation session. Work them **one at a time, in order** (dependencies are explicit). For each phase:

1. Start a fresh session. Read this document and the phase's section.
2. If the phase has open design decisions, run a short brainstorm/spec cycle first (`docs/superpowers/specs/`). If all decisions are settled, go straight to the writing-plans skill (`docs/superpowers/plans/`).
3. Implement the plan, keeping the sim layer fully unit-tested (existing convention).
4. Update the status table below and commit.

Do not pull work from a later phase into an earlier one; the cuts exist to keep context windows small.

## Status

| Phase | Title | Status |
|-------|-------|--------|
| 1 | Event feed & sim event bus | done |
| 2 | Business wallets & closed money loop | pending |
| 3 | Delivery trucks & supply chain | pending |
| 4 | Aging & mortality | pending |
| 5 | Relationships & social need | pending |
| 6 | Households: marriage, children, life stages | pending |
| 7 | Memory stream | pending |
| 8 | LLM spotlight citizens (optional) | pending |

## Design principles (apply to every phase)

- **The sim layer stays deterministic and dependency-free.** Same seed + tick count = same world. The only exception is Phase 8, which is feature-flagged and off by default.
- **Legibility over volume.** A mechanic only counts as "alive" if the player can see it happen (ticker events, inspector panels, visible agents on the map).
- **Closed loops over faucets.** Prefer money/resources that flow between entities to values that appear from or vanish into nothing.
- **Emergence from simple rules.** Tired workers have more accidents; friends cluster at venues. Wire causes to existing state rather than scripting outcomes.

---

## Phase 1 — Event feed & sim event bus

**Goal:** A typed event system in the sim layer and a news-ticker UI, so every later phase ships with visible, readable consequences.

**Depends on:** nothing.

**Scope:**
- `SimEvent` enum + a per-tick event queue the sim emits and the UI drains.
- Bottom-of-screen ticker showing recent events with game-time stamps; scrollback panel optional.
- Emit events for things the sim already does: venue out of stock, citizen hits critical need, wage payout day summary (pick 3–5; don't over-instrument).
- Clicking an event selects the citizen/building involved (reuse inspector selection).

**Out of scope:** event persistence, filtering UI.

**Key decisions (settled):** events are plain data in the sim layer (no rendering concerns); UI owns formatting/colors.

**Exit criteria:** ticker visible during play; at least 3 event types firing from existing mechanics; sim tests cover event emission.

---

## Phase 2 — Business wallets & closed money loop

**Goal:** Businesses have bank balances; money flows citizen → venue → supplier/wages → citizen, conserved end to end.

**Depends on:** Phase 1 (emits hiring/payment/insolvency events).

**Scope:**
- Balance field on buildings. Meal/arcade payments credit the venue instead of evaporating.
- Wages paid **from the employer's balance**, not from nothing.
- Farms charge venues a wholesale price per meal at distribution time (instant transfer for now; trucks come in Phase 3). Retail prices gain a margin.
- Simple insolvency rule: an employer that can't cover wages stops paying; unpaid workers quit (return to job market pool). Full business closure waits for Phase 3.
- Inspector building panel shows balance; money-conservation invariant covered by a test.

**Out of scope:** trucks/logistics, business closure, rent, utility bills.

**Open decisions (resolve during spec):** money supply — seed balances and let it ride, or add a small city-treasury faucet/sink (e.g., a stipend for the unemployed) to keep the economy from deadlocking. Where Holo Park (free venue) and non-commercial workplaces (Fusion Plant, Data Center, Robotics Fab) get revenue — likely a flat "city contract" income to fund their wages until they sell something real.

**Exit criteria:** total money in world is conserved (test); a venue's balance visibly rises and falls in the inspector; insolvency event appears in the ticker.

---

## Phase 3 — Delivery trucks & supply chain

**Goal:** Goods physically move. Farms hold stock, venues order it, trucks drive it, and a late truck means an empty noodle bar at lunch rush.

**Depends on:** Phase 2 (wholesale payments exist).

**Scope:**
- Vehicles become sim agents: position, A* route over the road grid, speed. (Current render-only traffic gets replaced or driven by these.)
- Farms accumulate stock; venues place purchase orders when below a threshold; orders dispatch a truck (pickup → deliver → payment on delivery).
- **Truck driver** occupation: employed by farms (or a depot), drives during shift.
- Business failure now real: a venue insolvent past a grace period closes — building goes dark, workers laid off, citizens reroute. Re-opening/new businesses can wait.
- Ticker events: delivery completed, stockout, business closed.

**Out of scope:** supply chains beyond food (power, materials), traffic congestion, multiple goods types.

**Open decisions (resolve during spec):** truck ownership model (per-farm vs. shared depot); whether closed venues ever reopen in this phase.

**Exit criteria:** trucks visibly drive pickup/delivery routes; venue stock only changes via deliveries and meals sold; a starved-of-stock venue closes and the ticker reports it.

---

## Phase 4 — Aging & mortality

**Goal:** Citizens age and die — of old age, starvation, workplace accidents, and traffic accidents — and the city absorbs the loss.

**Depends on:** Phase 3 (trucks enable traffic accidents; closed economy makes starvation reachable). Phase 1 (obituary events).

**Scope:**
- Age field + life-stage clock. **Key decision to settle first:** aging time scale, decoupled from the daily needs cycle (candidate: 1 game day ≈ 1 year of age; an 80-day lifespan ≈ 5 hours at 1×).
- Death causes:
  - *Old age:* hazard curve rising past a threshold age.
  - *Starvation:* sustained time at zero hunger.
  - *Workplace accidents:* per-building-type base risk (Fusion Plant high, Data Center low), scaled by the worker's energy level.
  - *Traffic accidents:* small per-encounter chance when a citizen and a moving truck share a tile.
- Consequences: job vacancy (employer rehires from unemployed pool), apartment freed, wallet handling (settle in spec: evaporate vs. simple inheritance).
- Population replacement: move-ins arrive at the city edge at a rate that tracks deaths (births replace this in Phase 6).
- Obituary ticker events with name, age, cause.

**Out of scope:** births/children (Phase 6), health/hospitals, funerals.

**Exit criteria:** population stays roughly stable over several game days (test); each death cause observed in tests; obituaries in ticker; roster handles citizens disappearing.

---

## Phase 5 — Relationships & social need

**Goal:** Citizens know each other. Friendships form from shared time, decay without it, and pull people toward each other.

**Depends on:** Phase 1 (relationship events). Independent of Phases 2–4 in principle, but keep order — Phase 6 needs both 4 and 5.

**Scope:**
- Sparse relationship graph: per-pair affinity score, bumped by co-presence (same venue activity, same shift), weighted by personality compatibility (e.g., two Slobs bond; Neat Freak + Slob grate). Slow decay toward neutral.
- Fifth need: **social**, satisfied by performing activities near liked citizens.
- AI integration: action scoring gets a bonus for venues where friends currently are; strong negative affinity (rivals) repels.
- Relationship tab in the citizen inspector (top friends/rivals with bars); optional affinity lines on the map for the selected citizen.
- Ticker events: new friendship, falling-out.

**Out of scope:** marriage/children (Phase 6), conversations (Phase 8).

**Open decisions (resolve during spec):** affinity scale and thresholds; cap on tracked edges per citizen (memory/perf).

**Exit criteria:** observable cliques (same groups recur at venues); relationship tab populated; social need visible in roster/inspector alongside the other four.

---

## Phase 6 — Households: marriage, children, life stages

**Goal:** High-affinity couples marry, share homes, and raise children who grow up and join the workforce — births replace move-ins as population balance.

**Depends on:** Phases 4 and 5.

**Scope:**
- Marriage: sustained high mutual affinity + compatibility → wed (ticker event), move into one apartment (other is freed).
- Children: married couples can have a child (rate-limited); child spawns as a new citizen in the household.
- Life stages: child (no job, simplified needs, stays near home/leisure) → adult (enters job market) → elder (retires, vacating a job). Tie stage transitions to the Phase 4 aging clock.
- Population balance: birth rate tuned so move-ins (Phase 4) can be throttled down or off.
- Inspector shows household/family members.

**Out of scope:** divorce, schools, multi-generation inheritance drama.

**Open decisions (resolve during spec):** household budget (shared wallet vs. separate); apartment capacity rules.

**Exit criteria:** weddings and births occur and appear in ticker; children visibly grow through stages and take jobs; population self-sustains with move-ins disabled (test).

---

## Phase 7 — Memory stream

**Goal:** Citizens remember salient experiences. Useful on its own for legibility and smarter heuristics; prerequisite fuel for Phase 8 prompts.

**Depends on:** Phases 1–6 supply the events worth remembering (works with whatever subset exists, but slot it here).

**Scope:**
- Per-citizen capped ring buffer of typed memories with game-time stamps (e.g., "noodle bar was out of stock," "worked a shift with Mira," "witnessed the accident on 5th").
- Salience scoring decides what's kept when the buffer is full.
- Simple heuristic feedback: recent bad memories of a venue lower its action score for a while.
- "Memories" tab in the citizen inspector — a readable diary.

**Out of scope:** any LLM usage; cross-citizen gossip.

**Exit criteria:** inspector diary reads as a coherent recent history of that citizen; venue-avoidance heuristic covered by a test; memory footprint bounded (test).

---

## Phase 8 — LLM spotlight citizens (optional, feature-flagged)

**Goal:** A small set of "spotlight" citizens get LLM-driven deliberation and inter-character conversations that nudge the deterministic sim.

**Depends on:** Phases 5 and 7 (relationships + memories are the prompt fuel). Richer world state from 2–6 is what makes outputs interesting.

**Scope:**
- **Two-tier cognition:** utility AI remains the autonomic layer for all routine decisions. The LLM layer fires rarely — daily reflection, major life choices (quit job, propose, move), and conversations when two related spotlight citizens co-locate.
- LLM returns **structured nudges only** (affinity delta, goal flag, scheduled plan like "arcade at 19:00"); the deterministic sim executes them. Dialogue text is shown to the player (speech bubbles or inspector transcript).
- **Spotlight selection:** a handful of citizens (player-chosen or rotating), bounded cost.
- **Async, non-blocking:** calls run off the sim thread; results apply on arrival; the sim never stalls.
- **Feature flag, native-only, off by default:** with the flag off, the build keeps zero external dependencies and full determinism (this is the project's first allowed dependency — HTTP client — and it must stay out of the default build). Record/replay of LLM responses for reproducible runs.

**Out of scope (v1 of this phase):** all-citizen LLM cognition, WASM support for the flag, gossip propagation, long-horizon planning.

**Open decisions (resolve during spec):** provider/model and prompt budget; how nudges are validated/clamped so the LLM can't break sim invariants; spotlight UI.

**Exit criteria:** with flag off, build and behavior are byte-identical to Phase 7; with flag on, a spotlight citizen holds a conversation that visibly changes a relationship or plan; a recorded run replays deterministically.

---

## Deliberately deferred (not on the roadmap yet)

Rent/housing market and eviction; utility bills (Fusion Plant selling power); health need + hospital + doctor; weather; festivals/weekends; crime; new business formation; save/load. Revisit after Phase 4 — several become much easier once the money loop and lifecycle exist.
