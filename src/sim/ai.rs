use crate::sim::citizen::{Activity, Citizen, NeedKind};
use crate::sim::city::{BuildingKind, City};
use crate::sim::economy;

/// How loudly an unmet need screams. 0 when full, ~3 when empty, non-linear.
pub fn urgency(value: f32) -> f32 {
    let d = 1.0 - value.clamp(0.0, 1.0);
    d * d * (1.0 + 2.0 * d)
}

const CRITICAL: f32 = 0.15;
/// Needs above this are not worth acting on.
const ACT_BELOW: f32 = 0.8;
const DIST_PENALTY: f32 = 0.01;
const MIN_SCORE: f32 = 0.02;

/// Pick the best (building, activity) for an idle citizen, or None to laze.
pub fn choose_action(
    c: &Citizen,
    city: &City,
    hour: u32,
    is_night: bool,
) -> Option<(u16, Activity)> {
    let critical = c.needs.min_value() < CRITICAL;

    // Work is scheduled, not scored — unless something is critical.
    if !critical {
        if let Some(job) = &c.job {
            if job.in_shift(hour) {
                return Some((job.workplace, Activity::Work));
            }
        }
    }

    let from = c.tile();
    let mut best: Option<(f32, u16, Activity)> = None;

    let dist = |door: (i32, i32)| {
        ((door.0 - from.0).abs() + (door.1 - from.1).abs()) as f32 * DIST_PENALTY
    };
    let weight = |k: NeedKind| c.personality.weights[k.index()];

    // Home: sleep & shower, free.
    let home = &city.buildings[c.home as usize];
    if c.needs.energy < ACT_BELOW {
        let night_bonus = if is_night { 1.5 } else { 1.0 };
        let s = urgency(c.needs.energy) * weight(NeedKind::Energy) * night_bonus - dist(home.door);
        if s > MIN_SCORE && best.map_or(true, |(bs, _, _)| s > bs) {
            best = Some((s, home.id, Activity::Sleep));
        }
    }
    if c.needs.hygiene < ACT_BELOW {
        let s = urgency(c.needs.hygiene) * weight(NeedKind::Hygiene) - dist(home.door);
        if s > MIN_SCORE && best.map_or(true, |(bs, _, _)| s > bs) {
            best = Some((s, home.id, Activity::Shower));
        }
    }

    // Food venues: must have stock and be affordable.
    if c.needs.hunger < ACT_BELOW {
        for b in city.buildings_of(|k: BuildingKind| k.is_food()) {
            if b.stock < 1.0 || c.money < economy::meal_price(b.kind) {
                continue;
            }
            let s = urgency(c.needs.hunger) * weight(NeedKind::Hunger) - dist(b.door);
            if s > MIN_SCORE && best.map_or(true, |(bs, _, _)| s > bs) {
                best = Some((s, b.id, Activity::Eat));
            }
        }
    }

    // Leisure.
    if c.needs.fun < ACT_BELOW {
        for b in city.buildings_of(|k: BuildingKind| k.is_leisure()) {
            if c.money < economy::fun_price(b.kind) {
                continue;
            }
            let s = urgency(c.needs.fun) * weight(NeedKind::Fun) - dist(b.door);
            if s > MIN_SCORE && best.map_or(true, |(bs, _, _)| s > bs) {
                best = Some((s, b.id, Activity::Fun));
            }
        }
    }

    best.map(|(_, b, a)| (b, a))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::citizen::{Citizen, Job};
    use crate::sim::city::City;
    use crate::sim::rng::Rng;

    fn setup() -> (City, Citizen) {
        let mut rng = Rng::new(11);
        let city = City::generate(&mut rng);
        let home = city.buildings.iter().find(|b| b.kind == BuildingKind::Apartment).unwrap();
        let door = (home.door.0 as f32 + 0.5, home.door.1 as f32 + 0.5);
        let mut c = Citizen::spawn(&mut rng, 0, home.id, door);
        c.needs = crate::sim::citizen::Needs::full();
        c.job = None;
        (city, c)
    }

    #[test]
    fn urgency_grows_as_need_empties() {
        assert!(urgency(0.1) > urgency(0.5));
        assert!(urgency(0.5) > urgency(0.9));
        assert!(urgency(1.0) < 0.01);
    }

    #[test]
    fn hungry_citizen_with_money_eats() {
        let (city, mut c) = setup();
        c.needs.hunger = 0.1;
        c.money = 100.0;
        let (b, act) = choose_action(&c, &city, 12, false).expect("action");
        assert_eq!(act, Activity::Eat);
        assert!(city.buildings[b as usize].kind.is_food());
    }

    #[test]
    fn broke_citizen_does_not_buy_food() {
        let (city, mut c) = setup();
        c.needs.hunger = 0.1;
        c.money = 0.0;
        let choice = choose_action(&c, &city, 12, false);
        if let Some((b, act)) = choice {
            assert!(
                !(act == Activity::Eat && city.buildings[b as usize].kind.is_food()),
                "bought food with no money"
            );
        }
    }

    #[test]
    fn satisfied_citizen_does_nothing() {
        let (city, c) = setup();
        assert!(choose_action(&c, &city, 12, false).is_none());
    }

    #[test]
    fn employed_citizen_works_during_shift() {
        let (city, mut c) = setup();
        let wp = city.buildings.iter().find(|b| b.kind.is_workplace()).unwrap();
        c.job = Some(Job { workplace: wp.id, shift_start: 8, shift_end: 16, wage_per_hour: 14.0 });
        let (b, act) = choose_action(&c, &city, 10, false).expect("action");
        assert_eq!(act, Activity::Work);
        assert_eq!(b, wp.id);
    }

    #[test]
    fn critical_need_overrides_work() {
        let (city, mut c) = setup();
        let wp = city.buildings.iter().find(|b| b.kind.is_workplace()).unwrap();
        c.job = Some(Job { workplace: wp.id, shift_start: 8, shift_end: 16, wage_per_hour: 14.0 });
        c.needs.hunger = 0.05;
        c.money = 100.0;
        let (_, act) = choose_action(&c, &city, 10, false).expect("action");
        assert_eq!(act, Activity::Eat);
    }
}
