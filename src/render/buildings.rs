use crate::sim::city::{Building, BuildingKind};
use crate::sim::time;
use crate::sim::world::World;
use crate::ui::camera::Camera;
use macroquad::prelude::*;

pub fn body_color(kind: BuildingKind) -> Color {
    match kind {
        BuildingKind::Apartment => Color::new(0.10, 0.14, 0.22, 1.0),
        BuildingKind::NoodleBar => Color::new(0.16, 0.09, 0.18, 1.0),
        BuildingKind::VendingPlaza => Color::new(0.10, 0.16, 0.13, 1.0),
        BuildingKind::FusionPlant => Color::new(0.17, 0.13, 0.08, 1.0),
        BuildingKind::HydroFarm => Color::new(0.08, 0.16, 0.12, 1.0),
        BuildingKind::RoboticsFab => Color::new(0.16, 0.12, 0.10, 1.0),
        BuildingKind::DataCenter => Color::new(0.09, 0.12, 0.20, 1.0),
        BuildingKind::Arcade => Color::new(0.15, 0.08, 0.20, 1.0),
        BuildingKind::HoloPark => Color::new(0.07, 0.13, 0.10, 1.0),
    }
}

pub fn trim_color(kind: BuildingKind) -> Color {
    match kind {
        BuildingKind::Apartment => Color::new(0.21, 0.88, 1.00, 1.0),
        BuildingKind::NoodleBar => Color::new(1.00, 0.24, 0.94, 1.0),
        BuildingKind::VendingPlaza => Color::new(0.62, 1.00, 0.34, 1.0),
        BuildingKind::FusionPlant => Color::new(1.00, 0.72, 0.24, 1.0),
        BuildingKind::HydroFarm => Color::new(0.34, 1.00, 0.62, 1.0),
        BuildingKind::RoboticsFab => Color::new(1.00, 0.55, 0.20, 1.0),
        BuildingKind::DataCenter => Color::new(0.30, 0.55, 1.00, 1.0),
        BuildingKind::Arcade => Color::new(1.00, 0.24, 0.82, 1.0),
        BuildingKind::HoloPark => Color::new(0.40, 1.00, 0.80, 1.0),
    }
}

fn hash(a: u32, b: u32, c: u32) -> u32 {
    let mut h = a ^ 0x9e3779b9;
    h = h.wrapping_mul(0x85ebca6b) ^ b;
    h = h.wrapping_mul(0xc2b2ae35) ^ c;
    h ^ (h >> 16)
}

pub fn draw_buildings(world: &World, cam: &Camera, t: f32, amb: f32, selected: Option<u16>) {
    let day_seed = world.day() as u32;
    for b in &world.city.buildings {
        let (sx, sy) = cam.to_screen(b.x as f32, b.y as f32);
        let (w, h) = (b.w as f32 * cam.ppt, b.h as f32 * cam.ppt);

        if b.kind == BuildingKind::HoloPark {
            draw_park(b, cam, t, amb);
            continue;
        }

        // base + inner roof slab
        let base = crate::render::lit(body_color(b.kind), amb);
        draw_rectangle(sx, sy, w, h, base);
        let inset = cam.ppt * 0.22;
        let roof = Color::new(base.r * 1.45 + 0.02, base.g * 1.45 + 0.02, base.b * 1.45 + 0.02, 1.0);
        draw_rectangle(sx + inset, sy + inset, w - inset * 2.0, h - inset * 2.0, roof);

        // roof greebles: AC units / vents, seeded
        let n = 2 + (b.vis_seed % 3) as i32;
        for i in 0..n {
            let hx = hash(b.vis_seed, i as u32, 1) % 1000;
            let hy = hash(b.vis_seed, i as u32, 2) % 1000;
            let gx = sx + inset + (hx as f32 / 1000.0) * (w - inset * 2.0 - cam.ppt * 0.3);
            let gy = sy + inset + (hy as f32 / 1000.0) * (h - inset * 2.0 - cam.ppt * 0.3);
            draw_rectangle(gx, gy, cam.ppt * 0.3, cam.ppt * 0.3, crate::render::lit(Color::new(0.05, 0.06, 0.1, 1.0), amb));
        }

        // skylight windows, lit per-night-per-building hash
        let lit_ratio = if time::is_night(world.tick) { 7 } else { 2 };
        let step = cam.ppt * 0.5;
        let (cols, rows) = (((w - inset * 2.0) / step) as i32, ((h - inset * 2.0) / step) as i32);
        for wy in 0..rows {
            for wx in 0..cols {
                if hash(b.vis_seed ^ day_seed, wx as u32, wy as u32) % 10 < lit_ratio {
                    let px = sx + inset + wx as f32 * step + step * 0.25;
                    let py = sy + inset + wy as f32 * step + step * 0.25;
                    draw_rectangle(px, py, step * 0.4, step * 0.4, Color::new(1.0, 0.85, 0.55, 0.5 + 0.5 * (1.0 - amb)));
                }
            }
        }

        if selected == Some(b.id) {
            draw_rectangle_lines(sx - 3.0, sy - 3.0, w + 6.0, h + 6.0, 3.0, WHITE);
        }
    }
}

