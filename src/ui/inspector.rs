use crate::render::agents::activity_color;
use crate::sim::citizen::{CitizenState, NEED_KINDS};
use crate::sim::city::Tile;
use crate::sim::world::World;
use crate::ui::camera::Camera;
use crate::ui::hud::{button, over, HudState, CYAN, PANEL, PANEL_EDGE};
use macroquad::prelude::*;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Selection {
    None,
    Citizen(usize),
    Building(u16),
}

pub struct Inspector {
    pub selection: Selection,
    pub follow: bool,
}

impl Inspector {
    pub fn new() -> Inspector {
        Inspector { selection: Selection::None, follow: false }
    }

    /// Click-to-select. Call after camera update; respects drags and UI hover.
    pub fn handle_click(&mut self, world: &World, cam: &Camera, hud: &HudState) {
        if hud.pointer_over_ui || cam.dragged || !is_mouse_button_released(MouseButton::Left) {
            return;
        }
        let (mx, my) = mouse_position();
        let (wx, wy) = cam.to_world(mx, my);

        // nearest visible citizen within 0.7 tiles
        let mut best: Option<(f32, usize)> = None;
        for c in &world.citizens {
            if matches!(c.state, CitizenState::Performing { .. }) {
                continue;
            }
            let d2 = (c.pos.0 - wx).powi(2) + (c.pos.1 - wy).powi(2);
            if d2 < 0.49 && best.map_or(true, |(bd, _)| d2 < bd) {
                best = Some((d2, c.id));
            }
        }
        if let Some((_, id)) = best {
            self.selection = Selection::Citizen(id);
            self.follow = false;
            return;
        }
        if let Tile::Building(id) = world.city.tile(wx as i32, wy as i32) {
            self.selection = Selection::Building(id);
            self.follow = false;
            return;
        }
        self.selection = Selection::None;
        self.follow = false;
    }

    /// `preview`: a citizen to show instead of the selection (roster hover).
    /// Previewing never alters the selection or follow state.
    pub fn draw(&mut self, world: &World, cam: &mut Camera, hud: &mut HudState, preview: Option<usize>) {
        // Follow-centering stays tied to the selection even while previewing,
        // so hovering a roster row never yanks a followed camera.
        if self.follow {
            if let Selection::Citizen(id) = self.selection {
                cam.center = world.citizens[id].pos;
            }
        }
        if let Some(id) = preview {
            self.draw_citizen_panel(world, cam, hud, id, true);
            return;
        }
        match self.selection {
            Selection::None => {}
            Selection::Citizen(id) => self.draw_citizen_panel(world, cam, hud, id, false),
            Selection::Building(id) => self.draw_building_panel(world, hud, id),
        }
    }

    fn panel_rect(&self) -> (f32, f32, f32, f32) {
        let w = 300.0;
        (screen_width() - w - 14.0, 66.0, w, 330.0)
    }

