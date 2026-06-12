use crate::sim::ai;
use crate::sim::citizen::{Activity, Citizen, CitizenState, Job, NeedKind, NEED_KINDS};
use crate::sim::city::{BuildingKind, City};
use crate::sim::economy;
use crate::sim::event::{push_event, EventKind, SimEvent};
use crate::sim::path;
use crate::sim::rng::Rng;
use crate::sim::time::{self, TICKS_PER_DAY, TICKS_PER_HOUR};
use std::collections::VecDeque;

pub struct World {
    pub rng: Rng,
    pub city: City,
    pub citizens: Vec<Citizen>,
    pub tick: u64,
    pub seed: u64,
    /// Events since the last drain (capped; see event::MAX_PENDING).
    /// Private so the cap in push_event can't be bypassed; consume via drain_events.
    events: VecDeque<SimEvent>,
    /// Wages paid since the last midnight rollover.
    wages_today: f32,
    /// Money created from nothing since world start (industry payroll —
    /// their revenue is deferred; see the Phase 2 spec). The conservation
    /// invariant is: Σ wallets + Σ balances == initial total + minted.
    pub minted: f32,
}

const SHIFTS: [(u32, u32); 3] = [(8, 16), (16, 24), (0, 8)];
/// Weighted shift pick: most people work days.
const SHIFT_WEIGHTS: [f32; 3] = [0.6, 0.25, 0.15];

impl World {
    pub fn new(seed: u64, n_citizens: usize) -> World {
        let mut rng = Rng::new(seed);
        let city = City::generate(&mut rng);
        let mut world = World { rng, city, citizens: vec![], tick: 0, seed, events: VecDeque::new(), wages_today: 0.0, minted: 0.0 };

        let homes: Vec<u16> = world
            .city
            .buildings_of(|k| k == BuildingKind::Apartment)
            .map(|b| b.id)
            .collect();
        let workplaces: Vec<u16> = world
            .city
            .buildings_of(|k: BuildingKind| k.is_workplace())
            .map(|b| b.id)
            .collect();

        let mut used_names = std::collections::HashSet::new();
        for i in 0..n_citizens {
            let home = homes[i % homes.len()];
            let door = world.city.buildings[home as usize].door;
            let pos = (door.0 as f32 + 0.5, door.1 as f32 + 0.5);
            let mut c = Citizen::spawn(&mut world.rng, i, home, pos, &mut used_names);
            // ~80% employed
            if world.rng.chance(0.8) {
                // Round-robin, but skip farms already at capacity — surplus
                // shifts to industry, whose minted wages absorb anyone.
                let mut idx = i % workplaces.len();
                for _ in 0..workplaces.len() {
                    let b = &world.city.buildings[workplaces[idx] as usize];
                    if b.kind != BuildingKind::HydroFarm
                        || b.workers.len() < economy::FARM_MAX_WORKERS
                    {
                        break;
                    }
                    idx = (idx + 1) % workplaces.len();
                }
                let wp = workplaces[idx];
                let roll = world.rng.gen_f32();
                let shift = if roll < SHIFT_WEIGHTS[0] {
                    SHIFTS[0]
                } else if roll < SHIFT_WEIGHTS[0] + SHIFT_WEIGHTS[1] {
                    SHIFTS[1]
                } else {
                    SHIFTS[2]
                };
                let (lo, hi) = economy::wage_range(world.city.buildings[wp as usize].kind);
                c.job = Some(Job {
                    workplace: wp,
                    shift_start: shift.0,
                    shift_end: shift.1,
                    wage_per_hour: world.rng.gen_f32_range(lo, hi),
                    unpaid_hours: 0,
                });
                world.city.buildings[wp as usize].workers.push(i);
            }
            c.state = CitizenState::Idle { until: world.rng.gen_range(0, 120) as u64 };
            world.citizens.push(c);
        }
        world
    }

    pub fn hour(&self) -> u32 {
        time::hour(self.tick)
    }
    pub fn hour_f(&self) -> f32 {
        time::hour_f(self.tick)
    }
    pub fn day(&self) -> u64 {
        time::day(self.tick)
    }

