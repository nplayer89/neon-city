use crate::sim::citizen::NEED_KINDS;
use crate::sim::world::World;
use crate::ui::hud::{over, HudState, CYAN, PANEL, PANEL_EDGE};
use macroquad::prelude::*;

pub(crate) const SIDEBAR_W: f32 = 240.0;
/// Below the 52 px top bar.
pub(crate) const TOP: f32 = 52.0;
/// Stops above the bottom-left population strip.
pub(crate) const BOTTOM_MARGIN: f32 = 32.0;
const HEADER_H: f32 = 28.0;
const ROW_H: f32 = 15.0;
/// 4 icons, 7 px each on an 11 px stride, right-aligned 6 px from the trailing gap.
const ICON_STRIDE: f32 = 11.0;
const ICON_SIZE: f32 = 7.0;
/// Reserved width for the right-aligned wallet readout, left of the icons.
const MONEY_COL_W: f32 = 44.0;

/// Discrete status band for a need value in [0, 1].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Band {
    High,
    Medium,
    Low,
}

/// Pure (no macroquad) so it stays unit-testable.
pub fn band(value: f32) -> Band {
    if value >= 0.6 {
        Band::High
    } else if value >= 0.3 {
        Band::Medium
    } else {
        Band::Low
    }
}

/// Discrete status band for a wallet balance. $15 ≈ the priciest meal ($12)
/// plus slack; $40 ≈ several meals of cushion (spawn-range floor).
pub fn money_band(balance: f32) -> Band {
    if balance >= 40.0 {
        Band::High
    } else if balance >= 15.0 {
        Band::Medium
    } else {
        Band::Low
    }
}

/// Compact wallet label, bounded to 5 chars so it always fits MONEY_COL_W.
pub fn money_label(balance: f32) -> String {
    if balance >= 1000.0 {
        format!("${:.0}k", (balance / 1000.0).floor())
    } else {
        format!("${:.0}", balance)
    }
}

fn band_color(b: Band) -> Color {
    match b {
        Band::High => Color::new(0.3, 0.95, 0.5, 1.0),
        Band::Medium => Color::new(0.95, 0.8, 0.25, 1.0),
        Band::Low => Color::new(1.0, 0.25, 0.4, 1.0),
    }
}

/// Draws text clipped to `max_w` so long names never run under the icons.
pub(crate) fn draw_clipped_text(text: &str, x: f32, y: f32, font_px: u16, max_w: f32, color: Color) {
    if measure_text(text, None, font_px, 1.0).width <= max_w {
        draw_text(text, x, y, font_px as f32, color);
        return;
    }
    let mut s = text.to_string();
    while !s.is_empty() && measure_text(&s, None, font_px, 1.0).width > max_w {
        s.pop();
    }
    draw_text(&s, x, y, font_px as f32, color);
}

pub struct Roster {
    /// Citizen ids sorted alphabetically by name; the population is fixed
    /// after world creation, so this is computed once.
    order: Vec<usize>,
    scroll: f32,
}

impl Roster {
    pub fn new(world: &World) -> Roster {
        let mut order: Vec<usize> = (0..world.citizens.len()).collect();
        order.sort_by(|&a, &b| world.citizens[a].name.cmp(&world.citizens[b].name));
        Roster { order, scroll: 0.0 }
    }

