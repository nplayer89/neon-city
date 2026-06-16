use crate::sim::citizen::NeedKind;
use crate::sim::event::{EventKind, SimEvent};
use crate::sim::time::{self, TICKS_PER_HOUR};
use crate::sim::world::World;
use crate::ui::hud::{over, HudState, CYAN, PANEL, PANEL_EDGE};
use crate::ui::inspector::{Selection, PANEL_H, PANEL_MARGIN, PANEL_TOP, PANEL_W};
use crate::ui::roster::draw_clipped_text;
use macroquad::prelude::*;
use std::collections::VecDeque;

/// Stored entries; the visible count is whatever fits the panel height.
const MAX_ENTRIES: usize = 40;
const LINE_H: f32 = 17.0;
const ENTRY_PAD: f32 = 6.0;
const FONT_PX: u16 = 14;
const HEADER_H: f32 = 28.0;
/// Sits below the inspector's reserved area so selecting never covers the log.
const TOP: f32 = PANEL_TOP + PANEL_H + 10.0;
const BOTTOM_MARGIN: f32 = 32.0;

struct Entry {
    /// Pre-wrapped at push time; the panel width is fixed.
    lines: Vec<String>,
    target: Selection,
    color: Color,
}

pub struct Ticker {
    entries: VecDeque<Entry>,
}

/// Ticker line + click-to-select target for a sim event.
pub fn format_event(world: &World, ev: &SimEvent) -> (String, Selection) {
    let mins = (ev.tick % TICKS_PER_HOUR) * 60 / TICKS_PER_HOUR;
    let stamp = format!("D{} {:02}:{:02}", time::day(ev.tick), time::hour(ev.tick), mins);
    // Phase 1: citizens and buildings are never removed, so direct indexing is
    // safe. Phase 4 (citizen mortality) must drain stale events before despawn
    // or these lookups need bounds checks.
    match ev.kind {
        EventKind::VenueSoldOut { building } => (
            format!(
                "[{stamp}] {} #{:03} sold out of meals",
                world.city.buildings[building as usize].kind.name(),
                building
            ),
            Selection::Building(building),
        ),
        EventKind::CriticalNeed { citizen, need } => {
            let verb = match need {
                NeedKind::Hunger => "is starving",
                NeedKind::Energy => "is exhausted",
                NeedKind::Hygiene => "desperately needs a shower",
                NeedKind::Fun => "is bored stiff",
            };
            (
                format!("[{stamp}] {} {verb}", world.citizens[citizen].name),
                Selection::Citizen(citizen),
            )
        }
        EventKind::CantAffordMeal { citizen, building } => (
            format!(
                "[{stamp}] {} can't afford a meal at {}",
                world.citizens[citizen].name,
                world.city.buildings[building as usize].kind.name()
            ),
            Selection::Citizen(citizen),
        ),
        EventKind::DailyWages { day, total } => (
            format!("[{stamp}] Day {day} wrap-up: ${total:.0} paid in wages"),
            Selection::None,
        ),
        EventKind::EmployerInsolvent { building } => (
            format!(
                "[{stamp}] {} #{:03} can't make payroll",
                world.city.buildings[building as usize].kind.name(),
                building
            ),
            Selection::Building(building),
        ),
        EventKind::WorkerQuit { citizen, building } => (
            format!(
                "[{stamp}] {} quit {} over unpaid wages",
                world.citizens[citizen].name,
                world.city.buildings[building as usize].kind.name()
            ),
            Selection::Citizen(citizen),
        ),
        EventKind::DeliveryCompleted { farm, venue, meals } => (
            format!(
                "[{stamp}] {} #{:03} delivered {meals} meals to {} #{:03}",
                world.city.buildings[farm as usize].kind.name(),
                farm,
                world.city.buildings[venue as usize].kind.name(),
                venue
            ),
            Selection::Building(venue),
        ),
    }
}

fn event_color(kind: EventKind) -> Color {
    match kind {
        EventKind::VenueSoldOut { .. } => Color::new(1.0, 0.75, 0.25, 1.0),
        EventKind::CriticalNeed { .. } => Color::new(1.0, 0.35, 0.35, 1.0),
        EventKind::CantAffordMeal { .. } => Color::new(1.0, 0.3, 0.85, 1.0),
        EventKind::DailyWages { .. } => CYAN,
        EventKind::EmployerInsolvent { .. } => Color::new(1.0, 0.55, 0.15, 1.0),
        EventKind::WorkerQuit { .. } => Color::new(1.0, 0.55, 0.45, 1.0),
        EventKind::DeliveryCompleted { .. } => Color::new(0.4, 0.9, 1.0, 1.0),
    }
}

/// Greedy word-wrap into at most `max_lines` lines of `max_w` px. Words past
/// the last line are appended to it; draw_clipped_text trims the overflow.
fn wrap_text(text: &str, font_px: u16, max_w: f32, max_lines: usize) -> Vec<String> {
    let mut lines = vec![String::new()];
    for word in text.split_whitespace() {
        let at_cap = lines.len() == max_lines;
        let last = lines.last_mut().unwrap();
        let cand = if last.is_empty() { word.to_string() } else { format!("{last} {word}") };
        if last.is_empty() || at_cap || measure_text(&cand, None, font_px, 1.0).width <= max_w {
            *last = cand;
        } else {
            lines.push(word.to_string());
        }
    }
    lines
}

impl Ticker {
    pub fn new() -> Ticker {
        Ticker { entries: VecDeque::new() }
    }

