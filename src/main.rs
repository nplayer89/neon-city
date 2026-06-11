mod render;
mod sim;
mod ui;

use macroquad::prelude::*;

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
    loop {
        clear_background(Color::new(0.04, 0.05, 0.09, 1.0));
        draw_text("NEON CITY — booting…", 40.0, 60.0, 32.0, Color::new(0.2, 0.9, 1.0, 1.0));
        next_frame().await
    }
}
