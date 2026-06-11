use crate::sim::citizen::{Activity, CitizenState};
use crate::sim::rng::Rng;
use crate::sim::time;
use crate::sim::world::World;
use crate::ui::camera::Camera;
use macroquad::prelude::*;

pub fn activity_color(state: &CitizenState) -> Color {
    let act = match state {
        CitizenState::Traveling { activity, .. } => Some(*activity),
        CitizenState::Performing { activity, .. } => Some(*activity),
        CitizenState::Idle { .. } => None,
    };
    match act {
        Some(Activity::Sleep) => Color::new(0.35, 0.55, 1.0, 1.0),
        Some(Activity::Eat) => Color::new(1.0, 0.62, 0.2, 1.0),
        Some(Activity::Work) => Color::new(1.0, 0.85, 0.3, 1.0),
        Some(Activity::Fun) => Color::new(1.0, 0.3, 0.85, 1.0),
        Some(Activity::Shower) => Color::new(0.4, 0.9, 1.0, 1.0),
        Some(Activity::Stroll) | None => Color::new(0.85, 0.95, 1.0, 1.0),
    }
}

pub fn draw_citizens(world: &World, cam: &Camera, t: f32) {
    for c in &world.citizens {
        // citizens inside buildings aren't drawn
        if matches!(c.state, CitizenState::Performing { .. }) {
            continue;
        }
        let (sx, sy) = cam.to_screen(c.pos.0, c.pos.1);
        if sx < -40.0 || sy < -40.0 || sx > screen_width() + 40.0 || sy > screen_height() + 40.0 {
            continue;
        }
        let col = activity_color(&c.state);
        let bob = (t * 9.0 + c.id as f32 * 1.7).sin() * cam.ppt * 0.03;
        let r = cam.ppt * 0.16;

        // motion streak behind walkers
        if let Some(&(nx, ny)) = c.path.front() {
            let (dx, dy) = (nx as f32 + 0.5 - c.pos.0, ny as f32 + 0.5 - c.pos.1);
            let d = (dx * dx + dy * dy).sqrt().max(0.001);
            for i in 1..=3 {
                let f = i as f32 / 3.0;
                let (tx, ty) = cam.to_screen(c.pos.0 - dx / d * f * 0.45, c.pos.1 - dy / d * f * 0.45);
                draw_circle(tx, ty, r * (1.0 - f * 0.6), Color::new(col.r, col.g, col.b, 0.18 * (1.0 - f)));
            }
        }

        draw_circle(sx, sy + bob, r * 2.1, Color::new(col.r, col.g, col.b, 0.16)); // glow
        draw_circle(sx, sy + bob, r, col);
        draw_circle(sx, sy + bob - r * 0.55, r * 0.45, Color::new(1.0, 1.0, 1.0, 0.9)); // head
    }
}

// ---- ambient vehicles (visual flavor only; lives outside the sim) ----

pub struct Vehicle {
    pos: (f32, f32),
    dir: (f32, f32),
    speed: f32,
    color: Color,
}

pub struct Traffic {
    pub vehicles: Vec<Vehicle>,
}

const CAR_COLORS: [Color; 4] = [
    Color::new(0.2, 0.9, 1.0, 1.0),
    Color::new(1.0, 0.3, 0.8, 1.0),
    Color::new(0.95, 0.75, 0.3, 1.0),
    Color::new(0.6, 1.0, 0.5, 1.0),
];

impl Traffic {
    pub fn new(city_w: i32, city_h: i32, seed: u64) -> Traffic {
        let mut rng = Rng::new(seed ^ 0xCAB5);
        let mut vehicles = vec![];
        for _ in 0..16 {
            let along_x = rng.chance(0.5);
            let lane = rng.gen_range(0, 8) * crate::sim::city::BLOCK;
            let sign = if rng.chance(0.5) { 1.0 } else { -1.0 };
            let off = 0.5 + sign * 0.22; // drive on the right
            let (pos, dir) = if along_x {
                ((rng.gen_f32() * city_w as f32, lane as f32 + off), (sign, 0.0))
            } else {
                ((lane as f32 + off, rng.gen_f32() * city_h as f32), (0.0, sign))
            };
            vehicles.push(Vehicle {
                pos,
                dir,
                speed: rng.gen_f32_range(2.5, 5.0),
                color: CAR_COLORS[rng.gen_range(0, 4) as usize],
            });
        }
        Traffic { vehicles }
    }

    pub fn update(&mut self, dt: f32, city_w: i32, city_h: i32) {
        for v in &mut self.vehicles {
            v.pos.0 += v.dir.0 * v.speed * dt;
            v.pos.1 += v.dir.1 * v.speed * dt;
            if v.pos.0 < -1.0 { v.pos.0 = city_w as f32 + 1.0 }
            if v.pos.0 > city_w as f32 + 1.0 { v.pos.0 = -1.0 }
            if v.pos.1 < -1.0 { v.pos.1 = city_h as f32 + 1.0 }
            if v.pos.1 > city_h as f32 + 1.0 { v.pos.1 = -1.0 }
        }
    }

    pub fn draw(&self, cam: &Camera, tick: u64) {
        let night = time::is_night(tick);
        for v in &self.vehicles {
            let (sx, sy) = cam.to_screen(v.pos.0, v.pos.1);
            let (l, w) = (cam.ppt * 0.5, cam.ppt * 0.26);
            let horizontal = v.dir.0.abs() > 0.0;
            let (rw, rh) = if horizontal { (l, w) } else { (w, l) };
            draw_circle(sx, sy, cam.ppt * 0.4, Color::new(v.color.r, v.color.g, v.color.b, 0.10));
            draw_rectangle(sx - rw / 2.0, sy - rh / 2.0, rw, rh, v.color);
            if night {
                let (hx, hy) = (sx + v.dir.0 * l * 0.6, sy + v.dir.1 * l * 0.6);
                draw_circle(hx, hy, cam.ppt * 0.09, Color::new(1.0, 1.0, 0.9, 0.9));
            }
        }
    }
}
