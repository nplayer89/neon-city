pub mod agents;
pub mod buildings;

use crate::sim::time;
use crate::sim::world::World;
use crate::ui::camera::Camera;
use macroquad::prelude::*;

pub const ROAD: Color = Color::new(0.075, 0.085, 0.12, 1.0);
pub const PAVEMENT: Color = Color::new(0.11, 0.125, 0.17, 1.0);
pub const LANE: Color = Color::new(0.22, 0.26, 0.36, 1.0);

/// Tint-multiply a color by daylight ambient.
pub fn lit(c: Color, amb: f32) -> Color {
    let a = 0.45 + 0.55 * amb;
    Color::new(c.r * a, c.g * a, c.b * a, c.a)
}

pub fn draw_world(world: &World, cam: &Camera, t: f32, selected_building: Option<u16>, traffic: &agents::Traffic) {
    let amb = time::daylight(world.tick);
    clear_background(Color::new(0.016, 0.02, 0.045, 1.0));

    // visible tile bounds
    let (wx0, wy0) = cam.to_world(0.0, 0.0);
    let (wx1, wy1) = cam.to_world(screen_width(), screen_height());
    let x0 = (wx0.floor() as i32 - 1).max(0);
    let y0 = (wy0.floor() as i32 - 1).max(0);
    let x1 = (wx1.ceil() as i32 + 1).min(world.city.w);
    let y1 = (wy1.ceil() as i32 + 1).min(world.city.h);

    // ground
    for y in y0..y1 {
        for x in x0..x1 {
            let (sx, sy) = cam.to_screen(x as f32, y as f32);
            let c = match world.city.tile(x, y) {
                crate::sim::city::Tile::Road => ROAD,
                _ => PAVEMENT,
            };
            draw_rectangle(sx, sy, cam.ppt + 1.0, cam.ppt + 1.0, lit(c, amb));
        }
    }
    // lane markings on the road grid lines
    for y in y0..y1 {
        for x in x0..x1 {
            if !world.city.is_road(x, y) {
                continue;
            }
            let center_row = y % crate::sim::city::BLOCK == 0 && x % 2 == 0;
            let center_col = x % crate::sim::city::BLOCK == 0 && y % 2 == 0;
            if center_row || center_col {
                let (sx, sy) = cam.to_screen(x as f32 + 0.42, y as f32 + 0.42);
                draw_rectangle(sx, sy, cam.ppt * 0.16, cam.ppt * 0.16, lit(LANE, amb));
            }
        }
    }

    buildings::draw_buildings(world, cam, t, amb, selected_building);

    // night overlay — neon layers draw after this and appear to glow
    let dark = (1.0 - amb) * 0.45;
    draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.01, 0.015, 0.06, dark));
    // dusk/dawn warm wash
    let h = world.hour_f();
    let dusk = (1.0 - ((h - 6.5).abs().min((h - 19.5).abs()) / 1.5)).max(0.0);
    if dusk > 0.0 {
        draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.9, 0.35, 0.1, dusk * 0.08));
    }

    buildings::draw_neon(world, cam, t, amb);
    traffic.draw(cam, world.tick);
    agents::draw_citizens(world, cam, t);
}
