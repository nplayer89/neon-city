pub const TICKS_PER_SECOND: u32 = 60;
/// 1 game day = 4 real minutes at 1x speed.
pub const TICKS_PER_HOUR: u64 = 600;
pub const HOURS_PER_DAY: u64 = 24;
pub const TICKS_PER_DAY: u64 = TICKS_PER_HOUR * HOURS_PER_DAY;
pub const TICK_DT: f32 = 1.0 / TICKS_PER_SECOND as f32;

/// Day number, starting at 1.
pub fn day(tick: u64) -> u64 {
    tick / TICKS_PER_DAY + 1
}

/// Hour of day, 0..24.
pub fn hour(tick: u64) -> u32 {
    ((tick % TICKS_PER_DAY) / TICKS_PER_HOUR) as u32
}

/// Fractional hour of day, 0.0..24.0 (drives lighting).
pub fn hour_f(tick: u64) -> f32 {
    (tick % TICKS_PER_DAY) as f32 / TICKS_PER_HOUR as f32
}

/// Daylight factor 0.0 (deep night) ..= 1.0 (midday), peaking at 13:00.
pub fn daylight(tick: u64) -> f32 {
    let t = (hour_f(tick) - 13.0) / 24.0 * std::f32::consts::TAU;
    (t.cos() * 0.5 + 0.5).powf(1.4)
}

pub fn is_night(tick: u64) -> bool {
    let h = hour(tick);
    h >= 22 || h < 6
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_math() {
        assert_eq!(day(0), 1);
        assert_eq!(hour(0), 0);
        assert_eq!(hour(TICKS_PER_HOUR * 13), 13);
        assert_eq!(day(TICKS_PER_DAY), 2);
        assert_eq!(hour(TICKS_PER_DAY + TICKS_PER_HOUR * 5), 5);
    }

    #[test]
    fn daylight_curve() {
        let noon = TICKS_PER_HOUR * 13;
        let midnight = TICKS_PER_HOUR * 1;
        assert!(daylight(noon) > 0.95);
        assert!(daylight(midnight) < 0.05);
        assert!(is_night(midnight) && !is_night(noon));
    }
}
