use crate::sim::city::{BuildingKind, City};
use crate::sim::economy;
use std::collections::VecDeque;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TruckState {
    /// Parked at the home farm, available to dispatch.
    Parked,
    /// Loaded, driving to a venue's door.
    Outbound { venue: u16 },
    /// Driving back to the home farm's door.
    Returning,
}

pub struct Truck {
    pub id: usize,
    pub home_farm: u16,
    /// Citizen currently driving; Some only between dispatch and park.
    pub driver: Option<usize>,
    pub pos: (f32, f32),
    pub path: VecDeque<(i32, i32)>,
    pub speed: f32,
    pub cargo: f32,
    pub state: TruckState,
}

/// City-wide (supply, demand) for the dynamic price: supply = meals on farms,
/// demand = unmet room across open food venues.
pub fn supply_demand(city: &City) -> (f32, f32) {
    let supply = city
        .buildings
        .iter()
        .filter(|b| b.kind == BuildingKind::HydroFarm)
        .map(|b| b.stock)
        .sum();
    let demand = city
        .buildings
        .iter()
        .filter(|b| b.kind.is_food() && b.open())
        .map(|b| (economy::STOCK_CAP - b.stock).max(0.0))
        .sum();
    (supply, demand)
}

/// Lowest-stock open food venue below the order threshold that no truck is
/// already serving (`served` = venues currently Outbound-targeted). Ties by id.
pub fn neediest_venue(city: &City, served: &[u16]) -> Option<u16> {
    city.buildings
        .iter()
        .filter(|b| b.kind.is_food() && b.open() && b.stock < economy::ORDER_THRESHOLD)
        .filter(|b| !served.contains(&b.id))
        .min_by(|a, b| a.stock.partial_cmp(&b.stock).unwrap().then(a.id.cmp(&b.id)))
        .map(|b| b.id)
}

/// Meals to load: the min of truck capacity, farm inventory, and venue room.
pub fn load_amount(capacity: f32, farm_stock: f32, venue_room: f32) -> f32 {
    capacity.min(farm_stock).min(venue_room).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::city::City;
    use crate::sim::rng::Rng;

    #[test]
    fn neediest_picks_lowest_open_unserved() {
        let mut city = City::generate(&mut Rng::new(2));
        let food: Vec<u16> =
            city.buildings.iter().filter(|b| b.kind.is_food()).map(|b| b.id).collect();
        assert!(food.len() >= 3, "need ≥3 food venues for this test");
        for &id in &food {
            city.buildings[id as usize].stock = economy::STOCK_CAP; // full -> not needy
        }
        city.buildings[food[1] as usize].stock = 5.0; // neediest
        city.buildings[food[2] as usize].stock = 10.0; // needy but higher
        // food[1] is neediest; if it's already served, food[2] wins.
        assert_eq!(neediest_venue(&city, &[]), Some(food[1]));
        assert_eq!(neediest_venue(&city, &[food[1]]), Some(food[2]));
    }

    #[test]
    fn neediest_skips_full_venues() {
        let mut city = City::generate(&mut Rng::new(2));
        for b in city.buildings.iter_mut().filter(|b| b.kind.is_food()) {
            b.stock = economy::STOCK_CAP;
        }
        assert_eq!(neediest_venue(&city, &[]), None);
    }

    #[test]
    fn load_amount_is_the_min() {
        assert_eq!(load_amount(30.0, 100.0, 100.0), 30.0); // capacity binds
        assert_eq!(load_amount(30.0, 12.0, 100.0), 12.0); // farm stock binds
        assert_eq!(load_amount(30.0, 100.0, 8.0), 8.0); // venue room binds
        assert_eq!(load_amount(30.0, 100.0, -5.0), 0.0); // no negative loads
    }
}
