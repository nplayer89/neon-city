use crate::sim::rng::Rng;
use crate::sim::time::TICKS_PER_HOUR;
use std::collections::VecDeque;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NeedKind {
    Hunger,
    Energy,
    Hygiene,
    Fun,
}

pub const NEED_KINDS: [NeedKind; 4] = [
    NeedKind::Hunger,
    NeedKind::Energy,
    NeedKind::Hygiene,
    NeedKind::Fun,
];

impl NeedKind {
    pub fn label(&self) -> &'static str {
        match self {
            NeedKind::Hunger => "HUNGER",
            NeedKind::Energy => "ENERGY",
            NeedKind::Hygiene => "HYGIENE",
            NeedKind::Fun => "FUN",
        }
    }
    fn index(&self) -> usize {
        *self as usize
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Needs {
    pub hunger: f32,
    pub energy: f32,
    pub hygiene: f32,
    pub fun: f32,
}

impl Needs {
    pub fn full() -> Self {
        Needs { hunger: 1.0, energy: 1.0, hygiene: 1.0, fun: 1.0 }
    }
    pub fn get(&self, k: NeedKind) -> f32 {
        match k {
            NeedKind::Hunger => self.hunger,
            NeedKind::Energy => self.energy,
            NeedKind::Hygiene => self.hygiene,
            NeedKind::Fun => self.fun,
        }
    }
    pub fn add(&mut self, k: NeedKind, dv: f32) {
        let v = (self.get(k) + dv).clamp(0.0, 1.0);
        match k {
            NeedKind::Hunger => self.hunger = v,
            NeedKind::Energy => self.energy = v,
            NeedKind::Hygiene => self.hygiene = v,
            NeedKind::Fun => self.fun = v,
        }
    }
    pub fn min_value(&self) -> f32 {
        self.hunger.min(self.energy).min(self.hygiene).min(self.fun)
    }
}

/// Base decay per tick, indexed [hunger, energy, hygiene, fun]:
/// full bar drains in 16h / 20h / 24h / 14h of game time.
const BASE_DECAY: [f32; 4] = [
    1.0 / (16.0 * TICKS_PER_HOUR as f32),
    1.0 / (20.0 * TICKS_PER_HOUR as f32),
    1.0 / (24.0 * TICKS_PER_HOUR as f32),
    1.0 / (14.0 * TICKS_PER_HOUR as f32),
];

#[derive(Clone, Copy, Debug)]
pub struct Personality {
    pub archetype: &'static str,
    /// Importance multiplier per need when scoring actions.
    pub weights: [f32; 4],
    /// Decay-rate multiplier per need.
    pub decay_mult: [f32; 4],
}

const ARCHETYPES: [Personality; 5] = [
    Personality { archetype: "Balanced", weights: [1.0, 1.0, 1.0, 1.0], decay_mult: [1.0, 1.0, 1.0, 1.0] },
    Personality { archetype: "Workaholic", weights: [1.0, 0.9, 0.9, 0.6], decay_mult: [1.0, 1.1, 1.0, 0.8] },
    Personality { archetype: "Hedonist", weights: [1.0, 0.9, 0.8, 1.6], decay_mult: [1.0, 1.0, 1.0, 1.3] },
    Personality { archetype: "Slob", weights: [1.2, 1.0, 0.5, 1.1], decay_mult: [1.1, 1.0, 0.7, 1.0] },
    Personality { archetype: "Neat Freak", weights: [0.9, 1.0, 1.7, 0.9], decay_mult: [1.0, 1.0, 1.4, 1.0] },
];

#[derive(Clone, Copy, Debug)]
pub struct Job {
    pub workplace: u16,
    pub shift_start: u32,
    pub shift_end: u32,
    pub wage_per_hour: f32,
}

impl Job {
    pub fn in_shift(&self, hour: u32) -> bool {
        self.shift_start <= hour && hour < self.shift_end
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Activity {
    Sleep,
    Shower,
    Eat,
    Fun,
    Work,
    Stroll,
}

impl Activity {
    pub fn label(&self) -> &'static str {
        match self {
            Activity::Sleep => "Sleeping",
            Activity::Shower => "Showering",
            Activity::Eat => "Eating",
            Activity::Fun => "Having fun",
            Activity::Work => "Working",
            Activity::Stroll => "Strolling",
        }
    }
    pub fn need(&self) -> Option<NeedKind> {
        match self {
            Activity::Sleep => Some(NeedKind::Energy),
            Activity::Shower => Some(NeedKind::Hygiene),
            Activity::Eat => Some(NeedKind::Hunger),
            Activity::Fun => Some(NeedKind::Fun),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CitizenState {
    /// Thinking; will pick a new action at `until` tick.
    Idle { until: u64 },
    /// Walking. `to: None` means an aimless stroll.
    Traveling { to: Option<u16>, activity: Activity },
    Performing { at: u16, activity: Activity },
}

pub struct Citizen {
    pub id: usize,
    pub name: String,
    pub pos: (f32, f32),
    pub home: u16,
    pub job: Option<Job>,
    pub needs: Needs,
    pub money: f32,
    pub personality: Personality,
    pub state: CitizenState,
    pub path: VecDeque<(i32, i32)>,
    /// Tiles per tick.
    pub speed: f32,
}

const FIRST_NAMES: [&str; 20] = [
    "Aria", "Juno", "Kai", "Vesper", "Orion", "Nova", "Ezra", "Lyra", "Dex", "Mira",
    "Caspian", "Zara", "Niko", "Echo", "Sol", "Indra", "Rune", "Vega", "Atlas", "Wren",
];
const LAST_NAMES: [&str; 16] = [
    "Tanaka", "Voss", "Okafor", "Reyes", "Stellan", "Qiu", "Marlowe", "Ito",
    "Kade", "Sorenson", "Anand", "Petrov", "Calloway", "Nyx", "Moreau", "Zhou",
];

impl Citizen {
    pub fn spawn(rng: &mut Rng, id: usize, home: u16, door_pos: (f32, f32)) -> Citizen {
        let first = FIRST_NAMES[rng.gen_range(0, FIRST_NAMES.len() as i32) as usize];
        let last = LAST_NAMES[rng.gen_range(0, LAST_NAMES.len() as i32) as usize];
        let personality = ARCHETYPES[rng.gen_range(0, ARCHETYPES.len() as i32) as usize];
        Citizen {
            id,
            name: format!("{first} {last}"),
            pos: door_pos,
            home,
            job: None,
            needs: Needs {
                hunger: rng.gen_f32_range(0.5, 1.0),
                energy: rng.gen_f32_range(0.5, 1.0),
                hygiene: rng.gen_f32_range(0.5, 1.0),
                fun: rng.gen_f32_range(0.5, 1.0),
            },
            money: rng.gen_f32_range(40.0, 80.0),
            personality,
            state: CitizenState::Idle { until: 0 },
            path: VecDeque::new(),
            speed: rng.gen_f32_range(0.045, 0.06),
        }
    }

    pub fn tile(&self) -> (i32, i32) {
        (self.pos.0 as i32, self.pos.1 as i32)
    }

    pub fn decay_needs(&mut self) {
        for k in NEED_KINDS {
            let i = k.index();
            self.needs.add(k, -BASE_DECAY[i] * self.personality.decay_mult[i]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::rng::Rng;

    #[test]
    fn needs_decay_over_time() {
        let mut c = Citizen::spawn(&mut Rng::new(5), 0, 0, (1.5, 0.5));
        c.needs = Needs::full();
        for _ in 0..1000 {
            c.decay_needs();
        }
        assert!(c.needs.hunger < 1.0);
        assert!(c.needs.energy < 1.0);
        assert!(c.needs.hygiene < 1.0);
        assert!(c.needs.fun < 1.0);
        assert!(c.needs.hunger > 0.5, "decay too fast");
    }

    #[test]
    fn needs_clamp_at_zero() {
        let mut c = Citizen::spawn(&mut Rng::new(5), 0, 0, (1.5, 0.5));
        for _ in 0..2_000_000 {
            c.decay_needs();
        }
        assert!(c.needs.hunger >= 0.0 && c.needs.fun >= 0.0);
    }

    #[test]
    fn spawn_is_deterministic() {
        let a = Citizen::spawn(&mut Rng::new(9), 3, 1, (0.5, 0.5));
        let b = Citizen::spawn(&mut Rng::new(9), 3, 1, (0.5, 0.5));
        assert_eq!(a.name, b.name);
        assert_eq!(a.personality.archetype, b.personality.archetype);
        assert_eq!(a.money, b.money);
    }
}
