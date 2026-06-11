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
}

const SHIFTS: [(u32, u32); 3] = [(8, 16), (16, 24), (0, 8)];
/// Weighted shift pick: most people work days.
const SHIFT_WEIGHTS: [f32; 3] = [0.6, 0.25, 0.15];

impl World {
    pub fn new(seed: u64, n_citizens: usize) -> World {
        let mut rng = Rng::new(seed);
        let city = City::generate(&mut rng);
        let mut world = World { rng, city, citizens: vec![], tick: 0, seed, events: VecDeque::new(), wages_today: 0.0 };

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

        for i in 0..n_citizens {
            let home = homes[i % homes.len()];
            let door = world.city.buildings[home as usize].door;
            let pos = (door.0 as f32 + 0.5, door.1 as f32 + 0.5);
            let mut c = Citizen::spawn(&mut world.rng, i, home, pos);
            // ~80% employed
            if world.rng.chance(0.8) {
                let wp = workplaces[i % workplaces.len()];
                let roll = world.rng.gen_f32();
                let shift = if roll < SHIFT_WEIGHTS[0] {
                    SHIFTS[0]
                } else if roll < SHIFT_WEIGHTS[0] + SHIFT_WEIGHTS[1] {
                    SHIFTS[1]
                } else {
                    SHIFTS[2]
                };
                c.job = Some(Job {
                    workplace: wp,
                    shift_start: shift.0,
                    shift_end: shift.1,
                    wage_per_hour: world.rng.gen_f32_range(11.0, 18.0),
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
            economy::produce_food(&mut self.city, hour);
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
            self.wages_today += tick_citizen(c, city, rng, tick, hour, night, events);
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

fn tick_citizen(
    c: &mut Citizen,
    city: &mut City,
    rng: &mut Rng,
    tick: u64,
    hour: u32,
    night: bool,
    events: &mut VecDeque<SimEvent>,
) -> f32 {
    let mut wages = 0.0;
    match c.state {
        CitizenState::Idle { until } => {
            if tick < until {
                return wages;
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
                    if let Some(job) = &c.job {
                        let pay = job.wage_per_hour / TICKS_PER_HOUR as f32;
                        c.money += pay;
                        wages = pay;
                        !job.in_shift(hour) || c.needs.min_value() < 0.08
                    } else {
                        true
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
    wages
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
            if building.stock < 1.0 {
                c.state = CitizenState::Idle { until: tick + 60 };
                return;
            }
            let price = economy::meal_price(building.kind);
            if c.money < price {
                push_event(events, SimEvent { tick, kind: EventKind::CantAffordMeal { citizen: c.id, building: b } });
                c.state = CitizenState::Idle { until: tick + 60 };
                return;
            }
            building.stock -= 1.0;
            c.money -= price;
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
}
