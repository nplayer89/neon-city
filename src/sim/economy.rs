use crate::sim::city::{BuildingKind, City};
use crate::sim::time::TICKS_PER_HOUR;

pub fn meal_price(kind: BuildingKind) -> f32 {
    match kind {
        BuildingKind::NoodleBar => 15.0,
        BuildingKind::VendingPlaza => 10.0,
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
/// Base wholesale price per meal; the dynamic price floats around it.
pub const WHOLESALE_BASE: f32 = 7.0;
/// Dynamic wholesale band: price = WHOLESALE_BASE * clamp(demand/(supply+1), LO, HI).
pub const PRICE_LO_MULT: f32 = 0.6;
pub const PRICE_HI_MULT: f32 = 1.6;
/// Farms hold inventory up to this (larger than the venue STOCK_CAP).
pub const FARM_STOCK_CAP: f32 = 120.0;
/// A venue is an open purchase order when stock drops below this.
pub const ORDER_THRESHOLD: f32 = 20.0;
/// Meals a truck carries per run.
pub const TRUCK_CAPACITY: f32 = 30.0;
/// Truck travel speed, tiles/tick (~2.4× a citizen).
pub const TRUCK_SPEED: f32 = 0.12;
/// Consecutive broke hours before a food venue closes for good.
pub const CLOSURE_GRACE_HOURS: u32 = 24;
/// Cap on farmhands per farm; keeps farm payroll under wholesale income
/// (~72 meals/day x $7 ≈ $504 vs 6 farmhands x ~$10/h x 8h ≈ $480).
pub const FARM_MAX_WORKERS: usize = 3;
/// A full shift of missed pay makes a worker quit.
pub const UNPAID_HOURS_TO_QUIT: u32 = 8;

/// Hourly wage range (lo, hi) for jobs at this building kind.
pub fn wage_range(kind: BuildingKind) -> (f32, f32) {
    match kind {
        BuildingKind::HydroFarm => (9.0, 11.0),
        _ => (11.0, 18.0),
    }
}

/// Dynamic wholesale spot price from current city-wide supply and demand.
/// supply = meals waiting on farms; demand = unmet venue room. Bounded so a
/// glut floors the price and a famine ceilings it; the `+1` avoids a blow-up
/// at zero supply.
pub fn wholesale_price(supply: f32, demand: f32) -> f32 {
    WHOLESALE_BASE * (demand / (supply + 1.0)).clamp(PRICE_LO_MULT, PRICE_HI_MULT)
}

/// Hourly tick: farms grow food and sell it to venues at WHOLESALE_BASE.
/// Each venue buys its even share, limited by the stock cap and its balance;
/// payment splits evenly across farms (equal output). Undelivered output is
/// lost — farms holding stock arrives with trucks in Phase 3.
pub fn distribute_food(city: &mut City, hour: u32) {
    if !(6..22).contains(&hour) {
        return;
    }
    let farms: Vec<usize> = city
        .buildings
        .iter()
        .filter(|b| b.kind == BuildingKind::HydroFarm)
        .map(|b| b.id as usize)
        .collect();
    let venues: Vec<usize> = city
        .buildings
        .iter()
        .filter(|b| b.kind.is_food())
        .map(|b| b.id as usize)
        .collect();
    if venues.is_empty() || farms.is_empty() {
        return;
    }
    let share = farms.len() as f32 * FARM_OUTPUT_PER_HOUR / venues.len() as f32;
    let mut wholesale_total = 0.0;
    for id in venues {
        let b = &mut city.buildings[id];
        let take = share
            .min((STOCK_CAP - b.stock).max(0.0))
            .min(b.balance / WHOLESALE_BASE);
        if take <= 0.0 {
            continue;
        }
        b.stock += take;
        let cost = (take * WHOLESALE_BASE).min(b.balance);
        b.balance -= cost;
        wholesale_total += cost;
    }
    let per_farm = wholesale_total / farms.len() as f32;
    for id in farms {
        city.buildings[id].balance += per_farm;
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
        distribute_food(&mut city, 10);
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
        distribute_food(&mut city, 3);
        for b in city.buildings.iter().filter(|b| b.kind.is_food()) {
            assert_eq!(b.stock, 0.0);
        }
    }

    #[test]
    fn stock_caps() {
        let mut city = City::generate(&mut Rng::new(2));
        for b in city.buildings.iter_mut().filter(|b| b.kind.is_food()) {
            b.balance = 1_000_000.0;
        }
        for _ in 0..1000 {
            distribute_food(&mut city, 10);
        }
        for b in city.buildings.iter().filter(|b| b.kind.is_food()) {
            assert!(b.stock <= STOCK_CAP);
        }
    }

    #[test]
    fn retail_covers_wholesale() {
        for k in [BuildingKind::NoodleBar, BuildingKind::VendingPlaza] {
            assert!(
                meal_price(k) > WHOLESALE_BASE,
                "{k:?} sells below wholesale"
            );
        }
    }

    #[test]
    fn farm_wages_below_other_wages() {
        assert_eq!(wage_range(BuildingKind::HydroFarm), (9.0, 11.0));
        assert_eq!(wage_range(BuildingKind::DataCenter), (11.0, 18.0));
        assert_eq!(wage_range(BuildingKind::FusionPlant), (11.0, 18.0));
    }

    #[test]
    fn distribution_charges_venues_and_pays_farms() {
        let mut city = City::generate(&mut Rng::new(2));
        for b in city.buildings.iter_mut().filter(|b| b.kind.is_food()) {
            b.stock = 0.0;
        }
        let venues_before: f32 =
            city.buildings.iter().filter(|b| b.kind.is_food()).map(|b| b.balance).sum();
        let farms_before: f32 = city
            .buildings
            .iter()
            .filter(|b| b.kind == BuildingKind::HydroFarm)
            .map(|b| b.balance)
            .sum();
        distribute_food(&mut city, 10);
        let venues_after: f32 =
            city.buildings.iter().filter(|b| b.kind.is_food()).map(|b| b.balance).sum();
        let farms_after: f32 = city
            .buildings
            .iter()
            .filter(|b| b.kind == BuildingKind::HydroFarm)
            .map(|b| b.balance)
            .sum();
        assert!(venues_after < venues_before, "venues paid nothing");
        let paid = venues_before - venues_after;
        let received = farms_after - farms_before;
        assert!((paid - received).abs() < 1e-3, "leak: paid {paid}, received {received}");
        let stocked: f32 =
            city.buildings.iter().filter(|b| b.kind.is_food()).map(|b| b.stock).sum();
        assert!(
            (paid - stocked * WHOLESALE_BASE).abs() < 1e-3,
            "paid {paid} != {stocked} meals x wholesale"
        );
    }

    #[test]
    fn broke_venue_gets_no_stock() {
        let mut city = City::generate(&mut Rng::new(2));
        let id = city.buildings.iter().find(|b| b.kind.is_food()).unwrap().id as usize;
        city.buildings[id].stock = 0.0;
        city.buildings[id].balance = 0.0;
        distribute_food(&mut city, 10);
        assert_eq!(city.buildings[id].stock, 0.0);
        assert_eq!(city.buildings[id].balance, 0.0);
    }

    #[test]
    fn wholesale_price_floors_and_ceils() {
        // Glut: huge supply, no demand -> floor.
        let lo = wholesale_price(1000.0, 0.0);
        assert!((lo - WHOLESALE_BASE * PRICE_LO_MULT).abs() < 1e-4, "got {lo}");
        // Famine: no supply, big demand -> ceiling.
        let hi = wholesale_price(0.0, 1000.0);
        assert!((hi - WHOLESALE_BASE * PRICE_HI_MULT).abs() < 1e-4, "got {hi}");
    }

    #[test]
    fn wholesale_price_balanced_is_near_base() {
        // demand ≈ supply -> ≈ base.
        let p = wholesale_price(100.0, 100.0);
        // supply≈demand → ratio 100/101 ≈ 0.99, so price ≈ 0.99·base; 0.2 leaves headroom.
        assert!((p - WHOLESALE_BASE).abs() < 0.2, "got {p}");
    }

    #[test]
    fn wholesale_price_rises_with_demand() {
        assert!(wholesale_price(50.0, 80.0) > wholesale_price(50.0, 40.0));
    }
}