    pub fn push(&mut self, world: &World, ev: &SimEvent) {
        let (text, target) = format_event(world, ev);
        if self.entries.len() == MAX_ENTRIES {
            self.entries.pop_front();
        }
        let lines = wrap_text(&text, FONT_PX, PANEL_W - 20.0, 2);
        self.entries.push_back(Entry { lines, target, color: event_color(ev.kind) });
    }

    /// Right-side event log under the inspector area, newest entry on top.
    /// Returns a clicked entry's target.
    pub fn draw(&self, hud: &mut HudState) -> Option<Selection> {
        let x = screen_width() - PANEL_W - PANEL_MARGIN;
        let (y, w) = (TOP, PANEL_W);
        let h = screen_height() - TOP - BOTTOM_MARGIN;
        if h < HEADER_H + LINE_H {
            return None; // window too short for the log
        }
        if over(x, y, w, h) {
            hud.pointer_over_ui = true;
        }
        draw_rectangle(x, y, w, h, PANEL);
        draw_rectangle_lines(x, y, w, h, 1.5, PANEL_EDGE);
        draw_text("EVENT LOG", x + 10.0, y + 19.0, 18.0, CYAN);

        let (mx, my) = mouse_position();
        let bottom = y + h;
        let mut ey = y + HEADER_H;
        let mut clicked = None;
        for e in self.entries.iter().rev() {
            let eh = e.lines.len() as f32 * LINE_H + ENTRY_PAD;
            if ey + eh > bottom {
                break;
            }
            let hit = mx >= x && mx <= x + w && my >= ey && my < ey + eh;
            if hit && e.target != Selection::None {
                draw_rectangle(x, ey, w, eh, Color::new(0.1, 0.2, 0.32, 0.6));
                if is_mouse_button_pressed(MouseButton::Left) {
                    clicked = Some(e.target);
                }
            }
            for (i, line) in e.lines.iter().enumerate() {
                draw_clipped_text(line, x + 10.0, ey + 13.0 + i as f32 * LINE_H, FONT_PX, w - 20.0, e.color);
            }
            ey += eh;
        }
        clicked
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sold_out_line_names_venue_and_targets_it() {
        let w = World::new(3, 4);
        let venue = w.city.buildings.iter().find(|b| b.kind.is_food()).unwrap();
        let ev = SimEvent { tick: TICKS_PER_HOUR * 13, kind: EventKind::VenueSoldOut { building: venue.id } };
        let (text, target) = format_event(&w, &ev);
        assert!(text.contains("sold out"), "{text}");
        assert!(text.contains(venue.kind.name()), "{text}");
        assert!(text.contains("D1 13:00"), "{text}");
        assert_eq!(target, Selection::Building(venue.id));
    }

    #[test]
    fn critical_need_line_names_citizen_and_targets_them() {
        let w = World::new(3, 4);
        let ev = SimEvent { tick: 30, kind: EventKind::CriticalNeed { citizen: 2, need: NeedKind::Hunger } };
        let (text, target) = format_event(&w, &ev);
        assert!(text.contains(&w.citizens[2].name), "{text}");
        assert!(text.contains("starving"), "{text}");
        assert_eq!(target, Selection::Citizen(2));
    }

    #[test]
    fn daily_wages_line_has_no_target() {
        let w = World::new(3, 4);
        let ev = SimEvent { tick: crate::sim::time::TICKS_PER_DAY, kind: EventKind::DailyWages { day: 1, total: 1240.0 } };
        let (text, target) = format_event(&w, &ev);
        assert!(text.contains("1240"), "{text}");
        assert!(text.contains("Day 1"), "{text}");
        assert_eq!(target, Selection::None);
    }

    #[test]
    fn cant_afford_line_names_both_and_targets_citizen() {
        let w = World::new(3, 4);
        let venue = w.city.buildings.iter().find(|b| b.kind.is_food()).unwrap();
        let ev = SimEvent { tick: 30, kind: EventKind::CantAffordMeal { citizen: 1, building: venue.id } };
        let (text, target) = format_event(&w, &ev);
        assert!(text.contains(&w.citizens[1].name), "{text}");
        assert!(text.contains("can't afford"), "{text}");
        assert!(text.contains(venue.kind.name()), "{text}");
        assert_eq!(target, Selection::Citizen(1));
    }

    #[test]
    fn insolvent_line_names_employer_and_targets_it() {
        let w = World::new(3, 4);
        let farm = w
            .city
            .buildings
            .iter()
            .find(|b| b.kind == crate::sim::city::BuildingKind::HydroFarm)
            .unwrap();
        let ev = SimEvent { tick: 30, kind: EventKind::EmployerInsolvent { building: farm.id } };
        let (text, target) = format_event(&w, &ev);
        assert!(text.contains("payroll"), "{text}");
        assert!(text.contains(farm.kind.name()), "{text}");
        assert_eq!(target, Selection::Building(farm.id));
    }

    #[test]
    fn worker_quit_line_names_both_and_targets_citizen() {
        let w = World::new(3, 4);
        let farm = w
            .city
            .buildings
            .iter()
            .find(|b| b.kind == crate::sim::city::BuildingKind::HydroFarm)
            .unwrap();
        let ev = SimEvent { tick: 30, kind: EventKind::WorkerQuit { citizen: 1, building: farm.id } };
        let (text, target) = format_event(&w, &ev);
        assert!(text.contains(&w.citizens[1].name), "{text}");
        assert!(text.contains("quit"), "{text}");
        assert!(text.contains(farm.kind.name()), "{text}");
        assert_eq!(target, Selection::Citizen(1));
    }
}