fn draw_park(b: &Building, cam: &Camera, t: f32, amb: f32) {
    let (sx, sy) = cam.to_screen(b.x as f32, b.y as f32);
    let (w, h) = (b.w as f32 * cam.ppt, b.h as f32 * cam.ppt);
    draw_rectangle(sx, sy, w, h, crate::render::lit(body_color(BuildingKind::HoloPark), amb));
    // holo-trees
    for i in 0..5u32 {
        let hx = hash(b.vis_seed, i, 7) % 1000;
        let hy = hash(b.vis_seed, i, 8) % 1000;
        let px = sx + (0.15 + 0.7 * hx as f32 / 1000.0) * w;
        let py = sy + (0.15 + 0.7 * hy as f32 / 1000.0) * h;
        let r = cam.ppt * (0.28 + 0.06 * ((t * 1.3 + i as f32).sin()));
        draw_circle(px, py, r, Color::new(0.25, 0.95, 0.65, 0.35));
        draw_circle(px, py, r * 0.45, Color::new(0.5, 1.0, 0.8, 0.5));
    }
}

/// Neon pass — drawn after the night overlay so it pops.
pub fn draw_neon(world: &World, cam: &Camera, t: f32, amb: f32) {
    let glow = 0.55 + 0.45 * (1.0 - amb);
    for b in &world.city.buildings {
        let (sx, sy) = cam.to_screen(b.x as f32, b.y as f32);
        let (w, h) = (b.w as f32 * cam.ppt, b.h as f32 * cam.ppt);
        let mut c = if b.closed { Color::new(0.3, 0.32, 0.38, 1.0) } else { trim_color(b.kind) };
        c.a = if b.closed { 0.5 } else { glow };
        draw_rectangle_lines(sx, sy, w, h, (cam.ppt * 0.09).max(1.5), c);

        // door marker
        let (dx, dy) = b.door;
        let (dsx, dsy) = cam.to_screen(dx as f32 + 0.5, dy as f32 + 0.5);
        draw_circle(dsx, dsy, cam.ppt * 0.12, Color::new(c.r, c.g, c.b, glow * 0.8));

        // fusion plant core pulse
        if b.kind == BuildingKind::FusionPlant {
            let (cx, cy) = (sx + w / 2.0, sy + h / 2.0);
            let r = cam.ppt * (0.7 + 0.12 * (t * 2.4).sin());
            draw_circle(cx, cy, r, Color::new(1.0, 0.72, 0.24, 0.18));
            draw_circle(cx, cy, r * 0.5, Color::new(1.0, 0.85, 0.5, 0.35));
        }

        // signs when zoomed in
        if cam.ppt > 20.0 && !matches!(b.kind, BuildingKind::Apartment) {
            let label = b.kind.name().to_uppercase();
            let fs = (cam.ppt * 0.45).max(12.0);
            let dim = measure_text(&label, None, fs as u16, 1.0);
            draw_text(&label, sx + (w - dim.width) / 2.0, sy - cam.ppt * 0.15, fs, c);
        }
    }
}
