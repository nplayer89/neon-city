use crate::sim::citizen::NEED_KINDS;
use crate::sim::city::{BuildingKind, City};
use crate::sim::economy::WHOLESALE_BASE;
use crate::sim::world::World;
use crate::ui::hud::{over, HudState, CYAN, PANEL, PANEL_EDGE};
use crate::ui::inspector::Selection;
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
/// Reserved width for the business detail column ("17m" / "7w"), left of the balance.
const DETAIL_COL_W: f32 = 34.0;

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

/// Display order of the BUSINESSES tab: commercial first, then industry.
/// Doubles as the membership filter — kinds not listed don't appear
/// (equivalent to is_workplace() || has_balance(); a membership test pins it).
const BUSINESS_KIND_ORDER: [BuildingKind; 7] = [
    BuildingKind::NoodleBar,
    BuildingKind::VendingPlaza,
    BuildingKind::Arcade,
    BuildingKind::HydroFarm,
    BuildingKind::FusionPlant,
    BuildingKind::RoboticsFab,
    BuildingKind::DataCenter,
];

/// Group rank of a kind in the businesses list; None = not a business.
pub fn business_rank(kind: BuildingKind) -> Option<usize> {
    BUSINESS_KIND_ORDER.iter().position(|k| *k == kind)
}

/// Building ids for the BUSINESSES tab: grouped by BUSINESS_KIND_ORDER,
/// id-ascending within a group. Buildings never spawn or despawn mid-run,
/// so this is computed once, like the citizen order.
pub fn business_order(city: &City) -> Vec<u16> {
    let mut ids: Vec<u16> = city
        .buildings
        .iter()
        .filter(|b| business_rank(b.kind).is_some())
        .map(|b| b.id)
        .collect();
    ids.sort_by_key(|&id| (business_rank(city.buildings[id as usize].kind), id));
    ids
}

/// Detail-column text: meals on hand for food venues, headcount for
/// employers, blank for arcades.
pub fn business_detail(kind: BuildingKind, stock: f32, workers: usize) -> String {
    if kind.is_food() {
        format!("{}m", stock.floor())
    } else if kind.is_workplace() {
        format!("{workers}w")
    } else {
        String::new()
    }
}

