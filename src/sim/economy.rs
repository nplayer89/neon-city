use crate::sim::city::{BuildingKind, City};
use crate::sim::time::TICKS_PER_HOUR;

pub fn meal_price(kind: BuildingKind) -> f32 {
    match kind {
        BuildingKind::NoodleBar => 12.0,
        BuildingKind::VendingPlaza => 5.0,
        _ => 0.0,
    }
}

/// Hunger restored per tick while eating.
pub fn eat_rate(kind: BuildingKind) -> f32 {
    match kind {
        BuildingKind::NoodleBar => 1.0 / 300.0,
        BuildingKind::VendingPlaza => 1.0 / 450.0,
        _ => 0.0,
    }
}

pub fn fun_price(kind: BuildingKind) -> f32 {
    match kind {
        BuildingKind::Arcade => 8.0,
        _ => 0.0,
    }
}

/// Fun restored per tick.
pub fn fun_rate(kind: BuildingKind) -> f32 {
    match kind {
        BuildingKind::Arcade => 1.0 / 900.0,
        BuildingKind::HoloPark => 1.0 / 1500.0,
        _ => 0.0,
    }
}

pub const SLEEP_RATE: f32 = 1.0 / (7.0 * TICKS_PER_HOUR as f32);
pub const SHOWER_RATE: f32 = 1.0 / (0.4 * TICKS_PER_HOUR as f32);
pub const STOCK_CAP: f32 = 60.0;
/// Meals produced per hydro farm per production hour (06:00–22:00).
pub const FARM_OUTPUT_PER_HOUR: f32 = 6.0;

/// Hourly tick: farms grow food, distributed evenly to food venues.
pub fn produce_food(city: &mut City, hour: u32) {
    if !(6..22).contains(&hour) {
        return;
    }
    let farms = city
        .buildings
        .iter()
        .filter(|b| b.kind == BuildingKind::HydroFarm)
        .count() as f32;
    let venues: Vec<usize> = city
        .buildings
        .iter()
        .filter(|b| b.kind.is_food())
        .map(|b| b.id as usize)
        .collect();
    if venues.is_empty() || farms == 0.0 {
        return;
    }
    let share = farms * FARM_OUTPUT_PER_HOUR / venues.len() as f32;
    for id in venues {
        let b = &mut city.buildings[id];
        b.stock = (b.stock + share).min(STOCK_CAP);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::rng::Rng;

    #[test]
    fn farms_stock_food_venues() {
        let mut city = City::generate(&mut Rng::new(2));
        for b in city.buildings.iter_mut().filter(|b| b.kind.is_food()) {
            b.stock = 0.0;
        }
        produce_food(&mut city, 10);
        for b in city.buildings.iter().filter(|b| b.kind.is_food()) {
            assert!(b.stock > 0.0);
        }
    }

    #[test]
    fn no_production_at_night() {
        let mut city = City::generate(&mut Rng::new(2));
        for b in city.buildings.iter_mut().filter(|b| b.kind.is_food()) {
            b.stock = 0.0;
        }
        produce_food(&mut city, 3);
        for b in city.buildings.iter().filter(|b| b.kind.is_food()) {
            assert_eq!(b.stock, 0.0);
        }
    }

    #[test]
    fn stock_caps() {
        let mut city = City::generate(&mut Rng::new(2));
        for _ in 0..1000 {
            produce_food(&mut city, 10);
        }
        for b in city.buildings.iter().filter(|b| b.kind.is_food()) {
            assert!(b.stock <= STOCK_CAP);
        }
    }
}
