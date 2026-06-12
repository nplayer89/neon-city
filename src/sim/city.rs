use crate::sim::rng::Rng;

pub const CITY_W: i32 = 49;
pub const CITY_H: i32 = 49;
pub const BLOCK: i32 = 6; // road every 6th row/col

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tile {
    Road,
    Pavement,
    Building(u16),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BuildingKind {
    Apartment,
    NoodleBar,
    VendingPlaza,
    FusionPlant,
    HydroFarm,
    RoboticsFab,
    DataCenter,
    Arcade,
    HoloPark,
}

impl BuildingKind {
    pub fn name(&self) -> &'static str {
        match self {
            BuildingKind::Apartment => "Hab Block",
            BuildingKind::NoodleBar => "Noodle Bar",
            BuildingKind::VendingPlaza => "Vending Plaza",
            BuildingKind::FusionPlant => "Fusion Plant",
            BuildingKind::HydroFarm => "Hydro Farm",
            BuildingKind::RoboticsFab => "Robotics Fab",
            BuildingKind::DataCenter => "Data Center",
            BuildingKind::Arcade => "Holo Arcade",
            BuildingKind::HoloPark => "Holo Park",
        }
    }

    pub fn is_workplace(&self) -> bool {
        matches!(
            self,
            BuildingKind::FusionPlant
                | BuildingKind::HydroFarm
                | BuildingKind::RoboticsFab
                | BuildingKind::DataCenter
        )
    }

    pub fn is_food(&self) -> bool {
        matches!(self, BuildingKind::NoodleBar | BuildingKind::VendingPlaza)
    }

    pub fn is_leisure(&self) -> bool {
        matches!(self, BuildingKind::Arcade | BuildingKind::HoloPark)
    }

    /// Participates in the Phase 2 money loop (holds a balance the UI shows).
    pub fn has_balance(&self) -> bool {
        self.is_food() || matches!(self, BuildingKind::Arcade | BuildingKind::HydroFarm)
    }

    /// Employers that pay wages from their own balance. Everyone else's
    /// wages are minted (industry revenue is deferred — see Phase 2 spec).
    pub fn wages_from_balance(&self) -> bool {
        matches!(self, BuildingKind::HydroFarm)
    }
}

pub struct Building {
    pub id: u16,
    pub kind: BuildingKind,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    /// Road tile adjacent to the building where citizens enter.
    pub door: (i32, i32),
    /// Food venues only: meals in stock.
    pub stock: f32,
    /// Money the business holds (Phase 2). Stays 0 for kinds outside the loop.
    pub balance: f32,
    /// Latch so EmployerInsolvent events edge-trigger.
    pub insolvent: bool,
    /// Citizen ids currently inside.
    pub occupants: Vec<usize>,
    /// Citizen ids employed here.
    pub workers: Vec<usize>,
    /// Per-building visual variation seed.
    pub vis_seed: u32,
}

pub struct City {
    pub w: i32,
    pub h: i32,
    pub tiles: Vec<Tile>,
    pub buildings: Vec<Building>,
}