/// Red-balance rule: an employer that missed payroll, or a food venue that
/// can't afford its next wholesale meal. Industry (no books yet) only trips
/// via the insolvent flag; arcades never struggle.
pub fn business_struggling(kind: BuildingKind, balance: f32, insolvent: bool) -> bool {
    (kind.is_workplace() && insolvent) || (kind.is_food() && balance < WHOLESALE_BASE)
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tab {
    Citizens,
    Businesses,
}

pub struct Roster {
    /// Citizen ids sorted alphabetically by name; the population is fixed
    /// after world creation, so this is computed once.
    order: Vec<usize>,
    /// Building ids for the BUSINESSES tab (see business_order).
    businesses: Vec<u16>,
    tab: Tab,
    /// Scroll offsets, kept per tab so switching back doesn't lose your place.
    scroll: f32,
    business_scroll: f32,
}

impl Roster {
    pub fn new(world: &World) -> Roster {
        let mut order: Vec<usize> = (0..world.citizens.len()).collect();
        order.sort_by(|&a, &b| world.citizens[a].name.cmp(&world.citizens[b].name));
        Roster {
            order,
            businesses: business_order(&world.city),
            tab: Tab::Citizens,
            scroll: 0.0,
            business_scroll: 0.0,
        }
    }

    /// Draws the sidebar. Returns (hovered row's selection, clicked this frame).
    /// Sets `hud.pointer_over_ui` when the pointer is over the sidebar.
    pub fn draw(&mut self, world: &World, hud: &mut HudState) -> (Option<Selection>, bool) {
        let (x, y, w) = (0.0, TOP, SIDEBAR_W);
        let h = screen_height() - TOP - BOTTOM_MARGIN;
        let hovering_panel = over(x, y, w, h);
        if hovering_panel {
            hud.pointer_over_ui = true;
        }

        draw_rectangle(x, y, w, h, PANEL);
        draw_rectangle_lines(x, y, w, h, 1.5, PANEL_EDGE);
        self.draw_tabs(x, y);

        let count = match self.tab {
            Tab::Citizens => self.order.len(),
            Tab::Businesses => self.businesses.len(),
        };
        let list_top = y + HEADER_H;
        let list_h = h - HEADER_H;
        let max_scroll = (count as f32 * ROW_H - list_h).max(0.0);
        let mut scroll = match self.tab {
            Tab::Citizens => self.scroll,
            Tab::Businesses => self.business_scroll,
        };
        if hovering_panel {
            let wheel = mouse_wheel().1;
            if wheel.abs() > 0.0 {
                scroll -= wheel.signum() * ROW_H * 3.0;
            }
        }
        scroll = scroll.clamp(0.0, max_scroll);
        match self.tab {
            Tab::Citizens => self.scroll = scroll,
            Tab::Businesses => self.business_scroll = scroll,
        }

        let (_, my) = mouse_position();
        let mut hovered: Option<Selection> = None;

        // No row hover while a left-drag is in progress (map pans sweeping
        // across the sidebar), except on the press frame so clicks register.
        let hover_enabled = !is_mouse_button_down(MouseButton::Left) || is_mouse_button_pressed(MouseButton::Left);

        for i in 0..count {
            let ry = list_top + i as f32 * ROW_H - scroll;
            // Partially clipped rows are skipped (no scissor in macroquad 2D).
            if ry < list_top || ry + ROW_H > y + h + 0.5 {
                continue;
            }
            let row_hovered = hover_enabled && hovering_panel && my >= ry && my < ry + ROW_H;
            if row_hovered {
                draw_rectangle(x, ry, w, ROW_H, Color::new(0.2, 0.9, 1.0, 0.12));
            }
            let sel = match self.tab {
                Tab::Citizens => {
                    self.draw_citizen_row(world, self.order[i], x, w, ry);
                    Selection::Citizen(self.order[i])
                }
                Tab::Businesses => {
                    self.draw_business_row(world, self.businesses[i], x, w, ry);
                    Selection::Building(self.businesses[i])
                }
            };
            if row_hovered {
                hovered = Some(sel);
            }
        }

        let clicked = hovered.is_some() && is_mouse_button_pressed(MouseButton::Left);
        (hovered, clicked)
    }

    /// Header tab strip: active label cyan with an underline, inactive dim.
    /// Tab clicks land in the header band, above list_top, so they can never
    /// double as row clicks.
    fn draw_tabs(&mut self, x: f32, y: f32) {
        let labels = [(Tab::Citizens, "CITIZENS"), (Tab::Businesses, "BUSINESSES")];
        let mut lx = x + 10.0;
        for (tab, label) in labels {
            let tw = measure_text(label, None, 18, 1.0).width;
            let active = self.tab == tab;
            let color = if active { CYAN } else { Color::new(0.45, 0.6, 0.75, 0.8) };
            draw_text(label, lx, y + 19.0, 18.0, color);
            if active {
                draw_line(lx, y + 23.0, lx + tw, y + 23.0, 2.0, CYAN);
            }
            if over(lx - 4.0, y + 4.0, tw + 8.0, HEADER_H - 6.0) && is_mouse_button_pressed(MouseButton::Left) {
                self.tab = tab;
            }
            lx += tw + 16.0;
        }
    }

    fn draw_citizen_row(&self, world: &World, id: usize, x: f32, w: f32, ry: f32) {
        let icons_x = x + w - 6.0 - NEED_KINDS.len() as f32 * ICON_STRIDE;
        let name_max_w = icons_x - MONEY_COL_W - (x + 10.0) - 4.0;
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

    fn draw_business_row(&self, world: &World, id: u16, x: f32, w: f32, ry: f32) {
        let b = &world.city.buildings[id as usize];
        let balance_right = x + w - 6.0;
        let detail_right = balance_right - MONEY_COL_W;
        let name_max_w = detail_right - DETAIL_COL_W - (x + 10.0) - 4.0;

        let name = format!("{} #{}", b.kind.name(), b.id);
        draw_clipped_text(&name, x + 10.0, ry + 11.5, 15, name_max_w, Color::new(0.8, 0.9, 1.0, 0.9));

        let detail = business_detail(b.kind, b.stock, b.workers.len());
        if !detail.is_empty() {
            let dw = measure_text(&detail, None, 13, 1.0).width;
            draw_text(&detail, detail_right - dw, ry + 11.5, 13.0, Color::new(0.6, 0.75, 0.9, 0.9));
        }

        let balance_text = if b.kind.has_balance() { money_label(b.balance) } else { "-".to_string() };
        let bw = measure_text(&balance_text, None, 13, 1.0).width;
        let color = if business_struggling(b.kind, b.balance, b.insolvent) {
            band_color(Band::Low)
        } else {
            Color::new(0.8, 0.9, 1.0, 0.9)
        };
        draw_text(&balance_text, balance_right - bw, ry + 11.5, 13.0, color);
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

    #[test]
    fn business_detail_per_kind() {
        use crate::sim::city::BuildingKind;
        assert_eq!(business_detail(BuildingKind::NoodleBar, 17.9, 0), "17m");
        assert_eq!(business_detail(BuildingKind::VendingPlaza, 0.4, 9), "0m");
        assert_eq!(business_detail(BuildingKind::HydroFarm, 5.0, 3), "3w");
        assert_eq!(business_detail(BuildingKind::DataCenter, 0.0, 5), "5w");
        assert_eq!(business_detail(BuildingKind::Arcade, 0.0, 0), "");
    }

    #[test]
    fn business_struggling_rules() {
        use crate::sim::city::BuildingKind::*;
        assert!(business_struggling(HydroFarm, 500.0, true), "insolvent employer");
        assert!(!business_struggling(HydroFarm, 0.0, false), "farms are judged by payroll, not restock");
        assert!(business_struggling(NoodleBar, crate::sim::economy::WHOLESALE_BASE - 0.01, false), "venue below one meal");
        assert!(!business_struggling(NoodleBar, crate::sim::economy::WHOLESALE_BASE, false), "boundary: exactly one meal");
        assert!(!business_struggling(Arcade, 0.0, false), "arcades never struggle");
        assert!(!business_struggling(DataCenter, 0.0, false), "industry balance is meaningless");
    }

    #[test]
    fn business_order_grouped_and_complete() {
        use crate::sim::city::BuildingKind;
        let world = crate::sim::world::World::new(2161, 48);
        let order = business_order(&world.city);
        assert_eq!(order.len(), 19, "4 noodle + 3 vending + 3 arcade + 2 farm + 2 fusion + 3 fab + 2 dc");
        let ranks: Vec<usize> = order
            .iter()
            .map(|&id| business_rank(world.city.buildings[id as usize].kind).unwrap())
            .collect();
        assert!(ranks.windows(2).all(|w| w[0] <= w[1]), "not grouped: {ranks:?}");
        for pair in order.windows(2) {
            let same_group = business_rank(world.city.buildings[pair[0] as usize].kind)
                == business_rank(world.city.buildings[pair[1] as usize].kind);
            if same_group {
                assert!(pair[0] < pair[1], "ids not ascending within a group");
            }
        }
        assert!(business_rank(BuildingKind::Apartment).is_none());
        assert!(business_rank(BuildingKind::HoloPark).is_none());
    }

    #[test]
    fn roster_carries_business_list_and_defaults_to_citizens() {
        let world = crate::sim::world::World::new(2161, 48);
        let r = Roster::new(&world);
        assert_eq!(r.businesses, business_order(&world.city));
        assert_eq!(r.tab, Tab::Citizens);
    }

    #[test]
    fn business_membership_matches_kind_helpers() {
        use crate::sim::city::BuildingKind::*;
        for kind in [Apartment, NoodleBar, VendingPlaza, FusionPlant, HydroFarm, RoboticsFab, DataCenter, Arcade, HoloPark] {
            assert_eq!(
                business_rank(kind).is_some(),
                kind.is_workplace() || kind.has_balance(),
                "{kind:?} membership drifted from is_workplace/has_balance"
            );
        }
    }
}