    pub fn tick(&mut self) {
        self.tick += 1;
        let (tick, hour, night) = (self.tick, self.hour(), time::is_night(self.tick));
        if self.tick % TICKS_PER_HOUR == 0 {
            economy::distribute_food(&mut self.city, hour);
        }
        let city = &mut self.city;
        let rng = &mut self.rng;
        let events = &mut self.events;
        for c in self.citizens.iter_mut() {
            let before = c.needs;
            c.decay_needs();
            for k in NEED_KINDS {
                if before.get(k) >= ai::CRITICAL && c.needs.get(k) < ai::CRITICAL {
                    push_event(events, SimEvent { tick, kind: EventKind::CriticalNeed { citizen: c.id, need: k } });
                }
            }
            let (w, m) = tick_citizen(c, city, rng, tick, hour, night, events);
            self.wages_today += w;
            self.minted += m;
        }
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

    /// FNV-style digest of mutable sim state, for determinism tests.
    #[cfg(test)]
    pub fn fingerprint(&self) -> u64 {
        fn mix(h: u64, v: u64) -> u64 {
            (h ^ v).wrapping_mul(0x100000001b3)
        }
        let mut h: u64 = 0xcbf29ce484222325;
        for c in &self.citizens {
            h = mix(h, c.pos.0.to_bits() as u64);
            h = mix(h, c.pos.1.to_bits() as u64);
            h = mix(h, c.money.to_bits() as u64);
            h = mix(h, c.needs.hunger.to_bits() as u64);
            h = mix(h, c.needs.energy.to_bits() as u64);
        }
        for b in &self.city.buildings {
            h = mix(h, b.stock.to_bits() as u64);
            h = mix(h, b.occupants.len() as u64);
        }
        h
    }
}

/// Hourly wage settlement while working. Returns (wages_paid, minted):
/// farm wages move from the employer balance; industry wages are minted.
fn settle_wage(
    c: &mut Citizen,
    city: &mut City,
    at: u16,
    tick: u64,
    events: &mut VecDeque<SimEvent>,
) -> (f32, f32) {
    if tick % TICKS_PER_HOUR != 0 {
        return (0.0, 0.0);
    }
    let Some(job) = c.job.as_mut() else { return (0.0, 0.0) };
    debug_assert_eq!(job.workplace, at, "settling wages away from the job site");
    let wage = job.wage_per_hour;
    let employer = &mut city.buildings[at as usize];
    if !employer.kind.wages_from_balance() {
        c.money += wage;
        return (wage, wage);
    }
    if employer.balance >= wage {
        employer.balance -= wage;
        c.money += wage;
        job.unpaid_hours = 0;
        employer.insolvent = false;
        return (wage, 0.0);
    }
    job.unpaid_hours += 1;
    if !employer.insolvent {
        employer.insolvent = true;
        push_event(events, SimEvent { tick, kind: EventKind::EmployerInsolvent { building: at } });
    }
    if job.unpaid_hours >= economy::UNPAID_HOURS_TO_QUIT {
        push_event(events, SimEvent { tick, kind: EventKind::WorkerQuit { citizen: c.id, building: at } });
        employer.workers.retain(|&id| id != c.id);
        c.job = None;
    }
    (0.0, 0.0)
}

fn tick_citizen(
    c: &mut Citizen,
    city: &mut City,
    rng: &mut Rng,
    tick: u64,
    hour: u32,
    night: bool,
    events: &mut VecDeque<SimEvent>,
) -> (f32, f32) {
    let mut wages = 0.0;
    let mut minted = 0.0;
    match c.state {
        CitizenState::Idle { until } => {
            if tick < until {
                return (wages, minted);
            }
            if let Some((b, act)) = ai::choose_action(c, city, hour, night) {
                start_travel(c, city, Some(b), act, tick, rng);
            } else if rng.chance(0.003) {
                // laze, occasionally wander toward a nearby corner
                let target = city.random_road_near(rng, c.tile(), 8);
                if let Some(p) = path::find_path(city, c.tile(), target) {
                    c.path = VecDeque::from(p);
                    c.state = CitizenState::Traveling { to: None, activity: Activity::Stroll };
                }
            }
        }
        CitizenState::Traveling { to, activity } => {
            if let Some(&(tx, ty)) = c.path.front() {
                let target = (tx as f32 + 0.5, ty as f32 + 0.5);
                let (dx, dy) = (target.0 - c.pos.0, target.1 - c.pos.1);
                let d = (dx * dx + dy * dy).sqrt();
                if d <= c.speed {
                    c.pos = target;
                    c.path.pop_front();
                } else {
                    c.pos.0 += dx / d * c.speed;
                    c.pos.1 += dy / d * c.speed;
                }
            }
            if c.path.is_empty() {
                match to {
                    Some(b) => arrive(c, city, b, activity, tick, events),
                    None => c.state = CitizenState::Idle { until: tick + rng.gen_range(60, 240) as u64 },
                }
            }
        }
        CitizenState::Performing { at, activity } => {
            let kind = city.buildings[at as usize].kind;
            let done = match activity {
                Activity::Sleep => {
                    c.needs.add(NeedKind::Energy, economy::SLEEP_RATE);
                    c.needs.energy >= 0.98
                }
                Activity::Shower => {
                    c.needs.add(NeedKind::Hygiene, economy::SHOWER_RATE);
                    c.needs.hygiene >= 0.98
                }
                Activity::Eat => {
                    c.needs.add(NeedKind::Hunger, economy::eat_rate(kind));
                    c.needs.hunger >= 0.97
                }
                Activity::Fun => {
                    c.needs.add(NeedKind::Fun, economy::fun_rate(kind));
                    c.needs.fun >= 0.97
                }
                Activity::Work => {
                    let (w, m) = settle_wage(c, city, at, tick, events);
                    wages = w;
                    minted = m;
                    match &c.job {
                        Some(job) => !job.in_shift(hour) || c.needs.min_value() < 0.08,
                        None => true,
                    }
                }
                Activity::Stroll => true,
            };
            if done {
                city.buildings[at as usize].occupants.retain(|&id| id != c.id);
                let door = city.buildings[at as usize].door;
                c.pos = (door.0 as f32 + 0.5, door.1 as f32 + 0.5);
                c.state = CitizenState::Idle { until: tick + rng.gen_range(20, 90) as u64 };
            }
        }
    }
    (wages, minted)
}

fn start_travel(c: &mut Citizen, city: &City, to: Option<u16>, act: Activity, tick: u64, rng: &mut Rng) {
    let Some(b) = to else { return };
    let door = city.buildings[b as usize].door;
    if c.tile() == door {
        // already at the door — walk straight in (arrive handles payment)
        c.path.clear();
        c.state = CitizenState::Traveling { to, activity: act };
        return;
    }
    match path::find_path(city, c.tile(), door) {
        Some(p) => {
            c.path = VecDeque::from(p);
            c.state = CitizenState::Traveling { to, activity: act };
        }
        None => c.state = CitizenState::Idle { until: tick + rng.gen_range(120, 360) as u64 },
    }
}

fn arrive(c: &mut Citizen, city: &mut City, b: u16, act: Activity, tick: u64, events: &mut VecDeque<SimEvent>) {
    let building = &mut city.buildings[b as usize];
    match act {
        Activity::Eat => {
            // Stock ran out mid-walk; stay silent — VenueSoldOut already fired
            // when the last meal sold.
            if building.stock < 1.0 {
                c.state = CitizenState::Idle { until: tick + 60 };
                return;
            }
            let price = economy::meal_price(building.kind);
            // Latent until money can drain mid-walk (Phase 2: rent/bills): the AI
            // pre-filters unaffordable venues and nothing spends while Traveling.
            if c.money < price {
                push_event(events, SimEvent { tick, kind: EventKind::CantAffordMeal { citizen: c.id, building: b } });
                c.state = CitizenState::Idle { until: tick + 60 };
                return;
            }
            building.stock -= 1.0;
            c.money -= price;
            building.balance += price;
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
            building.balance += price;
        }
        _ => {}
    }
    building.occupants.push(c.id);
    c.state = CitizenState::Performing { at: b, activity: act };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::ai;
    use crate::sim::citizen::{Activity, CitizenState, NeedKind, Needs};
    use crate::sim::event::EventKind;
    use crate::sim::time::TICKS_PER_HOUR;

    #[test]
    fn deterministic_across_runs() {
        let mut a = World::new(42, 30);
        let mut b = World::new(42, 30);
        for _ in 0..5000 {
            a.tick();
            b.tick();
        }
        assert_eq!(a.fingerprint(), b.fingerprint());
    }

    #[test]
    fn citizen_names_are_unique() {
        // Seed 2161 is the shipped seed; 100 adds collision pressure on the pool.
        for (seed, n) in [(2161u64, 48usize), (7, 100)] {
            let w = World::new(seed, n);
            let names: std::collections::HashSet<&str> =
                w.citizens.iter().map(|c| c.name.as_str()).collect();
            assert_eq!(names.len(), n, "duplicate names at seed {seed}");
        }
    }

    #[test]
    fn citizens_have_homes_and_most_have_jobs() {
        let w = World::new(7, 48);
        assert_eq!(w.citizens.len(), 48);
        let employed = w.citizens.iter().filter(|c| c.job.is_some()).count();
        assert!(employed >= 24, "only {employed} employed");
        for c in &w.citizens {
            assert_eq!(w.city.buildings[c.home as usize].kind, crate::sim::city::BuildingKind::Apartment);
        }
    }

    #[test]
    fn hungry_citizen_eventually_eats() {
        let mut w = World::new(13, 20);
        w.citizens[0].needs = Needs::full();
        w.citizens[0].needs.hunger = 0.12;
        w.citizens[0].money = 100.0;
        w.citizens[0].job = None;
        let money_before = w.citizens[0].money;
        let mut ate = false;
        for _ in 0..20_000 {
            w.tick();
            if matches!(w.citizens[0].state, CitizenState::Performing { activity: Activity::Eat, .. }) {
                ate = true;
                break;
            }
        }
        assert!(ate, "never started eating");
        assert!(w.citizens[0].money < money_before, "meal was free");
    }

    #[test]
    fn working_pays_wages() {
        let mut w = World::new(21, 20);
        // force citizen 0 into an always-on shift starting now (hour 0)
        let wp = w.city.buildings.iter().find(|b| b.kind.is_workplace()).unwrap().id;
        w.citizens[0].job = Some(crate::sim::citizen::Job {
            workplace: wp,
            shift_start: 0,
            shift_end: 24,
            wage_per_hour: 14.0,
            unpaid_hours: 0,
        });
        w.citizens[0].needs = Needs::full();
        let before = w.citizens[0].money;
        for _ in 0..(TICKS_PER_HOUR * 3) {
            w.tick();
        }
        assert!(w.citizens[0].money > before, "no wages paid");
    }

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

    /// End-to-end: over one full day, citizens work their shifts, sleep at
    /// night, and buy meals. Locks the sim's daily rhythm against regressions.
    #[test]
    fn full_day_cycle_behavior() {
        let mut w = World::new(99, 40);
        let mut saw_working = false;
        let mut saw_sleeping_at_night = false;
        let money_before: f32 = w.citizens.iter().map(|c| c.money).sum();
        let mut wages_paid = false;

        for _ in 0..crate::sim::time::TICKS_PER_DAY {
            w.tick();
            let hour = w.hour();
            for c in &w.citizens {
                match c.state {
                    CitizenState::Performing { activity: Activity::Work, .. } => {
                        saw_working = true;
                    }
                    CitizenState::Performing { activity: Activity::Sleep, .. } => {
                        if hour >= 22 || hour < 6 {
                            saw_sleeping_at_night = true;
                        }
                    }
                    _ => {}
                }
            }
        }

        let money_after: f32 = w.citizens.iter().map(|c| c.money).sum();
        if money_after != money_before {
            wages_paid = true;
        }
        assert!(saw_working, "nobody worked all day");
        assert!(saw_sleeping_at_night, "nobody slept at night");
        assert!(wages_paid, "economy never moved any money");
        let total_stock: f32 = w
            .city
            .buildings
            .iter()
            .filter(|b| b.kind.is_food())
            .map(|b| b.stock)
            .sum();
        assert!(total_stock > 0.0, "city ran completely out of food");
    }

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
        // 200 ticks < TICKS_PER_HOUR (600), so no hourly restock can interfere.
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

    #[test]
    fn meal_payment_credits_venue() {
        let mut w = World::new(17, 6);
        let venue = w.city.buildings.iter().find(|b| b.kind.is_food()).unwrap().id;
        w.city.buildings[venue as usize].stock = 10.0;
        for c in w.citizens.iter_mut() {
            c.needs = Needs::full();
            c.job = None;
        }
        w.citizens[0].money = 100.0;
        w.citizens[0].path.clear();
        w.citizens[0].state = CitizenState::Traveling { to: Some(venue), activity: Activity::Eat };
        let balance_before = w.city.buildings[venue as usize].balance;
        let price = crate::sim::economy::meal_price(w.city.buildings[venue as usize].kind);
        w.tick();
        let gained = w.city.buildings[venue as usize].balance - balance_before;
        assert!((gained - price).abs() < 1e-3, "venue gained {gained}, price {price}");
        assert!((w.citizens[0].money - (100.0 - price)).abs() < 1e-3);
    }

    #[test]
    fn farm_wages_come_from_farm_balance() {
        let mut w = World::new(21, 4);
        let farm =
            w.city.buildings.iter().find(|b| b.kind == BuildingKind::HydroFarm).unwrap().id;
        // Zero venue balances so no wholesale income muddies the farm's books.
        for b in w.city.buildings.iter_mut().filter(|b| b.kind.is_food()) {
            b.balance = 0.0;
        }
        w.city.buildings[farm as usize].balance = 500.0;
        for c in w.citizens.iter_mut() {
            c.needs = Needs::full();
            c.job = None;
        }
        w.citizens[0].job = Some(Job {
            workplace: farm,
            shift_start: 0,
            shift_end: 24,
            wage_per_hour: 10.0,
            unpaid_hours: 0,
        });
        w.city.buildings[farm as usize].workers.push(0);
        w.citizens[0].path.clear();
        w.citizens[0].state = CitizenState::Traveling { to: Some(farm), activity: Activity::Work };
        let money_before = w.citizens[0].money;
        for _ in 0..(TICKS_PER_HOUR * 2) {
            w.tick();
        }
        let earned = w.citizens[0].money - money_before;
        assert!(earned >= 10.0, "no hourly wage landed: {earned}");
        assert!(
            (earned / 10.0).fract().abs() < 1e-4,
            "wages not in whole-hour chunks: {earned}"
        );
        let farm_spent = 500.0 - w.city.buildings[farm as usize].balance;
        assert!((farm_spent - earned).abs() < 1e-3, "spent {farm_spent} != earned {earned}");
        assert_eq!(w.minted, 0.0, "farm wages must not mint");
    }

    #[test]
    fn industry_wages_are_minted_and_tracked() {
        let mut w = World::new(21, 4);
        let dc =
            w.city.buildings.iter().find(|b| b.kind == BuildingKind::DataCenter).unwrap().id;
        for c in w.citizens.iter_mut() {
            c.needs = Needs::full();
            c.job = None;
        }
        w.citizens[0].job = Some(Job {
            workplace: dc,
            shift_start: 0,
            shift_end: 24,
            wage_per_hour: 14.0,
            unpaid_hours: 0,
        });
        w.citizens[0].path.clear();
        w.citizens[0].state = CitizenState::Traveling { to: Some(dc), activity: Activity::Work };
        let money_before = w.citizens[0].money;
        for _ in 0..(TICKS_PER_HOUR * 2) {
            w.tick();
        }
        let earned = w.citizens[0].money - money_before;
        assert!(earned >= 14.0, "no wage landed: {earned}");
        assert!((w.minted - earned).abs() < 1e-3, "minted {} != earned {earned}", w.minted);
    }

    #[test]
    fn missed_payroll_fires_insolvency_once_then_quit_after_full_shift() {
        let mut w = World::new(21, 4);
        let farm =
            w.city.buildings.iter().find(|b| b.kind == BuildingKind::HydroFarm).unwrap().id;
        // No wholesale income for the farm: venues need balance to restock, and
        // zero stock means no meal sales can ever recapitalize them.
        for b in w.city.buildings.iter_mut().filter(|b| b.kind.is_food()) {
            b.balance = 0.0;
            b.stock = 0.0;
        }
        w.city.buildings[farm as usize].balance = 0.0;
        for c in w.citizens.iter_mut() {
            c.needs = Needs::full();
            c.job = None;
        }
        w.citizens[0].job = Some(Job {
            workplace: farm,
            shift_start: 0,
            shift_end: 24,
            wage_per_hour: 10.0,
            unpaid_hours: 0,
        });
        w.city.buildings[farm as usize].workers.push(0);
        w.citizens[0].path.clear();
        w.citizens[0].state = CitizenState::Traveling { to: Some(farm), activity: Activity::Work };

        let mut insolvent_events = 0;
        let mut quit_events = 0;
        for _ in 0..(TICKS_PER_HOUR * 9) {
            w.tick();
            for ev in w.drain_events() {
                match ev.kind {
                    EventKind::EmployerInsolvent { building } if building == farm => {
                        insolvent_events += 1
                    }
                    EventKind::WorkerQuit { citizen: 0, building } if building == farm => {
                        quit_events += 1
                    }
                    _ => {}
                }
            }
        }
        assert_eq!(insolvent_events, 1, "insolvency must edge-trigger once");
        assert_eq!(quit_events, 1, "expected exactly one quit");
        assert!(w.citizens[0].job.is_none(), "job not cleared");
        assert!(
            !w.city.buildings[farm as usize].workers.contains(&0),
            "still on the workers list"
        );
    }

    #[test]
    fn payment_resets_unpaid_count_and_insolvency_flag() {
        let mut w = World::new(21, 4);
        let farm =
            w.city.buildings.iter().find(|b| b.kind == BuildingKind::HydroFarm).unwrap().id;
        for b in w.city.buildings.iter_mut().filter(|b| b.kind.is_food()) {
            b.balance = 0.0;
        }
        w.city.buildings[farm as usize].balance = 0.0;
        for c in w.citizens.iter_mut() {
            c.needs = Needs::full();
            c.job = None;
        }
        w.citizens[0].job = Some(Job {
            workplace: farm,
            shift_start: 0,
            shift_end: 24,
            wage_per_hour: 10.0,
            unpaid_hours: 0,
        });
        w.city.buildings[farm as usize].workers.push(0);
        w.citizens[0].path.clear();
        w.citizens[0].state = CitizenState::Traveling { to: Some(farm), activity: Activity::Work };

        for _ in 0..(TICKS_PER_HOUR * 3) {
            w.tick();
        }
        assert!(w.citizens[0].job.unwrap().unpaid_hours >= 2, "no missed hours recorded");
        assert!(w.city.buildings[farm as usize].insolvent, "flag not set");

        w.city.buildings[farm as usize].balance = 500.0;
        for _ in 0..=TICKS_PER_HOUR {
            w.tick();
        }
        assert_eq!(
            w.citizens[0].job.unwrap().unpaid_hours,
            0,
            "paid hour must reset the counter"
        );
        assert!(!w.city.buildings[farm as usize].insolvent, "flag must clear on payment");
    }

    #[test]
    fn farm_staffing_capped_and_wages_match_kind() {
        for seed in [7u64, 21, 2161] {
            let w = World::new(seed, 48);
            for b in w.city.buildings.iter().filter(|b| b.kind == BuildingKind::HydroFarm) {
                assert!(
                    b.workers.len() <= economy::FARM_MAX_WORKERS,
                    "seed {seed}: farm has {} workers",
                    b.workers.len()
                );
            }
            for c in &w.citizens {
                if let Some(job) = &c.job {
                    let kind = w.city.buildings[job.workplace as usize].kind;
                    let (lo, hi) = economy::wage_range(kind);
                    assert!(
                        job.wage_per_hour >= lo && job.wage_per_hour <= hi,
                        "seed {seed}: {kind:?} wage {} outside [{lo}, {hi}]",
                        job.wage_per_hour
                    );
                }
            }
        }
    }

    #[test]
    fn final_shift_hour_paid_then_worker_leaves() {
        let mut w = World::new(21, 4);
        let dc =
            w.city.buildings.iter().find(|b| b.kind == BuildingKind::DataCenter).unwrap().id;
        for c in w.citizens.iter_mut() {
            c.needs = Needs::full();
            c.job = None;
        }
        // One-hour shift: the 01:00 boundary both pays the trailing hour and ends
        // the shift — settle-before-done means the wage lands, then they walk out.
        w.citizens[0].job = Some(Job {
            workplace: dc,
            shift_start: 0,
            shift_end: 1,
            wage_per_hour: 10.0,
            unpaid_hours: 0,
        });
        w.citizens[0].path.clear();
        w.citizens[0].state = CitizenState::Traveling { to: Some(dc), activity: Activity::Work };
        let money_before = w.citizens[0].money;
        for _ in 0..TICKS_PER_HOUR {
            w.tick();
        }
        let earned = w.citizens[0].money - money_before;
        assert!((earned - 10.0).abs() < 1e-3, "final hour not paid exactly once: {earned}");
        assert!(
            matches!(w.citizens[0].state, CitizenState::Idle { .. }),
            "worker should have left after the shift-end settlement"
        );
    }
}
