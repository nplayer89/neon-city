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
    let mut cam = Camera::new((sim::city::CITY_W as f32 / 2.0, sim::city::CITY_H as f32 / 2.0), 16.0);
    let mut acc: f32 = 0.0;
    let speed: u32 = 1;

    loop {
        let t = get_time() as f32;
        acc += get_frame_time() * speed as f32;
        let mut steps = 0;
        while acc >= TICK_DT && steps < 240 {
            world.tick();
            acc -= TICK_DT;
            steps += 1;
        }
        if steps == 240 {
            acc = 0.0;
        }

        cam.update(false);
        render::draw_world(&world, &cam, t, None);
        next_frame().await
    }
}
