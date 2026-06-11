use crate::sim::ai;
use crate::sim::citizen::{Activity, Citizen, CitizenState, Job, NeedKind};
use crate::sim::city::{BuildingKind, City};
use crate::sim::economy;
use crate::sim::path;
use crate::sim::rng::Rng;
use crate::sim::time::{self, TICKS_PER_HOUR};
use std::collections::VecDeque;

pub struct World {
    pub rng: Rng,
    pub city: City,
    pub citizens: Vec<Citizen>,
    pub tick: u64,
    pub seed: u64,
}

const SHIFTS: [(u32, u32); 3] = [(8, 16), (16, 24), (0, 8)];
/// Weighted shift pick: most people work days.
const SHIFT_WEIGHTS: [f32; 3] = [0.6, 0.25, 0.15];

impl World {
    pub fn new(seed: u64, n_citizens: usize) -> World {
        let mut rng = Rng::new(seed);
        let city = City::generate(&mut rng);
        let mut world = World { rng, city, citizens: vec![], tick: 0, seed };

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
        for c in self.citizens.iter_mut() {
            c.decay_needs();
            tick_citizen(c, city, rng, tick, hour, night);
        }
    }

    /// FNV-style digest of mutable sim state, for determinism tests.
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
) {
    match c.state {
        CitizenState::Idle { until } => {
            if tick < until {
                return;
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
                    Some(b) => arrive(c, city, b, activity, tick),
                    None => c.state = CitizenState::Idle { until: tick + rng.gen_range(60, 240) as u64 },
                }
            }
        }
        CitizenState::Performing { at, activity } => {
            let kind = city.buildings[at as usize].kind;
            let mut done = false;
            match activity {
                Activity::Sleep => {
                    c.needs.add(NeedKind::Energy, economy::SLEEP_RATE);
                    done = c.needs.energy >= 0.98;
                }
                Activity::Shower => {
                    c.needs.add(NeedKind::Hygiene, economy::SHOWER_RATE);
                    done = c.needs.hygiene >= 0.98;
                }
                Activity::Eat => {
                    c.needs.add(NeedKind::Hunger, economy::eat_rate(kind));
                    done = c.needs.hunger >= 0.97;
                }
                Activity::Fun => {
                    c.needs.add(NeedKind::Fun, economy::fun_rate(kind));
                    done = c.needs.fun >= 0.97;
                }
                Activity::Work => {
                    if let Some(job) = &c.job {
                        c.money += job.wage_per_hour / TICKS_PER_HOUR as f32;
                        done = !job.in_shift(hour) || c.needs.min_value() < 0.08;
                    } else {
                        done = true;
                    }
                }
                Activity::Stroll => done = true,
            }
            if done {
                city.buildings[at as usize].occupants.retain(|&id| id != c.id);
                let door = city.buildings[at as usize].door;
                c.pos = (door.0 as f32 + 0.5, door.1 as f32 + 0.5);
                c.state = CitizenState::Idle { until: tick + rng.gen_range(20, 90) as u64 };
            }
        }
    }
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

fn arrive(c: &mut Citizen, city: &mut City, b: u16, act: Activity, tick: u64) {
    let building = &mut city.buildings[b as usize];
    match act {
        Activity::Eat => {
            let price = economy::meal_price(building.kind);
            if building.stock < 1.0 || c.money < price {
                c.state = CitizenState::Idle { until: tick + 60 };
                return;
            }
            building.stock -= 1.0;
            c.money -= price;
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
    use crate::sim::citizen::{Activity, CitizenState, Needs};
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
}
