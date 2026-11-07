use gpui::{
    App, Context, Entity, Hsla, IntoElement, ParentElement as _, Render, Styled as _, Window, div,
    prelude::FluentBuilder as _, px,
};

use crate::feeds::{self, Feeds, MicLevel};

const RECORDING_BAR_TRACK_WIDTH: f32 = 20.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MicrophoneLevelKind {
    CaptureButton,
    DeviceRow,
    RecordingBar,
}

impl MicrophoneLevelKind {
    pub(crate) fn fill(self, db: f64) -> f32 {
        if !db.is_finite() {
            return 0.;
        }
        match self {
            Self::CaptureButton | Self::DeviceRow => (1. - feeds::picker_level(db)) as f32,
            Self::RecordingBar => feeds::bar_level(db) as f32,
        }
    }

    fn span(self, viewport_width: f32, scale: f32) -> f32 {
        match self {
            Self::CaptureButton | Self::DeviceRow => viewport_width * scale,
            Self::RecordingBar => RECORDING_BAR_TRACK_WIDTH * scale,
        }
    }

    fn shown(self, window: &Window) -> bool {
        match self {
            Self::RecordingBar => true,
            Self::CaptureButton | Self::DeviceRow => {
                !cfg!(any(target_os = "macos", target_os = "windows"))
                    || crate::platform::window_is_visible(window)
            }
        }
    }
}

pub(crate) fn moves_a_device_pixel(drawn: f32, next: f32, span: f32) -> bool {
    (drawn * span).round() != (next * span).round()
}

pub(crate) struct MicrophoneLevel {
    kind: MicrophoneLevelKind,
    active: bool,
    level_db: f64,
    fill: f32,
    color: Hsla,
    track: Hsla,
}

impl MicrophoneLevel {
    pub(crate) fn new(
        kind: MicrophoneLevelKind,
        level: &Entity<MicLevel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe_in(level, window, |this, level, window, cx| {
            this.level_db = level.read(cx).db();
            if !this.active || !this.kind.shown(window) {
                return;
            }
            let fill = this.kind.fill(this.level_db);
            let span = this.kind.span(
                f32::from(window.viewport_size().width),
                window.scale_factor(),
            );
            if moves_a_device_pixel(this.fill, fill, span) {
                this.fill = fill;
                cx.notify();
                if this.kind == MicrophoneLevelKind::RecordingBar {
                    window.refresh();
                }
            }
        })
        .detach();
        Self {
            kind,
            active: false,
            level_db: level.read(cx).db(),
            fill: 0.,
            color: gpui::transparent_black(),
            track: gpui::transparent_black(),
        }
    }

    pub(crate) fn capture_fill(cx: &App) -> f32 {
        let feeds = Feeds::global(cx).read(cx);
        if feeds.microphone.is_some() {
            MicrophoneLevelKind::CaptureButton.fill(feeds.mic_level().read(cx).db())
        } else {
            0.
        }
    }

    pub(crate) fn configure(
        &mut self,
        active: bool,
        color: Hsla,
        track: Hsla,
        cx: &mut Context<Self>,
    ) {
        let fill = if active {
            self.kind.fill(self.level_db)
        } else {
            0.
        };
        if self.active != active || self.color != color || self.track != track || self.fill != fill
        {
            self.active = active;
            self.color = color;
            self.track = track;
            self.fill = fill;
            cx.notify();
        }
    }

    pub(crate) fn snapshot(fill: f32, color: Hsla) -> gpui::Div {
        let mut background = color;
        background.a *= 0.14;
        div().size_full().when(fill > 0., |this| {
            this.child(
                div()
                    .h_full()
                    .w(gpui::relative(fill))
                    .rounded(px(10.))
                    .bg(background),
            )
        })
    }
}

impl Render for MicrophoneLevel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        match self.kind {
            MicrophoneLevelKind::CaptureButton => Self::snapshot(self.fill, self.color),
            MicrophoneLevelKind::DeviceRow => div()
                .mt(px(3.))
                .h(px(3.))
                .w_full()
                .rounded_full()
                .overflow_hidden()
                .bg(self.track)
                .child(
                    div()
                        .h_full()
                        .rounded_full()
                        .bg(self.color)
                        .w(gpui::relative(self.fill)),
                ),
            MicrophoneLevelKind::RecordingBar => div()
                .absolute()
                .bottom(px(4.))
                .left(px(4.))
                .right(px(4.))
                .h(px(2.))
                .rounded_full()
                .overflow_hidden()
                .bg(self.track)
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .bottom_0()
                        .w(gpui::relative(self.fill))
                        .bg(self.color),
                ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px_moved(kind: MicrophoneLevelKind, from_db: f64, to_db: f64, span: f32) -> bool {
        moves_a_device_pixel(kind.fill(from_db), kind.fill(to_db), span)
    }

