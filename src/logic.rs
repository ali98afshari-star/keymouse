//! Pure movement logic — no X11 needed, fully unit-tested.
//! Direct port of the v1.1 Python logic (`speed_at`, `direction_vector`,
//! `move_step`), so behaviour is identical.

/// Directions currently held.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HeldDirs {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
}

impl HeldDirs {
    pub fn is_empty(&self) -> bool {
        !self.up && !self.down && !self.left && !self.right
    }
}

/// Current px-per-tick: ramps from base to max. Pure.
pub fn speed_at(held_seconds: f64, base_speed: f64, max_speed: f64, accel_per_sec: f64) -> f64 {
    (base_speed + accel_per_sec * held_seconds).min(max_speed)
}

/// Held directions -> unit (dx, dy), or (0, 0). Diagonals are normalized.
pub fn direction_vector(held: HeldDirs) -> (f64, f64) {
    let dx = (held.right as i32 - held.left as i32) as f64;
    let dy = (held.down as i32 - held.up as i32) as f64;
    if dx != 0.0 && dy != 0.0 {
        let n = std::f64::consts::SQRT_2;
        (dx / n, dy / n)
    } else {
        (dx, dy)
    }
}

/// One tick of movement: (dx, dy) in whole pixels.
/// Slow (precise) wins when both fast and slow are held.
#[allow(clippy::too_many_arguments)]
pub fn move_step(
    held: HeldDirs,
    held_seconds: f64,
    fast: bool,
    slow: bool,
    base_speed: f64,
    max_speed: f64,
    accel_per_sec: f64,
    fast_multiplier: f64,
    slow_multiplier: f64,
) -> (i32, i32) {
    let (dx, dy) = direction_vector(held);
    if dx == 0.0 && dy == 0.0 {
        return (0, 0);
    }
    let mut speed = speed_at(held_seconds, base_speed, max_speed, accel_per_sec);
    if slow {
        speed *= slow_multiplier;
    } else if fast {
        speed *= fast_multiplier;
    }
    (Math::round(dx * speed) as i32, Math::round(dy * speed) as i32)
}

/// Small helper so the rounding matches Python's round-half-even closely
/// enough for our integer pixel steps (values here are never exactly .5
/// except 1.5 -> 2, which both round to 2).
struct Math;
impl Math {
    fn round(v: f64) -> f64 {
        v.round()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Defaults from v1.1: base 6, max 42, accel 90, fast x3, slow x0.25.
    const BASE: f64 = 6.0;
    const MAX: f64 = 42.0;
    const ACCEL: f64 = 90.0;
    const FAST: f64 = 3.0;
    const SLOW: f64 = 0.25;

    fn step(held: HeldDirs, s: f64, fast: bool, slow: bool) -> (i32, i32) {
        move_step(held, s, fast, slow, BASE, MAX, ACCEL, FAST, SLOW)
    }

    #[test]
    fn speed_ramps_and_caps() {
        assert_eq!(speed_at(0.0, BASE, MAX, ACCEL), 6.0);
        assert_eq!(speed_at(10.0, BASE, MAX, ACCEL), 42.0);
        assert!((speed_at(0.1, BASE, MAX, ACCEL) - 15.0).abs() < 1e-9);
    }

    #[test]
    fn direction_up_is_negative_y() {
        assert_eq!(direction_vector(HeldDirs { up: true, ..Default::default() }), (0.0, -1.0));
        assert_eq!(direction_vector(HeldDirs { left: true, ..Default::default() }), (-1.0, 0.0));
    }

    #[test]
    fn diagonal_is_unit_length() {
        let d = direction_vector(HeldDirs { up: true, right: true, ..Default::default() });
        let len = (d.0 * d.0 + d.1 * d.1).sqrt();
        assert!((len - 1.0).abs() < 1e-9, "diagonal {:?}", d);
        assert!(d.0 > 0.0 && d.1 < 0.0);
    }

    #[test]
    fn opposite_directions_cancel() {
        let d = direction_vector(HeldDirs { up: true, down: true, ..Default::default() });
        assert_eq!(d, (0.0, 0.0));
        let d = direction_vector(HeldDirs { left: true, right: true, ..Default::default() });
        assert_eq!(d, (0.0, 0.0));
    }

    #[test]
    fn move_steps_match_v1() {
        assert_eq!(step(HeldDirs::default(), 0.0, false, false), (0, 0));
        assert_eq!(step(HeldDirs { right: true, ..Default::default() }, 0.0, false, false), (6, 0));
        assert_eq!(step(HeldDirs { right: true, ..Default::default() }, 0.0, true, false), (18, 0)); // fast x3
        assert_eq!(step(HeldDirs { right: true, ..Default::default() }, 0.0, false, true), (2, 0)); // slow: 6*.25=1.5->2
        assert_eq!(step(HeldDirs { right: true, ..Default::default() }, 0.0, true, true), (2, 0)); // slow wins
        assert_eq!(step(HeldDirs { up: true, ..Default::default() }, 0.0, false, false), (0, -6));
    }

    #[test]
    fn move_step_accelerates() {
        // After 0.4s: 6 + 90*0.4 = 42 (capped at max).
        assert_eq!(step(HeldDirs { right: true, ..Default::default() }, 0.4, false, false), (42, 0));
        // After 0.1s: 6 + 9 = 15.
        assert_eq!(step(HeldDirs { right: true, ..Default::default() }, 0.1, false, false), (15, 0));
    }
}
