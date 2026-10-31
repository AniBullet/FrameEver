use gpui::Rgba;

use super::Appearance;

pub const ROW_HEIGHT: f32 = 34.;
pub const CONTROL_HEIGHT: f32 = 32.;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Oklch {
    pub l: f32,
    pub c: f32,
    pub h: f32,
    pub alpha: f32,
}

const fn ok(l: f32, c: f32, h: f32) -> Oklch {
    Oklch { l, c, h, alpha: 1. }
}

const fn ok_a(l: f32, c: f32, h: f32, alpha: f32) -> Oklch {
    Oklch { l, c, h, alpha }
}

impl Oklch {
    fn linear_srgb(self) -> [f64; 3] {
        let l = f64::from(self.l);
        let c = f64::from(self.c);
        let h = f64::from(self.h).to_radians();
        let (a, b) = (c * h.cos(), c * h.sin());
        let m1 = (l + 0.3963377774 * a + 0.2158037573 * b).powi(3);
        let m2 = (l - 0.1055613458 * a - 0.0638541728 * b).powi(3);
        let m3 = (l - 0.0894841775 * a - 1.291485548 * b).powi(3);
        [
            4.0767416621 * m1 - 3.3077115913 * m2 + 0.2309699292 * m3,
            -1.2684380046 * m1 + 2.6097574011 * m2 - 0.3413193965 * m3,
            -0.0041960863 * m1 - 0.7034186147 * m2 + 1.707614701 * m3,
        ]
    }

