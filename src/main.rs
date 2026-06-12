mod render;
mod sim;
mod ui;

use macroquad::prelude::*;
use sim::time::TICK_DT;
use sim::world::World;
use ui::camera::Camera;

fn window_conf() -> Conf {
    Conf {
        window_title: "NEON CITY".to_string(),
        window_width: 1360,
        window_height: 860,
        high_dpi: true,
        sample_count: 4,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let seed = 2161;
    let mut world = World::new(seed, 48);
    // Fit the city into the area the side panels leave clear, so neither the
    // roster (left) nor the inspector/event log column (right) covers it.
    let mut cam = Camera::fit(
        (sim::city::CITY_W as f32, sim::city::CITY_H as f32),
        ui::roster::SIDEBAR_W,
        ui::roster::TOP,
        screen_width() - ui::inspector::PANEL_W - ui::inspector::PANEL_MARGIN,
        screen_height() - ui::roster::BOTTOM_MARGIN,
    );
    let mut traffic = render::agents::Traffic::new(sim::city::CITY_W, sim::city::CITY_H, seed);
    let mut acc: f32 = 0.0;
    let mut hud = ui::hud::HudState::new();
    let mut inspector = ui::inspector::Inspector::new();
    let mut roster = ui::roster::Roster::new(&world);
    let mut ticker = ui::ticker::Ticker::new();

    loop {
        let t = get_time() as f32;
        if !hud.paused {
            acc += get_frame_time() * hud.speed as f32;
        }
        let mut steps = 0;
        while acc >= TICK_DT && steps < 240 {
            world.tick();
            acc -= TICK_DT;
            steps += 1;
        }
        if steps == 240 {
            acc = 0.0;
        }
        for ev in world.drain_events() {
            ticker.push(&world, &ev);
        }
        // Visual traffic is capped at 4x so vehicles don't teleport at 16x sim speed.
        let traffic_dt = if hud.paused { 0.0 } else { get_frame_time() * hud.speed.min(4) as f32 };
        traffic.update(traffic_dt, sim::city::CITY_W, sim::city::CITY_H);

        cam.update(hud.pointer_over_ui);
        inspector.handle_click(&world, &cam, &hud);
        let sel_building = match inspector.selection {
            ui::inspector::Selection::Building(b) => Some(b),
            _ => None,
        };
        render::draw_world(&world, &cam, t, sel_building, &traffic);
        ui::hud::draw_hud(&world, &mut hud);
        let (hovered, roster_clicked) = roster.draw(&world, &mut hud);
        if roster_clicked {
            if let Some(sel) = hovered {
                inspector.selection = sel;
                inspector.follow = false;
            }
        }
        if let Some(sel) = ticker.draw(&mut hud) {
            inspector.selection = sel;
            inspector.follow = false;
        }
        inspector.draw(&world, &mut cam, &mut hud, hovered);
        next_frame().await
    }
}
