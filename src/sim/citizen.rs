use crate::sim::rng::Rng;
use crate::sim::time::TICKS_PER_HOUR;
use std::collections::{HashSet, VecDeque};

/// Variant order is load-bearing: it indexes BASE_DECAY, Personality::weights
/// and Personality::decay_mult. Do not reorder or insert variants.
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
    pub fn index(&self) -> usize {
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
    #[cfg(test)]
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

const FIRST_NAMES: [&str; 48] = [
    "Aria", "Juno", "Kai", "Vesper", "Orion", "Nova", "Ezra", "Lyra", "Dex", "Mira",
    "Caspian", "Zara", "Niko", "Echo", "Sol", "Indra", "Rune", "Vega", "Atlas", "Wren",
    "Kira", "Soren", "Ravi", "Luna", "Jax", "Imara", "Theo", "Sable", "Yuki", "Dario",
    "Elara", "Finn", "Noor", "Cassia", "Remy", "Tova", "Idris", "Maeve", "Zephyr", "Anya",
    "Bodhi", "Selene", "Cyrus", "Ines", "Kenji", "Priya", "Malik", "Nadia",
];
const LAST_NAMES: [&str; 32] = [
    "Tanaka", "Voss", "Okafor", "Reyes", "Stellan", "Qiu", "Marlowe", "Ito",
    "Kade", "Sorenson", "Anand", "Petrov", "Calloway", "Nyx", "Moreau", "Zhou",
    "Halloran", "Mbeki", "Lindqvist", "Duarte", "Kowalski", "Sato", "Vance", "Iyer",
    "Brandt", "Okonkwo", "Castillo", "Nakamura", "Eriksen", "Adeyemi", "Solano", "Mercer",
];

/// Draw a "First Last" name not already in `used`, inserting the result.
/// Retries against the random pool, then falls back to generational suffixes
/// ("Dex Petrov II") so uniqueness survives pool exhaustion (move-ins, births).
pub fn unique_name(rng: &mut Rng, used: &mut HashSet<String>) -> String {
    let mut base = String::new();
    for _ in 0..64 {
        let first = FIRST_NAMES[rng.gen_range(0, FIRST_NAMES.len() as i32) as usize];
        let last = LAST_NAMES[rng.gen_range(0, LAST_NAMES.len() as i32) as usize];
        base = format!("{first} {last}");
        if used.insert(base.clone()) {
            return base;
        }
    }
    let mut n = 2;
    loop {
        let name = format!("{base} {}", roman(n));
        if used.insert(name.clone()) {
            return name;
        }
        n += 1;
    }
}

fn roman(mut n: u32) -> String {
    const VALS: [(u32, &str); 13] = [
        (1000, "M"), (900, "CM"), (500, "D"), (400, "CD"), (100, "C"), (90, "XC"),
        (50, "L"), (40, "XL"), (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I"),
    ];
    let mut s = String::new();
    for (v, r) in VALS {
        while n >= v {
            s.push_str(r);
            n -= v;
        }
    }
    s
}

impl Citizen {
    pub fn spawn(rng: &mut Rng, id: usize, home: u16, door_pos: (f32, f32), used_names: &mut HashSet<String>) -> Citizen {
        let name = unique_name(rng, used_names);
        let personality = ARCHETYPES[rng.gen_range(0, ARCHETYPES.len() as i32) as usize];
        Citizen {
            id,
            name,
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

    fn spawn_one(seed: u64) -> Citizen {
        Citizen::spawn(&mut Rng::new(seed), 0, 0, (1.5, 0.5), &mut HashSet::new())
    }

    #[test]
    fn needs_decay_over_time() {
        let mut c = spawn_one(5);
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
        let mut c = spawn_one(5);
        for _ in 0..2_000_000 {
            c.decay_needs();
        }
        assert!(c.needs.hunger >= 0.0 && c.needs.fun >= 0.0);
    }

    #[test]
    fn spawn_is_deterministic() {
        let a = Citizen::spawn(&mut Rng::new(9), 3, 1, (0.5, 0.5), &mut HashSet::new());
        let b = Citizen::spawn(&mut Rng::new(9), 3, 1, (0.5, 0.5), &mut HashSet::new());
        assert_eq!(a.name, b.name);
        assert_eq!(a.personality.archetype, b.personality.archetype);
        assert_eq!(a.money, b.money);
    }

    #[test]
    fn unique_name_survives_pool_exhaustion() {
        // More draws than first+last combos, forcing the suffix fallback.
        let pool = FIRST_NAMES.len() * LAST_NAMES.len();
        let draws = pool + 100;
        let mut rng = Rng::new(11);
        let mut used = HashSet::new();
        let names: Vec<String> = (0..draws).map(|_| unique_name(&mut rng, &mut used)).collect();
        let distinct: HashSet<&String> = names.iter().collect();
        assert_eq!(distinct.len(), draws);
        assert!(names.iter().any(|n| n.ends_with(" II")), "no generational suffix used");
    }

    #[test]
    fn need_kind_discriminants_match_array_order() {
        assert_eq!(NeedKind::Hunger.index(), 0);
        assert_eq!(NeedKind::Energy.index(), 1);
        assert_eq!(NeedKind::Hygiene.index(), 2);
        assert_eq!(NeedKind::Fun.index(), 3);
    }
}
