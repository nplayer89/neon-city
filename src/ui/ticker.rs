use crate::sim::citizen::NeedKind;
use crate::sim::event::{EventKind, SimEvent};
use crate::sim::time::{self, TICKS_PER_HOUR};
use crate::sim::world::World;
use crate::ui::hud::{over, HudState, CYAN, PANEL, PANEL_EDGE};
use crate::ui::roster::draw_clipped_text;
use crate::ui::inspector::Selection;
use macroquad::prelude::*;
use std::collections::VecDeque;

/// Visible rows; older entries scroll off.
const MAX_ENTRIES: usize = 5;
const ROW_H: f32 = 20.0;
const TICKER_W: f32 = 470.0;
/// Clear of the roster sidebar (240px) on the left.
const TICKER_X: f32 = 254.0;

struct Entry {
    text: String,
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
            format!("[{stamp}] Day {day} wrap-up: ₢ {total:.0} paid in wages"),
            Selection::None,
        ),
    }
}

fn event_color(kind: &EventKind) -> Color {
    match kind {
        EventKind::VenueSoldOut { .. } => Color::new(1.0, 0.75, 0.25, 1.0),
        EventKind::CriticalNeed { .. } => Color::new(1.0, 0.35, 0.35, 1.0),
        EventKind::CantAffordMeal { .. } => Color::new(1.0, 0.3, 0.85, 1.0),
        EventKind::DailyWages { .. } => CYAN,
    }
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
        self.entries.push_back(Entry { text, target, color: event_color(&ev.kind) });
    }

    /// Bottom strip, newest row last. Returns a clicked row's target.
    pub fn draw(&self, hud: &mut HudState) -> Option<Selection> {
        if self.entries.is_empty() {
            return None;
        }
        let h = ROW_H * self.entries.len() as f32;
        let y = screen_height() - 36.0 - h;
        draw_rectangle(TICKER_X, y, TICKER_W, h, PANEL);
        draw_rectangle_lines(TICKER_X, y, TICKER_W, h, 1.0, PANEL_EDGE);
        if over(TICKER_X, y, TICKER_W, h) {
            hud.pointer_over_ui = true;
        }
        let (mx, my) = mouse_position();
        let mut clicked = None;
        for (i, e) in self.entries.iter().enumerate() {
            let ry = y + i as f32 * ROW_H;
            let row_hit = mx >= TICKER_X && mx <= TICKER_X + TICKER_W && my >= ry && my < ry + ROW_H;
            if row_hit && e.target != Selection::None {
                draw_rectangle(TICKER_X, ry, TICKER_W, ROW_H, Color::new(0.1, 0.2, 0.32, 0.6));
                if is_mouse_button_pressed(MouseButton::Left) {
                    clicked = Some(e.target);
                }
            }
            draw_clipped_text(&e.text, TICKER_X + 10.0, ry + 15.0, 16, TICKER_W - 20.0, e.color);
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
}
