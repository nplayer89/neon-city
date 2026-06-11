use crate::sim::citizen::NeedKind;
use std::collections::VecDeque;

/// Sim-layer happenings surfaced by the UI news ticker.
/// Plain data only — wording and colors are the UI's job.
// PartialEq is derived; DailyWages::total is f32 — avoid == comparisons on that variant.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum EventKind {
    /// A food venue's last meal was just sold.
    VenueSoldOut { building: u16 },
    /// A need decayed across the critical threshold (ai::CRITICAL).
    CriticalNeed { citizen: usize, need: NeedKind },
    /// A citizen arrived hungry but couldn't pay for the meal.
    CantAffordMeal { citizen: usize, building: u16 },
    /// Total wages paid over the day that just ended.
    DailyWages { day: u64, total: f32 },
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SimEvent {
    pub tick: u64,
    pub kind: EventKind,
}

/// Pending-event cap so an undrained world (headless tests) can't grow unbounded.
pub const MAX_PENDING: usize = 256;

/// Push with cap: oldest events drop first.
pub fn push_event(events: &mut VecDeque<SimEvent>, ev: SimEvent) {
    if events.len() == MAX_PENDING {
        events.pop_front();
    }
    events.push_back(ev);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_caps_pending_events_dropping_oldest() {
        let mut q = VecDeque::new();
        for i in 0..(MAX_PENDING as u64 + 40) {
            push_event(&mut q, SimEvent { tick: i, kind: EventKind::VenueSoldOut { building: 0 } });
        }
        assert_eq!(q.len(), MAX_PENDING);
        assert_eq!(q.front().unwrap().tick, 40, "oldest events should drop first");
        assert_eq!(q.back().unwrap().tick, MAX_PENDING as u64 + 39);
    }
}