    pub fn to_rgba(self) -> Rgba {
        let gamma = |x: f64| {
            let y = x.clamp(0., 1.);
            let encoded = if y <= 0.0031308 {
                12.92 * y
            } else {
                1.055 * y.powf(1. / 2.4) - 0.055
            };
            encoded as f32
        };
        let [r, g, b] = self.linear_srgb();
        Rgba {
            r: gamma(r),
            g: gamma(g),
            b: gamma(b),
            a: self.alpha,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatusRamp {
    pub tint: Oklch,
    pub solid: Oklch,
    pub hover: Oklch,
    pub text: Oklch,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FixedColors {
    pub white: Oklch,
    pub recording: Oklch,
    pub record_gap: Oklch,
    pub preview: Oklch,
    pub preview_border: Oklch,
    pub preview_2: Oklch,
    pub preview_3: Oklch,
    pub badge: Oklch,
    pub overlay: Oklch,
    pub klein_tint_bar: Oklch,
    pub track_zoom: Oklch,
    pub track_text: Oklch,
    pub track_audio: Oklch,
    pub track_input: Oklch,
    pub track_style: Oklch,
    pub track_mask: Oklch,
    pub track_shot: Oklch,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Brand {
    pub neutral: [Oklch; 12],
    pub klein: [Oklch; 12],
    pub red: StatusRamp,
    pub amber: StatusRamp,
    pub green: StatusRamp,
    pub fixed: FixedColors,
}

const KLEIN_HUE: f32 = 266.4;
const NEUTRAL_HUE: f32 = 266.;

const TRACKS: [Oklch; 7] = [
    ok(0.473, 0.132, 268.5),
    ok(0.543, 0.082, 176.5),
    ok(0.471, 0.125, 304.2),
    ok(0.546, 0.103, 73.5),
    ok(0.51, 0.116, 350.4),
    ok(0.509, 0.122, 26.1),
    ok(0.456, 0.133, 280.3),
];

pub const DARK: Brand = Brand {
    neutral: [
        ok(0.145, 0.004, NEUTRAL_HUE),
        ok(0.165, 0.004, NEUTRAL_HUE),
        ok(0.195, 0.005, NEUTRAL_HUE),
        ok(0.24, 0.006, NEUTRAL_HUE),
        ok(0.25, 0.006, NEUTRAL_HUE),
        ok(0.265, 0.006, NEUTRAL_HUE),
        ok(0.29, 0.006, NEUTRAL_HUE),
        ok(0.53, 0.006, NEUTRAL_HUE),
        ok(0.6, 0.006, NEUTRAL_HUE),
        ok(0.66, 0.007, NEUTRAL_HUE),
        ok(0.72, 0.008, NEUTRAL_HUE),
        ok(0.94, 0.003, NEUTRAL_HUE),
    ],
    klein: [
        ok(0.18, 0.03, KLEIN_HUE),
        ok(0.205, 0.05, KLEIN_HUE),
        ok(0.25, 0.08, KLEIN_HUE),
        ok(0.3, 0.11, KLEIN_HUE),
        ok(0.32, 0.13, KLEIN_HUE),
        ok(0.34, 0.15, KLEIN_HUE),
        ok(0.38, 0.18, KLEIN_HUE),
        ok(0.42, 0.2, KLEIN_HUE),
        ok(0.463, 0.233, KLEIN_HUE),
        ok(0.53, 0.215, KLEIN_HUE),
        ok(0.76, 0.12, KLEIN_HUE),
        ok(0.93, 0.032, KLEIN_HUE),
    ],
    red: StatusRamp {
        tint: ok(0.27, 0.07, 25.),
        solid: ok(0.56, 0.21, 25.),
        hover: ok(0.52, 0.2, 25.),
        text: ok(0.74, 0.158, 25.),
    },
    amber: StatusRamp {
        tint: ok(0.28, 0.06, 75.),
        solid: ok(0.8, 0.15, 75.),
        hover: ok(0.84, 0.138, 75.),
        text: ok(0.82, 0.14, 80.),
    },
    green: StatusRamp {
        tint: ok(0.27, 0.06, 155.),
        solid: ok(0.7, 0.15, 155.),
        hover: ok(0.75, 0.14, 155.),
        text: ok(0.78, 0.14, 155.),
    },
    fixed: FixedColors {
        white: ok(0.99, 0., 0.),
        recording: ok(0.65, 0.22, 29.5),
        record_gap: ok(0.235, 0.135, 264.),
        preview: ok(0.26, 0., 0.),
        preview_border: ok(0.3, 0., 0.),
        preview_2: ok(0.36, 0., 0.),
        preview_3: ok(0.44, 0., 0.),
        badge: ok_a(0.15, 0., 0., 0.82),
        overlay: ok_a(0.145, 0.004, NEUTRAL_HUE, 0.6),
        klein_tint_bar: ok(0.38, 0.18, KLEIN_HUE),
        track_zoom: TRACKS[0],
        track_text: TRACKS[1],
        track_audio: TRACKS[2],
        track_input: TRACKS[3],
        track_style: TRACKS[4],
        track_mask: TRACKS[5],
        track_shot: TRACKS[6],
    },
};

pub const LIGHT: Brand = Brand {
    neutral: [
        ok(0.995, 0.001, NEUTRAL_HUE),
        ok(0.985, 0.002, NEUTRAL_HUE),
        ok(0.965, 0.003, NEUTRAL_HUE),
        ok(0.94, 0.004, NEUTRAL_HUE),
        ok(0.925, 0.004, NEUTRAL_HUE),
        ok(0.905, 0.005, NEUTRAL_HUE),
        ok(0.875, 0.005, NEUTRAL_HUE),
        ok(0.6, 0.007, NEUTRAL_HUE),
        ok(0.56, 0.007, NEUTRAL_HUE),
        ok(0.52, 0.008, NEUTRAL_HUE),
        ok(0.49, 0.01, NEUTRAL_HUE),
        ok(0.205, 0.006, NEUTRAL_HUE),
    ],
    klein: [
        ok(0.99, 0.004, KLEIN_HUE),
        ok(0.975, 0.011, KLEIN_HUE),
        ok(0.955, 0.021, KLEIN_HUE),
        ok(0.92, 0.038, KLEIN_HUE),
        ok(0.89, 0.053, KLEIN_HUE),
        ok(0.85, 0.073, KLEIN_HUE),
        ok(0.78, 0.105, KLEIN_HUE),
        ok(0.7, 0.14, KLEIN_HUE),
        ok(0.463, 0.233, KLEIN_HUE),
        ok(0.42, 0.215, KLEIN_HUE),
        ok(0.4, 0.19, KLEIN_HUE),
        ok(0.27, 0.11, KLEIN_HUE),
    ],
    red: StatusRamp {
        tint: ok(0.955, 0.022, 25.),
        solid: ok(0.56, 0.21, 25.),
        hover: ok(0.5, 0.2, 25.),
        text: ok(0.5, 0.2, 25.),
    },
    amber: StatusRamp {
        tint: ok(0.96, 0.04, 85.),
        solid: ok(0.78, 0.15, 75.),
        hover: ok(0.74, 0.15, 75.),
        text: ok(0.5, 0.11, 65.),
    },
    green: StatusRamp {
        tint: ok(0.955, 0.03, 155.),
        solid: ok(0.52, 0.129, 155.),
        hover: ok(0.47, 0.116, 155.),
        text: ok(0.47, 0.116, 155.),
    },
    fixed: FixedColors {
        white: ok(0.99, 0., 0.),
        recording: ok(0.65, 0.22, 29.5),
        record_gap: ok(0.235, 0.135, 264.),
        preview: ok(0.9, 0., 0.),
        preview_border: ok(0.86, 0., 0.),
        preview_2: ok(0.97, 0., 0.),
        preview_3: ok(0.8, 0., 0.),
        badge: ok_a(0.15, 0., 0., 0.82),
        overlay: ok_a(0.205, 0.006, NEUTRAL_HUE, 0.6),
        klein_tint_bar: ok(0.78, 0.105, KLEIN_HUE),
        track_zoom: TRACKS[0],
        track_text: TRACKS[1],
        track_audio: TRACKS[2],
        track_input: TRACKS[3],
        track_style: TRACKS[4],
        track_mask: TRACKS[5],
        track_shot: TRACKS[6],
    },
};

impl Brand {
    pub fn of(appearance: Appearance) -> &'static Brand {
        static DARK_BRAND: Brand = DARK;
        static LIGHT_BRAND: Brand = LIGHT;
        match appearance {
            Appearance::Dark => &DARK_BRAND,
            Appearance::Light => &LIGHT_BRAND,
        }
    }

    const fn n(&self, step: usize) -> Oklch {
        self.neutral[step - 1]
    }

    const fn k(&self, step: usize) -> Oklch {
        self.klein[step - 1]
    }
}

macro_rules! color_layer {
    ($layer:ident, $brand:ident; $($name:ident: $dark:expr, $light:expr;)*) => {
        #[allow(dead_code)]
        #[derive(Debug, Clone, Copy, PartialEq)]
        pub struct $layer {
            $(pub $name: Rgba,)*
        }

        impl $layer {
            #[cfg(test)]
            fn oklch(appearance: Appearance) -> Vec<(&'static str, Oklch)> {
                let $brand = Brand::of(appearance);
                let dark = appearance == Appearance::Dark;
                vec![$((stringify!($name), if dark { $dark } else { $light }),)*]
            }

            pub fn new(appearance: Appearance) -> Self {
                let $brand = Brand::of(appearance);
                let dark = appearance == Appearance::Dark;
                Self {
                    $($name: (if dark { $dark } else { $light }).to_rgba(),)*
                }
            }
        }
    };
}

color_layer! {
    Semantic, b;
    background: b.n(2), b.n(2);
    foreground: b.n(12), b.n(12);
    page: b.n(1), b.n(3);
    card: b.n(3), b.n(3);
    card_foreground: b.n(12), b.n(12);
    popover: b.n(4), b.n(1);
    popover_foreground: b.n(12), b.n(12);
    muted: b.n(3), b.n(3);
    muted_foreground: b.n(11), b.n(11);
    secondary: b.n(4), b.n(4);
    secondary_hover: b.n(5), b.n(5);
    secondary_active: b.n(7), b.n(7);
    secondary_foreground: b.n(12), b.n(12);
    accent: b.n(4), b.n(4);
    accent_foreground: b.n(12), b.n(12);
    primary: b.k(9), b.k(9);
    primary_hover: b.k(10), b.k(10);
    primary_active: b.k(8), b.k(11);
    primary_foreground: b.fixed.white, b.fixed.white;
    link: b.k(11), b.k(9);
    ring: b.k(11), b.k(9);
    selection: b.n(7), b.n(6);
    selection_foreground: b.n(12), b.n(12);
    border: b.n(6), b.n(6);
    border_strong: b.n(8), b.n(8);
    input: b.n(8), b.n(8);
    sidebar: b.n(1), b.n(3);
    danger: b.red.solid, b.red.solid;
    danger_hover: b.red.hover, b.red.hover;
    danger_foreground: b.fixed.white, b.fixed.white;
    danger_text: b.red.text, b.red.text;
    danger_tint: b.red.tint, b.red.tint;
    warning: b.amber.solid, b.amber.solid;
    warning_foreground: b.n(1), b.n(12);
    warning_text: b.amber.text, b.amber.text;
    success: b.green.solid, b.green.solid;
    success_text: b.green.text, b.green.text;
    success_tint: b.green.tint, b.green.tint;
    info: b.k(11), b.k(9);
    recording: b.fixed.recording, b.fixed.recording;
    record_gap: b.fixed.record_gap, b.fixed.record_gap;
    preview: b.fixed.preview, b.fixed.preview;
    preview_border: b.fixed.preview_border, b.fixed.preview_border;
    track_zoom: b.fixed.track_zoom, b.fixed.track_zoom;
    track_text: b.fixed.track_text, b.fixed.track_text;
    track_audio: b.fixed.track_audio, b.fixed.track_audio;
    track_input: b.fixed.track_input, b.fixed.track_input;
    track_style: b.fixed.track_style, b.fixed.track_style;
    track_mask: b.fixed.track_mask, b.fixed.track_mask;
    track_shot: b.fixed.track_shot, b.fixed.track_shot;
    overlay: b.fixed.overlay, b.fixed.overlay;
}

color_layer! {
    Component, b;
    switch: b.n(8), b.n(8);
    switch_thumb: b.n(12), b.fixed.white;
    slider_bar: b.n(6), b.n(6);
    slider_fill: b.k(9), b.k(9);
    slider_thumb: b.n(12), b.n(12);
    progress_bar: b.k(9), b.k(9);
    scrollbar_thumb: b.n(9), b.n(9);
    scrollbar_thumb_hover: b.n(10), b.n(10);
    playhead: b.n(12), b.n(12);
    waveform: b.n(11), b.n(11);
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;

    const TOKENS: &str = include_str!("../../../../docs/design/tokens.json");

    fn tokens() -> Value {
        serde_json::from_str(TOKENS).expect("tokens.json parses")
    }

    fn theme_key(appearance: Appearance) -> &'static str {
        match appearance {
            Appearance::Dark => "dark",
            Appearance::Light => "light",
        }
    }

    const BOTH: [Appearance; 2] = [Appearance::Dark, Appearance::Light];

    fn json_oklch(value: &Value) -> Oklch {
        let channel = |index: usize| value[index].as_f64().expect("a number") as f32;
        Oklch {
            l: channel(0),
            c: channel(1),
            h: channel(2),
            alpha: value.get(3).and_then(Value::as_f64).unwrap_or(1.) as f32,
        }
    }

    fn resolve(tokens: &Value, theme: &str, reference: &str) -> Oklch {
        if let Some((family, step)) = reference.split_once('.') {
            let family = &tokens["brand"][theme][family];
            return match step.parse::<usize>() {
                Ok(step) => json_oklch(&family[step - 1]),
                Err(_) => json_oklch(&family[step]),
            };
        }
        let list = tokens["semantic"]
            .as_array()
            .expect("semantic is a list")
            .iter()
            .chain(tokens["componentVars"].as_array().expect("a list"));
        let entry = list
            .into_iter()
            .find(|entry| entry["name"] == reference)
            .unwrap_or_else(|| panic!("unknown token {reference}"));
        resolve(tokens, theme, entry[theme].as_str().expect("a reference"))
    }

    fn hex_of(color: Rgba) -> String {
        let byte = |channel: f32| (channel * 255.).round() as u8;
        format!(
            "{:02X}{:02X}{:02X}",
            byte(color.r),
            byte(color.g),
            byte(color.b)
        )
    }

    fn channel_luminance(channel: f32) -> f64 {
        let channel = f64::from(channel);
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    }

    fn luminance(color: Rgba) -> f64 {
        0.2126 * channel_luminance(color.r)
            + 0.7152 * channel_luminance(color.g)
            + 0.0722 * channel_luminance(color.b)
    }

    fn over(top: Rgba, under: Rgba) -> Rgba {
        super::super::mix(under, Rgba { a: under.a, ..top }, top.a)
    }

    fn contrast(foreground: Rgba, background: Rgba) -> f64 {
        let (a, b) = (luminance(foreground), luminance(background));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    fn semantic(appearance: Appearance, name: &str) -> Rgba {
        Semantic::oklch(appearance)
            .into_iter()
            .find(|(token, _)| *token == name)
            .unwrap_or_else(|| panic!("no semantic token {name}"))
            .1
            .to_rgba()
    }

    #[test]
    fn brand_ramps_match_tokens_json() {
        let tokens = tokens();
        for appearance in BOTH {
            let theme = theme_key(appearance);
            let brand = Brand::of(appearance);
            let json = &tokens["brand"][theme];
            for (family, ramp) in [("neutral", &brand.neutral), ("klein", &brand.klein)] {
                let steps = json[family].as_array().expect("a ramp");
                assert_eq!(steps.len(), ramp.len());
                for (step, color) in steps.iter().zip(ramp) {
                    assert_eq!(json_oklch(step), *color, "{theme} {family}");
                }
            }
            for (family, ramp) in [
                ("red", &brand.red),
                ("amber", &brand.amber),
                ("green", &brand.green),
            ] {
                for (step, color) in [
                    ("tint", ramp.tint),
                    ("solid", ramp.solid),
                    ("hover", ramp.hover),
                    ("text", ramp.text),
                ] {
                    assert_eq!(json_oklch(&json[family][step]), color, "{theme} {family}");
                }
            }
            let fixed = brand.fixed;
            let rust = [
                ("white", fixed.white),
                ("recording", fixed.recording),
                ("record-gap", fixed.record_gap),
                ("preview", fixed.preview),
                ("preview-border", fixed.preview_border),
                ("preview-2", fixed.preview_2),
                ("preview-3", fixed.preview_3),
                ("badge", fixed.badge),
                ("overlay", fixed.overlay),
                ("klein-tint-bar", fixed.klein_tint_bar),
                ("track-zoom", fixed.track_zoom),
                ("track-text", fixed.track_text),
                ("track-audio", fixed.track_audio),
                ("track-input", fixed.track_input),
                ("track-style", fixed.track_style),
                ("track-mask", fixed.track_mask),
                ("track-shot", fixed.track_shot),
            ];
            assert_eq!(json["fixed"].as_object().expect("a map").len(), rust.len());
            for (name, color) in rust {
                assert_eq!(json_oklch(&json["fixed"][name]), color, "{theme} {name}");
            }
        }
    }

    #[test]
    fn semantic_layer_matches_tokens_json() {
        let tokens = tokens();
        let entries = tokens["semantic"].as_array().expect("a list");
        for appearance in BOTH {
            let theme = theme_key(appearance);
            let rust = Semantic::oklch(appearance);
            assert_eq!(rust.len(), entries.len());
            for entry in entries {
                let name = entry["name"].as_str().expect("a name");
                let found = rust
                    .iter()
                    .find(|(token, _)| *token == name)
                    .unwrap_or_else(|| panic!("no semantic token {name}"));
                assert_eq!(found.1, resolve(&tokens, theme, name), "{theme} {name}");
            }
        }
    }

    #[test]
    fn component_layer_matches_tokens_json() {
        let tokens = tokens();
        for appearance in BOTH {
            let theme = theme_key(appearance);
            let rust = Component::oklch(appearance);
            let find = |name: &str| {
                rust.iter()
                    .find(|(token, _)| *token == name)
                    .unwrap_or_else(|| panic!("no component token {name}"))
                    .1
            };
            for entry in tokens["componentVars"].as_array().expect("a list") {
                let name = entry["name"].as_str().expect("a name");
                assert_eq!(find(name), resolve(&tokens, theme, name), "{theme} {name}");
            }
            let mut checked = 0;
            for row in tokens["component"].as_array().expect("a list") {
                let fields = row["field"].as_str().expect("a field").split(" / ");
                let sources: Vec<&str> = row["from"]
                    .as_str()
                    .expect("a source")
                    .split(" / ")
                    .collect();
                let fields: Vec<&str> = fields.collect();
                if fields.len() != sources.len() {
                    continue;
                }
                for (field, source) in fields.iter().zip(&sources) {
                    let name = match field.strip_prefix('_') {
                        Some(suffix) => format!("{}_{suffix}", fields[0]),
                        None => (*field).to_string(),
                    };
                    if rust.iter().any(|(token, _)| *token == name) {
                        let reference = source.replacen("neutral-", "neutral.", 1);
                        assert_eq!(
                            find(&name),
                            resolve(&tokens, theme, &reference),
                            "{theme} {name}"
                        );
                        checked += 1;
                    }
                }
            }
            assert_eq!(checked, 8, "{theme}");
        }
    }

    #[test]
    fn sizes_match_tokens_json() {
        let tokens = tokens();
        let number = |value: &Value| value.as_f64().expect("a number") as f32;
        assert_eq!(number(&tokens["size"]["row"]), ROW_HEIGHT);
        assert_eq!(number(&tokens["size"]["control"]), CONTROL_HEIGHT);
    }

    #[test]
    fn klein_nine_is_the_brand_colour() {
        for appearance in BOTH {
            assert_eq!(hex_of(Brand::of(appearance).klein[8].to_rgba()), "1F3FD8");
        }
    }

    #[test]
    fn large_fills_stay_near_achromatic() {
        for appearance in BOTH {
            for step in Brand::of(appearance).neutral {
                assert!(step.c <= 0.01, "{step:?}");
            }
        }
    }

    #[test]
    fn text_pairs_reach_wcag_aa() {
        const PAIRS: [(&str, &str); 24] = [
            ("foreground", "background"),
            ("foreground", "card"),
            ("foreground", "popover"),
            ("foreground", "secondary"),
            ("foreground", "accent"),
            ("selection_foreground", "selection"),
            ("muted_foreground", "background"),
            ("muted_foreground", "card"),
            ("muted_foreground", "popover"),
            ("muted_foreground", "accent"),
            ("muted_foreground", "selection"),
            ("primary_foreground", "primary"),
            ("primary_foreground", "primary_hover"),
            ("primary_foreground", "primary_active"),
            ("link", "background"),
            ("link", "card"),
            ("link", "selection"),
            ("danger_text", "background"),
            ("danger_text", "danger_tint"),
            ("danger_foreground", "danger"),
            ("warning_text", "background"),
            ("warning_foreground", "warning"),
            ("success_text", "background"),
            ("success_text", "success_tint"),
        ];
        for appearance in BOTH {
            for (foreground, background) in PAIRS {
                let ratio = contrast(
                    semantic(appearance, foreground),
                    semantic(appearance, background),
                );
                assert!(
                    ratio >= 4.5,
                    "{appearance:?} {foreground} on {background} is {ratio:.2}:1"
                );
            }
            let fixed = Brand::of(appearance).fixed;
            let badge = over(fixed.badge.to_rgba(), semantic(appearance, "preview"));
            let ratio = contrast(fixed.white.to_rgba(), badge);
            assert!(ratio >= 4.5, "{appearance:?} badge is {ratio:.2}:1");
        }
    }

    #[test]
    fn timeline_track_colours_are_the_track_tokens() {
        use crate::editor_timeline::track_color;
        let packed = |color: Rgba| u32::from_str_radix(&hex_of(color), 16).expect("hex digits");
        let dark = Semantic::new(Appearance::Dark);
        for (constant, token) in [
            (track_color::ZOOM, dark.track_zoom),
            (track_color::CAPTION, dark.track_text),
            (track_color::TEXT, dark.track_text),
            (track_color::KEYBOARD, dark.track_input),
            (track_color::IMAGE, dark.track_input),
            (track_color::STYLE, dark.track_style),
            (track_color::MASK, dark.track_mask),
            (track_color::SCENE, dark.track_audio),
            (track_color::AUDIO, dark.track_audio),
            (track_color::THREE_D, dark.track_shot),
        ] {
            assert_eq!(constant, packed(token));
        }
    }

    #[test]
    fn gradients_only_paint_what_the_user_chose() {
        const USER_CONTENT: [&str; 4] = [
            "ui/color_picker.rs",
            "editor_sidebar.rs",
            "editor_sidebar/animated_gradient.rs",
            "screenshot_editor.rs",
        ];
        fn walk(dir: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(dir).expect("source folder").flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, files);
                } else if path.extension().is_some_and(|extension| extension == "rs") {
                    files.push(path);
                }
            }
        }
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        walk(&root, &mut files);
        let needle = ["linear", "_gradient("].concat();
        for file in files {
            let relative = file.strip_prefix(&root).expect("under src");
            let name = relative
                .to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/");
            let source = std::fs::read_to_string(&file).expect("readable source");
            assert!(
                USER_CONTENT.contains(&name.as_str()) || !source.contains(&needle),
                "{name} paints a gradient"
            );
        }
    }

    #[test]
    fn no_svg_asset_carries_a_gradient() {
        fn walk(dir: &std::path::Path) {
            for entry in std::fs::read_dir(dir).expect("asset folder").flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path);
                } else if path.extension().is_some_and(|extension| extension == "svg") {
                    let source = std::fs::read_to_string(&path).expect("readable svg");
                    assert!(!source.contains("linearGradient"), "{path:?}");
                }
            }
        }
        walk(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets"));
    }

    #[test]
    fn control_edges_reach_three_to_one() {
        const PAIRS: [(&str, &str); 6] = [
            ("ring", "background"),
            ("ring", "card"),
            ("input", "background"),
            ("input", "card"),
            ("border_strong", "background"),
            ("recording", "background"),
        ];
        for appearance in BOTH {
            for (edge, background) in PAIRS {
                let ratio = contrast(semantic(appearance, edge), semantic(appearance, background));
                assert!(
                    ratio >= 3.,
                    "{appearance:?} {edge} on {background} is {ratio:.2}:1"
                );
            }
        }
    }
}
