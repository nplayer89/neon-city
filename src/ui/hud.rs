use crate::sim::world::World;
use macroquad::prelude::*;

pub const CYAN: Color = Color::new(0.2, 0.9, 1.0, 1.0);
pub const PANEL: Color = Color::new(0.03, 0.05, 0.1, 0.88);
pub const PANEL_EDGE: Color = Color::new(0.2, 0.9, 1.0, 0.35);

pub struct HudState {
    pub speed: u32,
    pub paused: bool,
    /// True when the pointer is over any UI element this frame.
    pub pointer_over_ui: bool,
}

impl HudState {
    pub fn new() -> HudState {
        HudState { speed: 1, paused: false, pointer_over_ui: false }
    }
}

fn over(x: f32, y: f32, w: f32, h: f32) -> bool {
    let (mx, my) = mouse_position();
    mx >= x && mx <= x + w && my >= y && my <= y + h
}

/// Immediate-mode button. Returns true on click.
pub fn button(x: f32, y: f32, w: f32, h: f32, label: &str, active: bool, ui_hit: &mut bool) -> bool {
    let hover = over(x, y, w, h);
    if hover {
        *ui_hit = true;
    }
    let bg = if active {
        Color::new(0.16, 0.5, 0.6, 0.95)
    } else if hover {
        Color::new(0.1, 0.2, 0.32, 0.95)
    } else {
        Color::new(0.05, 0.09, 0.16, 0.9)
    };
    draw_rectangle(x, y, w, h, bg);
    draw_rectangle_lines(x, y, w, h, 1.5, if active { CYAN } else { PANEL_EDGE });
    let dim = measure_text(label, None, 18, 1.0);
    draw_text(label, x + (w - dim.width) / 2.0, y + h / 2.0 + 6.0, 18.0, if active { WHITE } else { CYAN });
    hover && is_mouse_button_pressed(MouseButton::Left)
}

/// Draws the HUD; updates speed/pause from clicks and keys.
pub fn draw_hud(world: &World, hud: &mut HudState) {
    hud.pointer_over_ui = false;

    // top bar
    let bar_h = 52.0;
    draw_rectangle(0.0, 0.0, screen_width(), bar_h, PANEL);
    draw_line(0.0, bar_h, screen_width(), bar_h, 1.5, PANEL_EDGE);
    if over(0.0, 0.0, screen_width(), bar_h) {
        hud.pointer_over_ui = true;
    }

    draw_text("NEON CITY", 18.0, 33.0, 30.0, CYAN);
    draw_text("// 2161", 168.0, 33.0, 20.0, Color::new(1.0, 0.3, 0.85, 0.9));

    let mins = (world.tick % crate::sim::time::TICKS_PER_HOUR) * 60 / crate::sim::time::TICKS_PER_HOUR;
    let clock = format!("DAY {}  {:02}:{:02}", world.day(), world.hour(), mins);
    draw_text(&clock, 290.0, 33.0, 24.0, WHITE);

    // speed buttons
    let bx = 470.0;
    let mut ui_hit = hud.pointer_over_ui;
    if button(bx, 10.0, 48.0, 32.0, "||", hud.paused, &mut ui_hit) {
        hud.paused = !hud.paused;
    }
    for (i, (label, s)) in [("1x", 1u32), ("4x", 4), ("16x", 16)].iter().enumerate() {
        if button(bx + 56.0 + i as f32 * 56.0, 10.0, 48.0, 32.0, label, !hud.paused && hud.speed == *s, &mut ui_hit) {
            hud.speed = *s;
            hud.paused = false;
        }
    }
    hud.pointer_over_ui = ui_hit;

    // keyboard shortcuts
    if is_key_pressed(KeyCode::Space) {
        hud.paused = !hud.paused;
    }
    if is_key_pressed(KeyCode::Key1) { hud.speed = 1; hud.paused = false; }
    if is_key_pressed(KeyCode::Key2) { hud.speed = 4; hud.paused = false; }
    if is_key_pressed(KeyCode::Key3) { hud.speed = 16; hud.paused = false; }

    // population strip, bottom-left
    let employed = world.citizens.iter().filter(|c| c.job.is_some()).count();
    let info = format!("POP {}   EMPLOYED {}   SEED {}", world.citizens.len(), employed, world.seed);
    draw_text(&info, 18.0, screen_height() - 14.0, 18.0, Color::new(0.6, 0.75, 0.9, 0.8));
}
