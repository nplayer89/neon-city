use crate::sim::citizen::{Activity, Citizen, CitizenState};
use crate::sim::city::{BuildingKind, City};
use crate::sim::economy;
use crate::sim::event::{push_event, EventKind, SimEvent};
use crate::sim::path;
use crate::sim::time::TICKS_PER_HOUR;
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

/// One tick of the supply chain: hourly farm production, then per-truck
/// dispatch / drive / deliver. Runs after the citizen loop each tick.
pub fn tick(
    city: &mut City,
    trucks: &mut [Truck],
    citizens: &mut [Citizen],
    tick: u64,
    hour: u32,
    events: &mut VecDeque<SimEvent>,
) {
    // 1. Farm production (06:00–22:00), accumulating into farm stock.
    if tick % TICKS_PER_HOUR == 0 && (6..22).contains(&hour) {
        for b in city.buildings.iter_mut().filter(|b| b.kind == BuildingKind::HydroFarm) {
            b.stock = (b.stock + economy::FARM_OUTPUT_PER_HOUR).min(economy::FARM_STOCK_CAP);
        }
    }

    // Venues already being served this tick (so two trucks don't both grab one).
    let mut served: Vec<u16> = trucks
        .iter()
        .filter_map(|t| match t.state {
            TruckState::Outbound { venue } => Some(venue),
            _ => None,
        })
        .collect();

    for i in 0..trucks.len() {
        match trucks[i].state {
            TruckState::Parked => {
                let farm = trucks[i].home_farm;
                if city.buildings[farm as usize].stock <= 0.0 {
                    continue;
                }
                // Driver = lowest-id farm worker currently working at the farm.
                let driver = city.buildings[farm as usize]
                    .workers
                    .iter()
                    .copied()
                    .filter(|&d| matches!(citizens[d].state, CitizenState::Performing { at, activity: Activity::Work } if at == farm))
                    .min();
                let Some(d) = driver else { continue };
                let Some(venue) = neediest_venue(city, &served) else { continue };
                let room = economy::STOCK_CAP - city.buildings[venue as usize].stock;
                let cargo = load_amount(economy::TRUCK_CAPACITY, city.buildings[farm as usize].stock, room);
                if cargo <= 0.0 {
                    continue;
                }
                let from = city.buildings[farm as usize].door;
                let to = city.buildings[venue as usize].door;
                let Some(p) = path::find_path(city, from, to) else { continue };
                city.buildings[farm as usize].stock -= cargo;
                trucks[i].cargo = cargo;
                trucks[i].driver = Some(d);
                trucks[i].pos = (from.0 as f32 + 0.5, from.1 as f32 + 0.5);
                trucks[i].path = VecDeque::from(p);
                trucks[i].state = TruckState::Outbound { venue };
                citizens[d].state = CitizenState::Driving { truck: i };
                city.buildings[farm as usize].occupants.retain(|&o| o != d);
                served.push(venue);
            }
            TruckState::Outbound { venue } => {
                advance(&mut trucks[i]);
                if let Some(d) = trucks[i].driver {
                    citizens[d].pos = trucks[i].pos;
                }
                if trucks[i].path.is_empty() {
                    // Borrow discipline: end every `&mut city.buildings[..]` borrow before
                    // the next one (supply_demand needs `&city`; the farm write is a 2nd index).
                    let open_and_room = {
                        let v = &city.buildings[venue as usize];
                        v.open() && v.stock < economy::STOCK_CAP
                    };
                    if open_and_room {
                        let (supply, demand) = supply_demand(city);
                        let price = economy::wholesale_price(supply, demand);
                        let bought;
                        {
                            let v = &mut city.buildings[venue as usize];
                            let room = economy::STOCK_CAP - v.stock;
                            bought = trucks[i].cargo.min(v.balance / price).min(room).max(0.0);
                            v.stock += bought;
                            v.balance -= bought * price;
                        }
                        trucks[i].cargo -= bought;
                        let farm = trucks[i].home_farm;
                        city.buildings[farm as usize].balance += bought * price;
                        push_event(events, SimEvent { tick, kind: EventKind::DeliveryCompleted { farm, venue, meals: bought.round() as u16 } });
                    }
                    let farm = trucks[i].home_farm;
                    let from = city.buildings[venue as usize].door;
                    let to = city.buildings[farm as usize].door;
                    trucks[i].path = path::find_path(city, from, to).map(VecDeque::from).unwrap_or_default();
                    trucks[i].state = TruckState::Returning;
                }
            }
            TruckState::Returning => {
                advance(&mut trucks[i]);
                if let Some(d) = trucks[i].driver {
                    citizens[d].pos = trucks[i].pos;
                }
                if trucks[i].path.is_empty() {
                    let farm = trucks[i].home_farm;
                    let door;
                    {
                        let fb = &mut city.buildings[farm as usize];
                        fb.stock = (fb.stock + trucks[i].cargo).min(economy::FARM_STOCK_CAP);
                        door = fb.door;
                    }
                    trucks[i].cargo = 0.0;
                    if let Some(d) = trucks[i].driver.take() {
                        citizens[d].pos = (door.0 as f32 + 0.5, door.1 as f32 + 0.5);
                        let resume = matches!(&citizens[d].job, Some(j) if j.workplace == farm && j.in_shift(hour));
                        if resume {
                            citizens[d].state = CitizenState::Performing { at: farm, activity: Activity::Work };
                            city.buildings[farm as usize].occupants.push(d);
                        } else {
                            citizens[d].state = CitizenState::Idle { until: tick + 1 };
                        }
                    }
                    trucks[i].state = TruckState::Parked;
                    trucks[i].path.clear();
                }
            }
        }
    }
}

/// Move a truck one tick along its path (same vector math as citizen travel).
fn advance(t: &mut Truck) {
    if let Some(&(tx, ty)) = t.path.front() {
        let target = (tx as f32 + 0.5, ty as f32 + 0.5);
        let (dx, dy) = (target.0 - t.pos.0, target.1 - t.pos.1);
        let d = (dx * dx + dy * dy).sqrt();
        if d <= t.speed {
            t.pos = target;
            t.path.pop_front();
        } else {
            t.pos.0 += dx / d * t.speed;
            t.pos.1 += dy / d * t.speed;
        }
    }
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
