use macroquad::prelude::*;
use crate::sim::world::World;

/// Discrete status band for a need value in [0, 1].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Band {
    High,
    Medium,
    Low,
}

/// Pure (no macroquad) so it stays unit-testable.
pub fn band(value: f32) -> Band {
    if value >= 0.6 {
        Band::High
    } else if value >= 0.3 {
        Band::Medium
    } else {
        Band::Low
    }
}

fn band_color(b: Band) -> Color {
    match b {
        Band::High => Color::new(0.3, 0.95, 0.5, 1.0),
        Band::Medium => Color::new(0.95, 0.8, 0.25, 1.0),
        Band::Low => Color::new(1.0, 0.25, 0.4, 1.0),
    }
}

pub struct Roster {
    /// Citizen ids sorted alphabetically by name; the population is fixed
    /// after world creation, so this is computed once.
    order: Vec<usize>,
    scroll: f32,
}

impl Roster {
    pub fn new(world: &World) -> Roster {
        let mut order: Vec<usize> = (0..world.citizens.len()).collect();
        order.sort_by(|&a, &b| world.citizens[a].name.cmp(&world.citizens[b].name));
        Roster { order, scroll: 0.0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn band_boundaries() {
        assert_eq!(band(0.0), Band::Low);
        assert_eq!(band(0.29), Band::Low);
        assert_eq!(band(0.3), Band::Medium);
        assert_eq!(band(0.59), Band::Medium);
        assert_eq!(band(0.6), Band::High);
        assert_eq!(band(1.0), Band::High);
    }

    #[test]
    fn roster_order_is_alphabetical_and_complete() {
        let world = crate::sim::world::World::new(2161, 48);
        let r = Roster::new(&world);
        assert_eq!(r.order.len(), world.citizens.len());
        for pair in r.order.windows(2) {
            assert!(
                world.citizens[pair[0]].name <= world.citizens[pair[1]].name,
                "roster not sorted: {} before {}",
                world.citizens[pair[0]].name,
                world.citizens[pair[1]].name
            );
        }
    }
}
