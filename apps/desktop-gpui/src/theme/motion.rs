#![allow(dead_code)]

use std::time::Duration;

pub const FAST: Duration = Duration::from_millis(120);
pub const MID: Duration = Duration::from_millis(180);
pub const EXIT: Duration = Duration::from_millis(120);
pub const MOVE: Duration = Duration::from_millis(380);
pub const EXPAND: Duration = Duration::from_millis(300);
pub const COLLAPSE: Duration = Duration::from_millis(150);
pub const PALETTE_IN: Duration = Duration::from_millis(150);
pub const PALETTE_OUT: Duration = Duration::from_millis(100);

pub const EASE_EFFECTS: CubicBezier = CubicBezier::new(0.2, 0., 0., 1.);
pub const EASE_EXIT: CubicBezier = CubicBezier::new(0.4, 0., 1., 1.);
pub const EASE_FLUENT_IN: CubicBezier = CubicBezier::new(0., 0., 0., 1.);
pub const EASE_FLUENT_OUT: CubicBezier = CubicBezier::new(1., 0., 1., 1.);

pub const MOVE_ZETA: f32 = 0.75;
pub const MOVE_OMEGA: f32 = 10.;

const BISECTION_STEPS: usize = 24;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CubicBezier {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

impl CubicBezier {
    pub const fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self { x1, y1, x2, y2 }
    }

    pub fn at(self, t: f32) -> f32 {
        if t <= 0. {
            return 0.;
        }
        if t >= 1. {
            return 1.;
        }
        let (mut low, mut high) = (0., 1.);
        for _ in 0..BISECTION_STEPS {
            let middle = (low + high) / 2.;
            if cubic(self.x1, self.x2, middle) < t {
                low = middle;
            } else {
                high = middle;
            }
        }
        cubic(self.y1, self.y2, (low + high) / 2.)
    }

    pub fn curve(self) -> impl Fn(f32) -> f32 {
        move |t| self.at(t)
    }
}

fn cubic(first: f32, second: f32, s: f32) -> f32 {
    let u = 1. - s;
    3. * u * u * s * first + 3. * u * s * s * second + s * s * s
}

pub fn ease_move(t: f32) -> f32 {
    if t <= 0. {
        return 0.;
    }
    if t >= 1. {
        return 1.;
    }
    let damped = MOVE_OMEGA * (1. - MOVE_ZETA * MOVE_ZETA).sqrt();
    let decay = MOVE_ZETA * MOVE_OMEGA;
    1. - (-decay * t).exp() * ((damped * t).cos() + decay / damped * (damped * t).sin())
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;

    const TOKENS: &str = include_str!("../../../../docs/design/tokens.json");

    fn motion_tokens() -> Value {
        let tokens: Value = serde_json::from_str(TOKENS).expect("tokens.json parses");
        tokens["motion"].clone()
    }

    fn bezier_text(curve: CubicBezier) -> String {
        format!(
            "cubic-bezier({}, {}, {}, {})",
            curve.x1, curve.y1, curve.x2, curve.y2
        )
    }

    #[test]
    fn durations_match_tokens_json() {
        let tokens = motion_tokens();
        for (name, duration) in [
            ("fast", FAST),
            ("mid", MID),
            ("exit", EXIT),
            ("move", MOVE),
            ("expand", EXPAND),
            ("collapse", COLLAPSE),
            ("palette-in", PALETTE_IN),
            ("palette-out", PALETTE_OUT),
        ] {
            assert_eq!(
                tokens["duration"][name].as_u64(),
                Some(duration.as_millis() as u64),
                "{name}"
            );
        }
    }

    #[test]
    fn curves_match_tokens_json() {
        let tokens = motion_tokens();
        for (name, curve) in [
            ("effects", EASE_EFFECTS),
            ("exit", EASE_EXIT),
            ("fluent-in", EASE_FLUENT_IN),
            ("fluent-out", EASE_FLUENT_OUT),
        ] {
            assert_eq!(tokens["ease"][name], bezier_text(curve), "{name}");
        }
        assert_eq!(tokens["move"]["zeta"].as_f64(), Some(f64::from(MOVE_ZETA)));
        assert_eq!(
            tokens["move"]["omega"].as_f64(),
            Some(f64::from(MOVE_OMEGA))
        );
    }

    #[test]
    fn a_straight_bezier_is_the_identity() {
        let straight = CubicBezier::new(1. / 3., 1. / 3., 2. / 3., 2. / 3.);
        for t in [0., 0.25, 0.5, 0.8, 1.] {
            assert!((straight.at(t) - t).abs() < 0.001, "{t}");
        }
    }

    #[test]
    fn colour_and_opacity_curves_land_exactly_and_never_overshoot() {
        for curve in [EASE_EFFECTS, EASE_EXIT, EASE_FLUENT_IN, EASE_FLUENT_OUT] {
            assert_eq!(curve.at(0.), 0.);
            assert_eq!(curve.at(1.), 1.);
            let mut previous = 0.;
            for step in 0..=100 {
                let value = curve.at(step as f32 / 100.);
                assert!(value >= previous - 0.0001 && value <= 1.);
                previous = value;
            }
        }
    }

    #[test]
    fn decelerating_curves_are_past_halfway_at_the_halfway_point() {
        assert!(EASE_EFFECTS.at(0.5) > 0.5);
        assert!(EASE_EXIT.at(0.5) < 0.5);
    }

    #[test]
    fn the_move_spring_settles_on_the_end_with_a_small_overshoot() {
        assert_eq!(ease_move(0.), 0.);
        assert_eq!(ease_move(1.), 1.);
        let peak = (0..=1000)
            .map(|step| ease_move(step as f32 / 1000.))
            .fold(0., f32::max);
        assert!(peak > 1.02 && peak < 1.029, "{peak}");
    }
}