    /// Draws the sidebar. Returns (hovered citizen id, clicked this frame).
    /// Sets `hud.pointer_over_ui` when the pointer is over the sidebar.
    pub fn draw(&mut self, world: &World, hud: &mut HudState) -> (Option<usize>, bool) {
        let (x, y, w) = (0.0, TOP, SIDEBAR_W);
        let h = screen_height() - TOP - BOTTOM_MARGIN;
        let hovering_panel = over(x, y, w, h);
        if hovering_panel {
            hud.pointer_over_ui = true;
        }

        draw_rectangle(x, y, w, h, PANEL);
        draw_rectangle_lines(x, y, w, h, 1.5, PANEL_EDGE);
        draw_text("CITIZENS", x + 10.0, y + 19.0, 18.0, CYAN);

        let list_top = y + HEADER_H;
        let list_h = h - HEADER_H;
        let max_scroll = (self.order.len() as f32 * ROW_H - list_h).max(0.0);
        if hovering_panel {
            let wheel = mouse_wheel().1;
            if wheel.abs() > 0.0 {
                self.scroll -= wheel.signum() * ROW_H * 3.0;
            }
        }
        self.scroll = self.scroll.clamp(0.0, max_scroll);

        let (_, my) = mouse_position();
        let icons_x = x + w - 6.0 - NEED_KINDS.len() as f32 * ICON_STRIDE;
        let name_max_w = icons_x - MONEY_COL_W - (x + 10.0) - 4.0;
        let mut hovered: Option<usize> = None;

        // No row hover while a left-drag is in progress (map pans sweeping
        // across the sidebar), except on the press frame so clicks register.
        let hover_enabled = !is_mouse_button_down(MouseButton::Left) || is_mouse_button_pressed(MouseButton::Left);

        for (i, &id) in self.order.iter().enumerate() {
            let ry = list_top + i as f32 * ROW_H - self.scroll;
            // Partially clipped rows are skipped (no scissor in macroquad 2D).
            if ry < list_top || ry + ROW_H > y + h + 0.5 {
                continue;
            }
            if hover_enabled && hovering_panel && my >= ry && my < ry + ROW_H {
                hovered = Some(id);
                draw_rectangle(x, ry, w, ROW_H, Color::new(0.2, 0.9, 1.0, 0.12));
            }
            let c = &world.citizens[id];
            draw_clipped_text(&c.name, x + 10.0, ry + 11.5, 15, name_max_w, Color::new(0.8, 0.9, 1.0, 0.9));
            let money_text = money_label(c.money);
            let mw = measure_text(&money_text, None, 13, 1.0).width;
            draw_text(&money_text, icons_x - 4.0 - mw, ry + 11.5, 13.0, band_color(money_band(c.money)));
            for (j, k) in NEED_KINDS.iter().enumerate() {
                let color = band_color(band(c.needs.get(*k)));
                draw_rectangle(icons_x + j as f32 * ICON_STRIDE, ry + 4.0, ICON_SIZE, ICON_SIZE, color);
            }
        }

        let clicked = hovered.is_some() && is_mouse_button_pressed(MouseButton::Left);
        (hovered, clicked)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn band_boundaries() {
        assert_eq!(band(0.0), Band::Low);
        assert_eq!(band(0.29), Band::Low);
        assert_eq!(band(0.3), Band::Medium);
        assert_eq!(band(0.59), Band::Medium);
        assert_eq!(band(0.6), Band::High);
        assert_eq!(band(1.0), Band::High);
    }

    #[test]
    fn money_band_boundaries() {
        assert_eq!(money_band(0.0), Band::Low);
        assert_eq!(money_band(14.9), Band::Low);
        assert_eq!(money_band(15.0), Band::Medium);
        assert_eq!(money_band(39.9), Band::Medium);
        assert_eq!(money_band(40.0), Band::High);
        assert_eq!(money_band(500.0), Band::High);
    }

    #[test]
    fn money_label_compacts_large_balances() {
        assert_eq!(money_label(0.0), "$0");
        assert_eq!(money_label(47.4), "$47");
        assert_eq!(money_label(999.0), "$999");
        assert_eq!(money_label(1000.0), "$1k");
        assert_eq!(money_label(12_345.0), "$12k");
        assert_eq!(money_label(999_999.0), "$999k");
    }

    #[test]
    fn roster_order_is_alphabetical_and_complete() {
        let world = crate::sim::world::World::new(2161, 48);
        let r = Roster::new(&world);
        let mut ids = r.order.clone();
        ids.sort_unstable();
        assert_eq!(
            ids,
            (0..world.citizens.len()).collect::<Vec<_>>(),
            "order is not a permutation of citizen ids"
        );
        for pair in r.order.windows(2) {
            assert!(
                world.citizens[pair[0]].name <= world.citizens[pair[1]].name,
                "roster not sorted: {} before {}",
                world.citizens[pair[0]].name,
                world.citizens[pair[1]].name
            );
        }
    }
}
