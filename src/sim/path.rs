use crate::sim::city::City;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// A* over road tiles. Returns waypoints from `from` to `to` inclusive.
pub fn find_path(city: &City, from: (i32, i32), to: (i32, i32)) -> Option<Vec<(i32, i32)>> {
    if !city.is_road(from.0, from.1) || !city.is_road(to.0, to.1) {
        return None;
    }
    let w = city.w;
    let idx = |p: (i32, i32)| (p.1 * w + p.0) as usize;
    let n = (city.w * city.h) as usize;
    let mut g = vec![u32::MAX; n];
    let mut came: Vec<u32> = vec![u32::MAX; n];
    let heur = |p: (i32, i32)| ((p.0 - to.0).abs() + (p.1 - to.1).abs()) as u32;

    let mut open = BinaryHeap::new();
    g[idx(from)] = 0;
    open.push(Reverse((heur(from), idx(from))));

    while let Some(Reverse((_, cur))) = open.pop() {
        let cur_p = (cur as i32 % w, cur as i32 / w);
        if cur_p == to {
            let mut path = vec![to];
            let mut at = cur;
            while came[at] != u32::MAX {
                at = came[at] as usize;
                path.push((at as i32 % w, at as i32 / w));
            }
            path.reverse();
            return Some(path);
        }
        let ng = g[cur] + 1;
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let np = (cur_p.0 + dx, cur_p.1 + dy);
            if !city.is_road(np.0, np.1) {
                continue;
            }
            let ni = idx(np);
            if ng < g[ni] {
                g[ni] = ng;
                came[ni] = cur as u32;
                open.push(Reverse((ng + heur(np), ni)));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::city::City;
    use crate::sim::rng::Rng;

    #[test]
    fn straight_road_path() {
        let city = City::generate(&mut Rng::new(1));
        // row 0 is all road
        let p = find_path(&city, (0, 0), (10, 0)).expect("path");
        assert_eq!(p.first(), Some(&(0, 0)));
        assert_eq!(p.last(), Some(&(10, 0)));
        assert_eq!(p.len(), 11); // manhattan-optimal along one road
    }

    #[test]
    fn all_doors_reachable() {
        let city = City::generate(&mut Rng::new(4));
        let start = city.buildings[0].door;
        for b in &city.buildings {
            assert!(
                find_path(&city, start, b.door).is_some(),
                "{:?} unreachable",
                b.kind
            );
        }
    }

    #[test]
    fn non_road_target_fails() {
        let city = City::generate(&mut Rng::new(1));
        // find a building tile
        let b = &city.buildings[0];
        assert!(find_path(&city, (0, 0), (b.x, b.y)).is_none());
    }
}