impl City {
    pub fn tile(&self, x: i32, y: i32) -> Tile {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return Tile::Pavement;
        }
        self.tiles[(y * self.w + x) as usize]
    }

    pub fn is_road(&self, x: i32, y: i32) -> bool {
        self.tile(x, y) == Tile::Road
    }

    pub fn buildings_of(&self, pred: impl Fn(BuildingKind) -> bool) -> impl Iterator<Item = &Building> {
        self.buildings.iter().filter(move |b| pred(b.kind))
    }

    /// A random road tile within `radius` (chebyshev) of `from`.
    pub fn random_road_near(&self, rng: &mut Rng, from: (i32, i32), radius: i32) -> (i32, i32) {
        for _ in 0..32 {
            let x = rng.gen_range(from.0 - radius, from.0 + radius + 1).clamp(0, self.w - 1);
            let y = rng.gen_range(from.1 - radius, from.1 + radius + 1).clamp(0, self.h - 1);
            if self.is_road(x, y) {
                return (x, y);
            }
        }
        from
    }

    pub fn generate(rng: &mut Rng) -> City {
        let (w, h) = (CITY_W, CITY_H);
        let mut tiles = vec![Tile::Pavement; (w * h) as usize];
        for y in 0..h {
            for x in 0..w {
                if x % BLOCK == 0 || y % BLOCK == 0 {
                    tiles[(y * w + x) as usize] = Tile::Road;
                }
            }
        }

        // 8x8 blocks of 5x5 interior tiles, classified into rings from center.
        let mut rings: [Vec<(i32, i32)>; 3] = [vec![], vec![], vec![]];
        for by in 0..8 {
            for bx in 0..8 {
                let d = (bx as f32 - 3.5).abs().max((by as f32 - 3.5).abs());
                let ring = if d <= 1.5 { 0 } else if d <= 2.5 { 1 } else { 2 };
                rings[ring].push((bx, by));
            }
        }
        for r in rings.iter_mut() {
            rng.shuffle(r);
        }

        use BuildingKind::*;
        // (kind, count, preferred ring): commercial center, residential mid, industrial outer.
        let wishlist: [(BuildingKind, usize, usize); 9] = [
            (NoodleBar, 4, 0),
            (VendingPlaza, 3, 0),
            (Arcade, 3, 0),
            (DataCenter, 2, 0),
            (Apartment, 12, 1),
            (HoloPark, 4, 1),
            (FusionPlant, 2, 2),
            (HydroFarm, 2, 2),
            (RoboticsFab, 3, 2),
        ];

        let mut city = City { w, h, tiles, buildings: vec![] };
        for (kind, count, pref) in wishlist {
            for _ in 0..count {
                // take a block from preferred ring, falling back to any non-empty
                let order = [pref, (pref + 1) % 3, (pref + 2) % 3];
                let block = order.iter().find_map(|&r| rings[r].pop());
                let Some((bx, by)) = block else { break };
                city.place_building(rng, kind, bx, by);
            }
        }
        city
    }

    fn place_building(&mut self, rng: &mut Rng, kind: BuildingKind, bx: i32, by: i32) {
        let (ox, oy) = (bx * BLOCK + 1, by * BLOCK + 1);
        let full = matches!(kind, BuildingKind::FusionPlant | BuildingKind::HoloPark);
        let bw = if full { 5 } else { rng.gen_range(3, 6) };
        let bh = if full { 5 } else { rng.gen_range(3, 6) };
        // snap to a corner of the block so at least two sides face roads
        let x = ox + if rng.chance(0.5) { 0 } else { 5 - bw };
        let y = oy + if rng.chance(0.5) { 0 } else { 5 - bh };

        let id = self.buildings.len() as u16;
        for ty in y..y + bh {
            for tx in x..x + bw {
                self.tiles[(ty * self.w + tx) as usize] = Tile::Building(id);
            }
        }

        // collect road tiles adjacent to the rect perimeter, pick one as the door
        let mut candidates = vec![];
        for tx in x..x + bw {
            for &dy in &[y - 1, y + bh] {
                if self.is_road(tx, dy) {
                    candidates.push((tx, dy));
                }
            }
        }
        for ty in y..y + bh {
            for &dx in &[x - 1, x + bw] {
                if self.is_road(dx, ty) {
                    candidates.push((dx, ty));
                }
            }
        }
        let door = candidates[rng.gen_range(0, candidates.len() as i32) as usize];

        let stock = if kind.is_food() { 20.0 } else { 0.0 };
        // Day-one float: venues can buy the first deliveries, farms can cover
        // roughly half a day of payroll before wholesale revenue arrives.
        let balance = if kind.is_food() {
            100.0
        } else if kind == BuildingKind::HydroFarm {
            300.0
        } else {
            0.0
        };
        self.buildings.push(Building {
            id,
            kind,
            x,
            y,
            w: bw,
            h: bh,
            door,
            stock,
            balance,
            insolvent: false,
            occupants: vec![],
            workers: vec![],
            vis_seed: rng.next_u32(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::rng::Rng;

    #[test]
    fn generates_required_buildings() {
        for seed in 1..=5u64 {
            let city = City::generate(&mut Rng::new(seed));
            let count = |k| city.buildings.iter().filter(|b| b.kind == k).count();
            assert!(count(BuildingKind::Apartment) >= 10, "seed {seed}");
            assert!(count(BuildingKind::NoodleBar) >= 2);
            assert!(count(BuildingKind::VendingPlaza) >= 2);
            assert!(count(BuildingKind::FusionPlant) >= 1);
            assert!(count(BuildingKind::HydroFarm) >= 1);
            assert!(count(BuildingKind::RoboticsFab) >= 1);
            assert!(count(BuildingKind::DataCenter) >= 1);
            assert!(count(BuildingKind::Arcade) >= 1);
            assert!(count(BuildingKind::HoloPark) >= 1);
        }
    }

    #[test]
    fn doors_are_roads_adjacent_to_building() {
        let city = City::generate(&mut Rng::new(3));
        for b in &city.buildings {
            let (dx, dy) = b.door;
            assert!(city.is_road(dx, dy), "door of {:?} not on road", b.kind);
            // door must touch the building rect
            let touches = dx >= b.x - 1
                && dx <= b.x + b.w
                && dy >= b.y - 1
                && dy <= b.y + b.h;
            assert!(touches, "door of {:?} not adjacent", b.kind);
        }
    }

    #[test]
    fn building_tiles_match_rects() {
        let city = City::generate(&mut Rng::new(9));
        for y in 0..city.h {
            for x in 0..city.w {
                if let Tile::Building(id) = city.tile(x, y) {
                    let b = &city.buildings[id as usize];
                    assert!(x >= b.x && x < b.x + b.w && y >= b.y && y < b.y + b.h);
                }
            }
        }
    }

    #[test]
    fn balances_seeded_by_kind() {
        let city = City::generate(&mut Rng::new(4));
        for b in &city.buildings {
            let expected = if b.kind.is_food() {
                100.0
            } else if b.kind == BuildingKind::HydroFarm {
                300.0
            } else {
                0.0
            };
            assert_eq!(b.balance, expected, "{:?}", b.kind);
            assert!(!b.insolvent, "{:?} starts insolvent", b.kind);
        }
    }

    #[test]
    fn money_loop_participation_by_kind() {
        assert!(BuildingKind::NoodleBar.has_balance());
        assert!(BuildingKind::VendingPlaza.has_balance());
        assert!(BuildingKind::Arcade.has_balance());
        assert!(BuildingKind::HydroFarm.has_balance());
        assert!(!BuildingKind::Apartment.has_balance());
        assert!(!BuildingKind::FusionPlant.has_balance());
        assert!(BuildingKind::HydroFarm.wages_from_balance());
        assert!(!BuildingKind::DataCenter.wages_from_balance());
    }
}