    fn draw_citizen_panel(&mut self, world: &World, cam: &mut Camera, hud: &mut HudState, id: usize, preview: bool) {
        let (x, y, w, h) = self.panel_rect();
        if over(x, y, w, h) {
            hud.pointer_over_ui = true;
        }
        draw_rectangle(x, y, w, h, PANEL);
        draw_rectangle_lines(x, y, w, h, 1.5, PANEL_EDGE);

        // Borrow citizen data we need, keeping borrows short
        let (name, archetype, money) = {
            let c = &world.citizens[id];
            (c.name.clone(), c.personality.archetype, c.money)
        };
        draw_text(&name, x + 14.0, y + 30.0, 26.0, WHITE);
        draw_text(archetype, x + 14.0, y + 52.0, 18.0, Color::new(1.0, 0.3, 0.85, 0.9));

        // need bars
        let mut by = y + 78.0;
        for k in NEED_KINDS {
            let v = world.citizens[id].needs.get(k);
            draw_text(k.label(), x + 14.0, by + 12.0, 15.0, Color::new(0.6, 0.75, 0.9, 0.9));
            let (bx, bw, bh) = (x + 90.0, w - 110.0, 12.0);
            draw_rectangle(bx, by, bw, bh, Color::new(0.08, 0.1, 0.16, 1.0));
            let fill = Color::new(1.0 - v * 0.8, 0.2 + v * 0.7, 0.35, 1.0);
            draw_rectangle(bx, by, bw * v, bh, fill);
            draw_rectangle_lines(bx, by, bw, bh, 1.0, PANEL_EDGE);
            by += 26.0;
        }

        // job + state
        by += 8.0;
        draw_text("WALLET", x + 14.0, by + 12.0, 15.0, Color::new(0.6, 0.75, 0.9, 0.9));
        draw_text(&format!("₢ {:.0}", money), x + 90.0, by + 14.0, 18.0, Color::new(0.95, 0.85, 0.3, 1.0));
        by += 26.0;
        let job = match &world.citizens[id].job {
            Some(j) => format!(
                "{}  {:02}:00–{:02}:00  ₢{:.0}/h",
                world.city.buildings[j.workplace as usize].kind.name(),
                j.shift_start, j.shift_end, j.wage_per_hour
            ),
            None => "Unemployed".to_string(),
        };
        draw_text("JOB", x + 14.0, by + 12.0, 15.0, Color::new(0.6, 0.75, 0.9, 0.9));
        draw_text(&job, x + 90.0, by + 12.0, 15.0, WHITE);
        by += 26.0;

        let (state_str, state_color) = {
            let c = &world.citizens[id];
            let s = match &c.state {
                CitizenState::Idle { .. } => "Idle — deciding".to_string(),
                CitizenState::Traveling { to, activity } => match to {
                    Some(b) => format!("→ {} ({})", world.city.buildings[*b as usize].kind.name(), activity.label()),
                    None => "Strolling".to_string(),
                },
                CitizenState::Performing { at, activity } => {
                    format!("{} @ {}", activity.label(), world.city.buildings[*at as usize].kind.name())
                }
            };
            (s, activity_color(&c.state))
        };
        draw_text("NOW", x + 14.0, by + 12.0, 15.0, Color::new(0.6, 0.75, 0.9, 0.9));
        draw_text(&state_str, x + 90.0, by + 12.0, 15.0, state_color);
        by += 34.0;

        if !preview {
            let mut ui_hit = hud.pointer_over_ui;
            if button(x + 14.0, by, 110.0, 30.0, if self.follow { "FOLLOWING" } else { "FOLLOW" }, self.follow, &mut ui_hit) {
                self.follow = !self.follow;
            }
            hud.pointer_over_ui = ui_hit;
        }

        // marker ring in-world
        let pos = world.citizens[id].pos;
        let (sx, sy) = cam.to_screen(pos.0, pos.1);
        draw_circle_lines(sx, sy, cam.ppt * 0.34, 2.0, CYAN);
    }

    fn draw_building_panel(&mut self, world: &World, hud: &mut HudState, id: u16) {
        let b = &world.city.buildings[id as usize];
        let (x, y, w, h) = self.panel_rect();
        if over(x, y, w, h) {
            hud.pointer_over_ui = true;
        }
        draw_rectangle(x, y, w, h, PANEL);
        draw_rectangle_lines(x, y, w, h, 1.5, PANEL_EDGE);

        draw_text(b.kind.name(), x + 14.0, y + 30.0, 26.0, crate::render::buildings::trim_color(b.kind));
        draw_text(&format!("#{:03}", b.id), x + w - 60.0, y + 30.0, 18.0, Color::new(0.6, 0.75, 0.9, 0.8));

        let mut by = y + 64.0;

        // Use a local helper to avoid borrow checker issues with closures capturing x
        fn draw_line_item(label: &str, value: &str, x: f32, by: f32) {
            draw_text(label, x + 14.0, by, 15.0, Color::new(0.6, 0.75, 0.9, 0.9));
            draw_text(value, x + 110.0, by, 15.0, WHITE);
        }

        if b.kind.is_food() {
            draw_line_item("STOCK", &format!("{:.0} meals", b.stock), x, by);
            by += 24.0;
            draw_line_item("PRICE", &format!("₢ {:.0}", crate::sim::economy::meal_price(b.kind)), x, by);
            by += 24.0;
        }
        if b.kind.is_workplace() {
            draw_line_item("WORKERS", &format!("{}", b.workers.len()), x, by);
            by += 24.0;
        }
        draw_line_item("INSIDE", &format!("{}", b.occupants.len()), x, by);
        by += 24.0;

        by += 6.0;
        for &cid in b.occupants.iter().take(8) {
            draw_text(&format!("· {}", world.citizens[cid].name), x + 14.0, by, 15.0, Color::new(0.8, 0.9, 1.0, 0.85));
            by += 20.0;
        }
    }
}