    #[test]
    fn a_level_change_of_a_device_pixel_or_more_repaints() {
        let span = MicrophoneLevelKind::CaptureButton.span(320., 2.);
        assert!(px_moved(
            MicrophoneLevelKind::CaptureButton,
            -30.,
            -20.,
            span
        ));
        assert!(px_moved(
            MicrophoneLevelKind::CaptureButton,
            -96.,
            -39.,
            span
        ));
        assert!(px_moved(MicrophoneLevelKind::DeviceRow, -12., -10., span));
        assert!(moves_a_device_pixel(0.5, 0.5 + 1. / span, span));
    }

    #[test]
    fn a_sub_pixel_level_change_does_not_repaint() {
        let span = MicrophoneLevelKind::CaptureButton.span(320., 2.);
        assert!(!moves_a_device_pixel(0.5, 0.5 + 0.4 / span, span));
        assert!(!moves_a_device_pixel(0.25, 0.25, span));
        assert!(!px_moved(
            MicrophoneLevelKind::CaptureButton,
            -20.,
            -20.25,
            40.
        ));
    }

    #[test]
    fn levels_below_the_visible_range_never_repaint() {
        let span = MicrophoneLevelKind::CaptureButton.span(4096., 3.);
        for db in [-96., -72., -60., -55.5, -48., -40.5, -40.] {
            assert!(!px_moved(
                MicrophoneLevelKind::CaptureButton,
                -96.,
                db,
                span
            ));
            assert!(!px_moved(MicrophoneLevelKind::DeviceRow, -96., db, span));
        }
        let span = MicrophoneLevelKind::RecordingBar.span(0., 2.);
        for db in [-96., -80., -60.5, -60.] {
            assert!(!px_moved(MicrophoneLevelKind::RecordingBar, -96., db, span));
        }
        assert!(px_moved(
            MicrophoneLevelKind::RecordingBar,
            -96.,
            -58.,
            span
        ));
    }

    #[test]
    fn the_recording_bar_repaints_per_device_pixel_of_its_track() {
        let kind = MicrophoneLevelKind::RecordingBar;
        assert!(!px_moved(kind, -30., -30.5, kind.span(0., 2.)));
        assert!(px_moved(kind, -30., -32., kind.span(0., 2.)));
        assert!(!px_moved(kind, -30., -31., kind.span(0., 1.)));
        assert!(px_moved(kind, -30., -33., kind.span(0., 1.)));
        assert_eq!(kind.fill(-96.) * kind.span(0., 2.), 0.);
        assert_eq!(kind.fill(0.) * kind.span(0., 1.), 20.);
        assert_eq!(kind.fill(0.) * kind.span(0., 2.), 40.);
    }

    #[test]
    fn fills_match_the_existing_level_mappings() {
        assert_eq!(MicrophoneLevelKind::CaptureButton.fill(-96.), 0.);
        assert_eq!(MicrophoneLevelKind::CaptureButton.fill(0.), 1.);
        assert_eq!(MicrophoneLevelKind::DeviceRow.fill(-40.), 0.);
        assert_eq!(MicrophoneLevelKind::RecordingBar.fill(-60.), 0.);
        assert_eq!(MicrophoneLevelKind::RecordingBar.fill(-30.), 0.5);
        assert_eq!(
            MicrophoneLevelKind::CaptureButton.fill(f64::NEG_INFINITY),
            0.
        );
    }

    #[test]
    #[ignore = "benchmark"]
    fn bench_meter_repaints_per_thousand_level_samples() {
        let mut state = 0x2545_f491_4f6c_dd1du64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 11) as f64 / (1u64 << 53) as f64
        };
        let mut db = -45.0f64;
        let levels: Vec<f64> = (0..1000)
            .map(|_| {
                db = (db + (next() - 0.5) * 14.0).clamp(-96.0, -6.0);
                (db * 2.0).round() / 2.0
            })
            .collect();

        for (name, kind, span) in [
            (
                "capture button, 720 px",
                MicrophoneLevelKind::CaptureButton,
                720.,
            ),
            ("device row, 720 px", MicrophoneLevelKind::DeviceRow, 720.),
            (
                "recording bar, 40 px",
                MicrophoneLevelKind::RecordingBar,
                40.,
            ),
        ] {
            let mut by_level = 0;
            let mut by_pixel = 0;
            let mut previous = levels[0];
            let mut drawn = kind.fill(previous);
            for &level in &levels[1..] {
                by_level += usize::from(level != previous);
                previous = level;
                let fill = kind.fill(level.max(-60.0));
                if moves_a_device_pixel(drawn, fill, span) {
                    by_pixel += 1;
                    drawn = fill;
                }
            }
            println!(
                "{name}: repaints per 1000 samples, on every level change {by_level}, on a device pixel {by_pixel}"
            );
        }
    }
}
