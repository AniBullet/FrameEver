use frameever_project::{
    ProjectConfiguration, TimelineSegment, VoiceDeclick, VoiceEq, VoiceEqBandKind, VoiceEqPreset,
};
use gpui::{
    AnyElement, Bounds, Context, CursorStyle, Hsla, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, ParentElement, PathBuilder, PathStyle, Pixels, Point,
    ScrollEvent, SharedString, StatefulInteractiveElement, Styled, Window, canvas, div, point,
    prelude::FluentBuilder, px,
};
use lyon_tessellation::{LineCap, LineJoin, StrokeOptions};

use crate::{
    editor_edits::set_clip_voice_eq,
    editor_sidebar::SliderKey,
    editor_timeline::TrackKind,
    editor_window::EditorWindow,
    i18n::{t, tc},
    ui,
};

const MIN_HZ: f32 = 20.;
const MAX_HZ: f32 = 20_000.;
const PLOT_DB: f32 = 15.;
const GRAPH_HEIGHT: f32 = 150.;
const LABEL_STRIP: f32 = 22.;
const HANDLE: f32 = 10.;
const HIT_RADIUS: f32 = 14.;
const CURVE_POINTS: usize = 160;
const GRID_HZ: [f32; 3] = [100., 1_000., 10_000.];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VoiceEqHandle {
    HighPass,
    Band(VoiceEqBandKind),
}

impl VoiceEqHandle {
    pub const ALL: [Self; 5] = [
        Self::HighPass,
        Self::Band(VoiceEqBandKind::Low),
        Self::Band(VoiceEqBandKind::LowMid),
        Self::Band(VoiceEqBandKind::HighMid),
        Self::Band(VoiceEqBandKind::High),
    ];

    fn label(self) -> &'static str {
        match self {
            Self::HighPass => t!("Low cut"),
            Self::Band(VoiceEqBandKind::Low) => tc!("eq band", "Low"),
            Self::Band(VoiceEqBandKind::LowMid) => t!("Low-mid"),
            Self::Band(VoiceEqBandKind::HighMid) => t!("High-mid"),
            Self::Band(VoiceEqBandKind::High) => tc!("eq band", "High"),
        }
    }

    fn hz_range(self) -> (f32, f32) {
        match self {
            Self::HighPass => VoiceEq::HIGH_PASS_RANGE,
            Self::Band(kind) => kind.hz_range(),
        }
    }

    fn hz(self, voice_eq: &VoiceEq) -> f32 {
        match self {
            Self::HighPass => voice_eq.high_pass_hz,
            Self::Band(kind) => voice_eq.band(kind).hz,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VoiceEqSlider {
    HighPassHz,
    Hz(VoiceEqBandKind),
    Gain(VoiceEqBandKind),
    Q(VoiceEqBandKind),
}

impl VoiceEqSlider {
    pub fn limits(self) -> (f32, f32, f32) {
        match self {
            Self::HighPassHz => {
                let (min, max) = VoiceEq::HIGH_PASS_RANGE;
                (min.log2(), max.log2(), 0.01)
            }
            Self::Hz(kind) => {
                let (min, max) = kind.hz_range();
                (min.log2(), max.log2(), 0.01)
            }
            Self::Gain(_) => (VoiceEq::GAIN_RANGE.0, VoiceEq::GAIN_RANGE.1, 0.5),
            Self::Q(_) => (VoiceEq::Q_RANGE.0, VoiceEq::Q_RANGE.1, 0.1),
        }
    }

    pub fn unit(self) -> &'static str {
        match self {
            Self::HighPassHz | Self::Hz(_) => "log2hz",
            Self::Gain(_) => "sdb",
            Self::Q(_) => "q",
        }
    }

    pub fn read(self, voice_eq: &VoiceEq) -> f32 {
        match self {
            Self::HighPassHz => voice_eq.high_pass_hz.log2(),
            Self::Hz(kind) => voice_eq.band(kind).hz.log2(),
            Self::Gain(kind) => voice_eq.band(kind).gain_db,
            Self::Q(kind) => voice_eq.band(kind).q,
        }
    }

    pub fn write(self, voice_eq: &mut VoiceEq, value: f32) {
        match self {
            Self::HighPassHz => {
                voice_eq.high_pass = true;
                voice_eq.high_pass_hz = value.exp2();
            }
            Self::Hz(kind) => voice_eq.band_mut(kind).hz = value.exp2(),
            Self::Gain(kind) => voice_eq.band_mut(kind).gain_db = value,
            Self::Q(kind) => voice_eq.band_mut(kind).q = value,
        }
    }
}

pub struct VoiceEqView {
    pub selected: VoiceEqHandle,
    pub hover: Option<VoiceEqHandle>,
    pub drag: Option<(VoiceEqHandle, Bounds<Pixels>)>,
    plot: ui::SliderTrack,
}

impl Default for VoiceEqView {
    fn default() -> Self {
        Self {
            selected: VoiceEqHandle::Band(VoiceEqBandKind::Low),
            hover: None,
            drag: None,
            plot: ui::SliderTrack::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetTile {
    Off,
    Preset(VoiceEqPreset),
}

impl PresetTile {
    pub const ALL: [Self; 6] = [
        Self::Off,
        Self::Preset(VoiceEqPreset::Natural),
        Self::Preset(VoiceEqPreset::Clear),
        Self::Preset(VoiceEqPreset::Warm),
        Self::Preset(VoiceEqPreset::Bright),
        Self::Preset(VoiceEqPreset::Broadcast),
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Off => t!("Off"),
            Self::Preset(preset) => preset_label(preset),
        }
    }

    pub fn applied_to(self, shown: Option<VoiceEq>) -> Option<VoiceEq> {
        match self {
            Self::Off => None,
            Self::Preset(preset) => Some(VoiceEq {
                enabled: true,
                ..shown.unwrap_or_default().with_preset(preset)
            }),
        }
    }
}

pub fn voice_eq_is_off(shown: Option<VoiceEq>) -> bool {
    shown.is_none_or(|voice_eq| !voice_eq.enabled)
}

pub fn selected_tile(shown: Option<VoiceEq>) -> Option<PresetTile> {
    match shown {
        _ if voice_eq_is_off(shown) => Some(PresetTile::Off),
        Some(VoiceEq {
            preset: VoiceEqPreset::Custom,
            ..
        }) => None,
        Some(voice_eq) => Some(PresetTile::Preset(voice_eq.preset)),
        None => Some(PresetTile::Off),
    }
}

pub fn reset_handle(voice_eq: &mut VoiceEq, handle: VoiceEqHandle, preset: VoiceEqPreset) {
    let reference = VoiceEq::from_preset(preset);
    match handle {
        VoiceEqHandle::HighPass => {
            voice_eq.high_pass = true;
            voice_eq.high_pass_hz = reference.high_pass_hz;
        }
        VoiceEqHandle::Band(kind) => *voice_eq.band_mut(kind) = reference.band(kind),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoiceEqScope {
    AllClips,
    Mixed,
    Clip(usize),
    Clips(usize),
}

impl VoiceEqScope {
    pub fn of(targets: &[usize], clip_count: usize, mixed: bool) -> Self {
        match targets {
            [index] if clip_count > 1 => Self::Clip(*index),
            targets if targets.len() < clip_count => Self::Clips(targets.len()),
            _ if mixed => Self::Mixed,
            _ => Self::AllClips,
        }
    }

    fn label(&self) -> String {
        match self {
            Self::AllClips => t!("All clips").to_string(),
            Self::Mixed => tc!("voice eq scope", "Mixed").to_string(),
            Self::Clip(index) => t!("Clip {number}", number = index + 1),
            Self::Clips(count) => t!("{count} clips", count = count),
        }
    }

    fn is_selection(&self) -> bool {
        matches!(self, Self::Clip(_) | Self::Clips(_))
    }
}

pub fn hz_to_fraction(hz: f32) -> f32 {
    ((hz.max(MIN_HZ) / MIN_HZ).log10() / (MAX_HZ / MIN_HZ).log10()).clamp(0., 1.)
}

pub fn fraction_to_hz(fraction: f32) -> f32 {
    MIN_HZ * (MAX_HZ / MIN_HZ).powf(fraction.clamp(0., 1.))
}

pub fn db_to_fraction(db: f32) -> f32 {
    (0.5 - db / (2. * PLOT_DB)).clamp(0., 1.)
}

pub fn fraction_to_db(fraction: f32) -> f32 {
    (0.5 - fraction) * 2. * PLOT_DB
}

pub fn format_hz(hz: f32) -> String {
    if hz.round() >= 1_000. {
        let khz = (hz / 100.).round() / 10.;
        if khz.fract() == 0. {
            format!("{khz:.0} kHz")
        } else {
            format!("{khz:.1} kHz")
        }
    } else {
        format!("{:.0} Hz", hz.round())
    }
}

pub fn format_gain(db: f32) -> String {
    let db = (db * 10.).round() / 10.;
    if db == 0. {
        "0.0 dB".into()
    } else {
        format!(
            "{}{:.1} dB",
            if db > 0. { "+" } else { "\u{2212}" },
            db.abs()
        )
    }
}

fn preset_label(preset: VoiceEqPreset) -> &'static str {
    match preset {
        VoiceEqPreset::Natural => t!("Natural"),
        VoiceEqPreset::Clear => tc!("voice eq preset", "Clear"),
        VoiceEqPreset::Warm => t!("Warm"),
        VoiceEqPreset::Bright => t!("Bright"),
        VoiceEqPreset::Broadcast => t!("Broadcast"),
        VoiceEqPreset::Custom => t!("Custom"),
    }
}

pub fn voice_eq_summary(segment: &TimelineSegment) -> String {
    let Some(voice_eq) = segment.voice_eq else {
        return t!("Off").to_string();
    };
    let tone = voice_eq
        .shapes_tone()
        .then(|| preset_label(voice_eq.preset));
    let declick = (voice_eq.declick != VoiceDeclick::Off).then(|| t!("De-click"));
    match (tone, declick) {
        (Some(tone), Some(declick)) => format!("{tone} \u{b7} {declick}"),
        (Some(tone), None) => tone.to_string(),
        (None, Some(declick)) => declick.to_string(),
        (None, None) => t!("Off").to_string(),
    }
}

fn response(voice_eq: &VoiceEq) -> frameever_audio::ToneResponse {
    frameever_audio::VoiceToneSettings {
        filters: frameever_editor::voice_eq_filters(voice_eq),
        declick: None,
    }
    .response()
}

pub fn handle_position(voice_eq: &VoiceEq, handle: VoiceEqHandle) -> (f32, f32) {
    let x = hz_to_fraction(handle.hz(voice_eq));
    let db = match handle {
        VoiceEqHandle::HighPass if voice_eq.high_pass => {
            response(voice_eq).db(voice_eq.high_pass_hz)
        }
        VoiceEqHandle::HighPass => 0.,
        VoiceEqHandle::Band(kind) => voice_eq.band(kind).gain_db,
    };
    (x, db_to_fraction(db))
}

pub fn hit_test(
    voice_eq: &VoiceEq,
    bounds: Bounds<Pixels>,
    position: Point<Pixels>,
    selected: VoiceEqHandle,
) -> Option<VoiceEqHandle> {
    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    if width <= 0. || height <= 0. {
        return None;
    }
    let local_x = f32::from(position.x - bounds.origin.x);
    let local_y = f32::from(position.y - bounds.origin.y);
    VoiceEqHandle::ALL
        .into_iter()
        .map(|handle| {
            let (x, y) = handle_position(voice_eq, handle);
            let distance = (x * width - local_x).hypot(y * height - local_y);
            let bias = if handle == selected { -0.5 } else { 0. };
            (handle, distance + bias)
        })
        .filter(|(_, distance)| *distance <= HIT_RADIUS)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(handle, _)| handle)
}

pub fn drag_to(
    voice_eq: &mut VoiceEq,
    handle: VoiceEqHandle,
    bounds: Bounds<Pixels>,
    position: Point<Pixels>,
) -> bool {
    let Some((x, y)) = ui::pad_position(position, bounds) else {
        return false;
    };
    let (min, max) = handle.hz_range();
    let hz = fraction_to_hz(x as f32).clamp(min, max);
    let hz = if hz >= 1_000. {
        (hz / 10.).round() * 10.
    } else {
        hz.round()
    };
    let gain = (fraction_to_db(y as f32).clamp(VoiceEq::GAIN_RANGE.0, VoiceEq::GAIN_RANGE.1) * 2.)
        .round()
        / 2.;
    let before = *voice_eq;
    match handle {
        VoiceEqHandle::HighPass => {
            voice_eq.high_pass = true;
            voice_eq.high_pass_hz = hz;
        }
        VoiceEqHandle::Band(kind) => {
            let band = voice_eq.band_mut(kind);
            band.hz = hz;
            band.gain_db = gain;
        }
    }
    *voice_eq != before
}

pub fn widen(voice_eq: &mut VoiceEq, kind: VoiceEqBandKind, delta: f32) -> bool {
    if kind.is_shelf() || delta == 0. {
        return false;
    }
    let band = voice_eq.band_mut(kind);
    let before = band.q;
    let next = band.q * (1.0 + delta / 400.).clamp(0.8, 1.25);
    band.q = (next.clamp(VoiceEq::Q_RANGE.0, VoiceEq::Q_RANGE.1) * 100.).round() / 100.;
    band.q != before
}

fn segment_count(project: &ProjectConfiguration) -> usize {
    project
        .timeline
        .as_ref()
        .map_or(0, |timeline| timeline.segments.len())
}

impl EditorWindow {
    pub(crate) fn voice_eq_targets(&self) -> Vec<usize> {
        let count = segment_count(&self.project);
        let mut selected = self
            .selection
            .as_ref()
            .filter(|selection| selection.track == TrackKind::Clip)
            .map(|selection| {
                selection
                    .indices
                    .iter()
                    .copied()
                    .filter(|index| *index < count)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        selected.sort_unstable();
        selected.dedup();
        if selected.is_empty() {
            (0..count).collect()
        } else {
            selected
        }
    }

    pub(crate) fn shown_voice_eq(&self) -> Option<VoiceEq> {
        let timeline = self.project.timeline.as_ref()?;
        let first = *self.voice_eq_targets().first()?;
        timeline.segments.get(first)?.voice_eq
    }

    fn voice_eq_mixed(&self) -> bool {
        let Some(timeline) = self.project.timeline.as_ref() else {
            return false;
        };
        let mut settings = self
            .voice_eq_targets()
            .into_iter()
            .filter_map(|index| timeline.segments.get(index))
            .map(|segment| segment.voice_eq);
        let Some(first) = settings.next() else {
            return false;
        };
        settings.any(|other| other != first)
    }

    fn voice_eq_scope(&self) -> VoiceEqScope {
        VoiceEqScope::of(
            &self.voice_eq_targets(),
            segment_count(&self.project),
            self.voice_eq_mixed(),
        )
    }

    fn write_voice_eq(
        &mut self,
        reason: &'static str,
        voice_eq: Option<VoiceEq>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let targets = self.voice_eq_targets();
        self.edit_project(reason, window, cx, move |project| {
            project
                .timeline
                .as_mut()
                .is_some_and(|timeline| set_clip_voice_eq(timeline, &targets, voice_eq))
        });
    }

    fn edit_voice_eq_tone(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut VoiceEq),
    ) {
        let mut next = self.shown_voice_eq().unwrap_or_default().sanitized();
        change(&mut next);
        next.enabled = true;
        next.preset = VoiceEqPreset::Custom;
        self.write_voice_eq("voice-eq", Some(next), window, cx);
    }

    pub(crate) fn apply_voice_eq_slider(
        &mut self,
        slider: VoiceEqSlider,
        value: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit_voice_eq_tone(window, cx, |voice_eq| slider.write(voice_eq, value));
    }

    pub(crate) fn voice_eq_slider_value(&self, slider: VoiceEqSlider) -> f32 {
        slider.read(&self.shown_voice_eq().unwrap_or_default().sanitized())
    }

    fn apply_voice_eq_to_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let shown = self.shown_voice_eq();
        let all = (0..segment_count(&self.project)).collect::<Vec<_>>();
        self.edit_project("voice-eq-all", window, cx, move |project| {
            project
                .timeline
                .as_mut()
                .is_some_and(|timeline| set_clip_voice_eq(timeline, &all, shown))
        });
    }

    pub(crate) fn voice_eq_dragging(&self) -> bool {
        self.sidebar.voice_eq.drag.is_some()
    }

    fn voice_eq_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(bounds) = self.sidebar.voice_eq.plot.get() else {
            return;
        };
        let shown = self.shown_voice_eq().unwrap_or_default().sanitized();
        let Some(handle) = hit_test(
            &shown,
            bounds,
            event.position,
            self.sidebar.voice_eq.selected,
        ) else {
            return;
        };
        cx.stop_propagation();
        self.sidebar.voice_eq.selected = handle;
        if event.click_count >= 2 {
            let preset = match shown.preset {
                VoiceEqPreset::Custom => VoiceEqPreset::Natural,
                preset => preset,
            };
            self.edit_voice_eq_tone(window, cx, |voice_eq| {
                reset_handle(voice_eq, handle, preset)
            });
            return;
        }
        self.history.pause();
        self.sidebar.voice_eq.drag = Some((handle, bounds));
        cx.notify();
    }

    pub(crate) fn voice_eq_mouse_move(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((handle, bounds)) = self.sidebar.voice_eq.drag else {
            return;
        };
        let mut next = self.shown_voice_eq().unwrap_or_default().sanitized();
        if !drag_to(&mut next, handle, bounds, position) {
            return;
        }
        self.edit_voice_eq_tone(window, cx, move |voice_eq| *voice_eq = next);
    }

    pub(crate) fn voice_eq_mouse_up(&mut self, cx: &mut Context<Self>) {
        if self.sidebar.voice_eq.drag.take().is_some() {
            let config = self.project.clone();
            self.history.resume(&config);
            cx.notify();
        }
    }

    fn voice_eq_hover_at(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let hover = self.sidebar.voice_eq.plot.get().and_then(|bounds| {
            let shown = self.shown_voice_eq().unwrap_or_default().sanitized();
            hit_test(&shown, bounds, position, self.sidebar.voice_eq.selected)
        });
        if hover != self.sidebar.voice_eq.hover {
            self.sidebar.voice_eq.hover = hover;
            cx.notify();
        }
    }

    fn voice_eq_scroll(
        &mut self,
        event: &ScrollEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(VoiceEqHandle::Band(kind)) = self.sidebar.voice_eq.hover else {
            return;
        };
        let delta = f32::from(event.delta.pixel_delta(window.line_height()).y);
        let mut next = self.shown_voice_eq().unwrap_or_default().sanitized();
        if !widen(&mut next, kind, delta) {
            return;
        }
        cx.stop_propagation();
        self.sidebar.voice_eq.selected = VoiceEqHandle::Band(kind);
        self.edit_voice_eq_tone(window, cx, move |voice_eq| *voice_eq = next);
    }

    pub(crate) fn open_clip_voice_eq(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.sidebar.tab != crate::editor_sidebar::SidebarTab::Audio
            && let Some(position) = crate::editor_sidebar::SidebarTab::ALL
                .iter()
                .position(|tab| *tab == crate::editor_sidebar::SidebarTab::Audio)
        {
            self.select_sidebar_tab(position, window, cx);
        }
        self.set_selection(
            Some(crate::editor_edits::Selection::single(
                TrackKind::Clip,
                index,
            )),
            cx,
        );
        cx.notify();
    }

    pub(crate) fn render_voice_eq_group(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.theme;
        let shown = self.shown_voice_eq();
        let voice_eq = shown.unwrap_or_default().sanitized();
        let off = voice_eq_is_off(shown);
        let scope = self.voice_eq_scope();

        ui::Field::section(&theme, t!("Voice EQ"))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(self.render_voice_eq_scope(&scope, shown, cx))
                    .child(self.render_voice_eq_presets(shown, cx))
                    .when(!off, |group| {
                        group
                            .child(self.render_voice_eq_graph(&voice_eq, cx))
                            .child(self.render_voice_eq_bands(cx))
                            .child(self.render_voice_eq_band_sliders(cx))
                            .child(self.render_voice_eq_declick(voice_eq.declick, cx))
                    })
                    .when(scope.is_selection(), |group| {
                        group.child(
                            ui::Button::plain(
                                &theme,
                                "voice-eq-apply-all",
                                ui::ButtonVariant::Gray,
                                ui::ButtonSize::Md,
                            )
                            .label(t!("Apply to all"))
                            .on_click(cx.listener(
                                |this, _, window, cx| {
                                    this.apply_voice_eq_to_all(window, cx);
                                },
                            )),
                        )
                    }),
            )
            .into_any_element()
    }

    fn render_voice_eq_scope(
        &self,
        scope: &VoiceEqScope,
        shown: Option<VoiceEq>,
        _cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let custom = selected_tile(shown).is_none();
        let tag = |label: String| {
            div()
                .px(px(8.))
                .h(px(22.))
                .flex()
                .items_center()
                .rounded(px(6.))
                .bg(Hsla::from(theme.muted))
                .text_size(px(12.))
                .text_color(Hsla::from(theme.foreground))
                .child(label)
        };
        ui::Field::editor_row(&theme, t!("Applies to"))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(6.))
                    .justify_end()
                    .child(tag(scope.label()))
                    .when(custom, |row| row.child(tag(t!("Custom").to_string()))),
            )
            .into_any_element()
    }

    fn render_voice_eq_presets(
        &self,
        shown: Option<VoiceEq>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let selected = selected_tile(shown);
        let tile = |tile: PresetTile| {
            let on = selected == Some(tile);
            div()
                .id(SharedString::from(format!("voice-eq-preset-{tile:?}")))
                .role(gpui::Role::RadioButton)
                .aria_label(tile.label())
                .aria_toggled(if on {
                    gpui::Toggled::True
                } else {
                    gpui::Toggled::False
                })
                .flex()
                .items_center()
                .justify_center()
                .min_h(px(44.))
                .px(px(4.))
                .rounded(px(6.))
                .text_size(px(12.))
                .cursor_pointer()
                .map(|this| {
                    if on {
                        this.bg(Hsla::from(theme.selection))
                            .border_2()
                            .border_color(Hsla::from(theme.foreground))
                            .text_color(Hsla::from(theme.foreground))
                    } else {
                        this.bg(Hsla::from(theme.muted))
                            .text_color(Hsla::from(theme.muted_foreground))
                            .hover(|this| this.text_color(Hsla::from(theme.foreground)))
                    }
                })
                .child(tile.label())
                .on_click(cx.listener(move |this, _, window, cx| {
                    let next = tile.applied_to(this.shown_voice_eq());
                    this.write_voice_eq("voice-eq-preset", next, window, cx);
                }))
        };
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .children(PresetTile::ALL.chunks(3).map(|row| {
                div().flex().flex_row().gap(px(6.)).children(
                    row.iter()
                        .map(|value| div().flex_1().min_w_0().child(tile(*value))),
                )
            }))
            .into_any_element()
    }

    fn render_voice_eq_graph(&self, voice_eq: &VoiceEq, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.theme;
        let view = &self.sidebar.voice_eq;
        let plot_cell = view.plot.clone();
        let selected = view.selected;
        let active = view.drag.map(|(handle, _)| handle).or(view.hover);
        let total = response(voice_eq);
        let curve = (0..CURVE_POINTS)
            .map(|index| {
                let fraction = index as f32 / (CURVE_POINTS - 1) as f32;
                (fraction, db_to_fraction(total.db(fraction_to_hz(fraction))))
            })
            .collect::<Vec<_>>();
        let colors = PlotColors {
            grid: Hsla::from(theme.border),
            zero: Hsla::from(theme.border_strong),
            curve: Hsla::from(theme.foreground),
        };

        let handles = VoiceEqHandle::ALL.into_iter().map(|handle| {
            let (x, y) = handle_position(voice_eq, handle);
            let on = handle == selected || active == Some(handle);
            div()
                .absolute()
                .left(gpui::relative(x))
                .top(gpui::relative(y))
                .ml(px(-HANDLE / 2.))
                .mt(px(-HANDLE / 2.))
                .size(px(HANDLE))
                .rounded_full()
                .border_1()
                .border_color(Hsla::from(theme.foreground))
                .bg(if on {
                    Hsla::from(theme.foreground)
                } else {
                    Hsla::from(theme.card)
                })
        });

        let readout = active.map(|handle| {
            let (x, y) = handle_position(voice_eq, handle);
            let text = match handle {
                VoiceEqHandle::HighPass => {
                    format!(
                        "{} \u{b7} {}",
                        handle.label(),
                        format_hz(voice_eq.high_pass_hz)
                    )
                }
                VoiceEqHandle::Band(kind) => {
                    let band = voice_eq.band(kind);
                    format!(
                        "{} \u{b7} {} \u{b7} {}",
                        handle.label(),
                        format_hz(band.hz),
                        format_gain(band.gain_db)
                    )
                }
            };
            let above = y > 0.32;
            let leftward = x > 0.5;
            div()
                .absolute()
                .top(gpui::relative(y))
                .mt(px(if above { -30. } else { 10. }))
                .when(leftward, |pill| {
                    pill.right(gpui::relative(1. - x)).mr(px(-8.))
                })
                .when(!leftward, |pill| pill.left(gpui::relative(x)).ml(px(-8.)))
                .px(px(6.))
                .py(px(2.))
                .rounded(px(4.))
                .bg(Hsla::from(theme.popover))
                .border_1()
                .border_color(Hsla::from(theme.border))
                .text_color(Hsla::from(theme.popover_foreground))
                .text_size(px(11.))
                .whitespace_nowrap()
                .child(text)
        });

        let labels = GRID_HZ
            .into_iter()
            .zip(["100", "1k", "10k"])
            .map(|(hz, text)| {
                div()
                    .absolute()
                    .bottom(px(4.))
                    .left(gpui::relative(hz_to_fraction(hz)))
                    .ml(px(-16.))
                    .w(px(32.))
                    .flex()
                    .justify_center()
                    .text_size(px(10.))
                    .text_color(Hsla::from(theme.muted_foreground))
                    .child(text)
            });

        let plot = div()
            .id("voice-eq-plot")
            .absolute()
            .top(px(6.))
            .left_0()
            .right_0()
            .bottom(px(LABEL_STRIP))
            .cursor(if self.sidebar.voice_eq.drag.is_some() {
                CursorStyle::ClosedHand
            } else if self.sidebar.voice_eq.hover.is_some() {
                CursorStyle::OpenHand
            } else {
                CursorStyle::Arrow
            })
            .child(
                canvas(
                    move |bounds, _, _| {
                        plot_cell.set(Some(bounds));
                        bounds
                    },
                    move |_, bounds, window, _| paint_plot(window, bounds, &curve, colors),
                )
                .absolute()
                .inset_0(),
            )
            .children(handles)
            .children(readout)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    this.voice_eq_mouse_down(event, window, cx);
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                if !this.voice_eq_dragging() {
                    this.voice_eq_hover_at(event.position, cx);
                }
            }))
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                if !*hovered && this.sidebar.voice_eq.hover.take().is_some() {
                    cx.notify();
                }
            }))
            .on_scroll(cx.listener(|this, event: &ScrollEvent, window, cx| {
                this.voice_eq_scroll(event, window, cx);
            }));

        div()
            .relative()
            .w_full()
            .h(px(GRAPH_HEIGHT))
            .rounded(px(6.))
            .bg(Hsla::from(theme.card))
            .overflow_hidden()
            .child(plot)
            .children(labels)
            .into_any_element()
    }

    fn render_voice_eq_bands(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.theme;
        let selected = self.sidebar.voice_eq.selected;
        ui::SegmentedControl::editor(
            &theme,
            "voice-eq-band",
            VoiceEqHandle::ALL
                .iter()
                .map(|handle| ui::SegmentOption::new(handle.label(), *handle == selected))
                .collect(),
        )
        .stretch()
        .on_select(cx.listener(|this, index: &usize, _, cx| {
            if let Some(handle) = VoiceEqHandle::ALL.get(*index) {
                this.sidebar.voice_eq.selected = *handle;
                cx.notify();
            }
        }))
        .into_any_element()
    }

    fn render_voice_eq_band_sliders(&self, cx: &mut Context<Self>) -> AnyElement {
        let rows: Vec<AnyElement> = match self.sidebar.voice_eq.selected {
            VoiceEqHandle::HighPass => vec![
                self.slider_field(
                    t!("Frequency"),
                    SliderKey::VoiceEq(VoiceEqSlider::HighPassHz),
                    VoiceEqSlider::HighPassHz.unit(),
                    cx,
                )
                .into_any_element(),
            ],
            VoiceEqHandle::Band(kind) => {
                let mut rows = vec![
                    self.slider_field(
                        t!("Frequency"),
                        SliderKey::VoiceEq(VoiceEqSlider::Hz(kind)),
                        VoiceEqSlider::Hz(kind).unit(),
                        cx,
                    )
                    .into_any_element(),
                    self.slider_field(
                        t!("Gain"),
                        SliderKey::VoiceEq(VoiceEqSlider::Gain(kind)),
                        VoiceEqSlider::Gain(kind).unit(),
                        cx,
                    )
                    .into_any_element(),
                ];
                if !kind.is_shelf() {
                    rows.push(
                        self.slider_field(
                            "Q",
                            SliderKey::VoiceEq(VoiceEqSlider::Q(kind)),
                            VoiceEqSlider::Q(kind).unit(),
                            cx,
                        )
                        .into_any_element(),
                    );
                }
                rows
            }
        };
        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .children(rows)
            .into_any_element()
    }

    fn render_voice_eq_declick(&self, current: VoiceDeclick, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.theme;
        let options = [
            (VoiceDeclick::Off, t!("Off")),
            (VoiceDeclick::Light, tc!("strength", "Light")),
            (VoiceDeclick::Strong, t!("Strong")),
        ];
        ui::Field::editor_row(&theme, t!("De-click"))
            .child(
                div().w(px(180.)).child(
                    ui::SegmentedControl::editor(
                        &theme,
                        "voice-eq-declick",
                        options
                            .iter()
                            .map(|(value, label)| ui::SegmentOption::new(*label, current == *value))
                            .collect(),
                    )
                    .stretch()
                    .on_select(cx.listener(
                        move |this, index: &usize, window, cx| {
                            let Some(&(declick, _)) = options.get(*index) else {
                                return;
                            };
                            if declick == current {
                                return;
                            }
                            let mut next = this.shown_voice_eq().unwrap_or_default();
                            next.declick = declick;
                            this.write_voice_eq("voice-eq-declick", Some(next), window, cx);
                        },
                    )),
                ),
            )
            .into_any_element()
    }
}

#[derive(Clone, Copy)]
struct PlotColors {
    grid: Hsla,
    zero: Hsla,
    curve: Hsla,
}

fn paint_plot(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    curve: &[(f32, f32)],
    colors: PlotColors,
) {
    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    if width <= 0. || height <= 0. || curve.len() < 2 {
        return;
    }
    let at = |x: f32, y: f32| {
        point(
            bounds.origin.x + px(x * width),
            bounds.origin.y + px(y * height),
        )
    };

    let mut grid = PathBuilder::stroke(px(1.));
    for hz in GRID_HZ {
        let x = hz_to_fraction(hz);
        grid.move_to(at(x, 0.));
        grid.line_to(at(x, 1.));
    }
    grid.move_to(at(0., 0.));
    grid.line_to(at(1., 0.));
    grid.move_to(at(0., 1.));
    grid.line_to(at(1., 1.));
    if let Ok(path) = grid.build() {
        window.paint_path(path, colors.grid);
    }
    let mut zero = PathBuilder::stroke(px(1.));
    zero.move_to(at(0., db_to_fraction(0.)));
    zero.line_to(at(1., db_to_fraction(0.)));
    if let Ok(path) = zero.build() {
        window.paint_path(path, colors.zero);
    }

    let mut stroke = PathBuilder::stroke(px(1.5)).with_style(PathStyle::Stroke(
        StrokeOptions::default()
            .with_line_width(1.5)
            .with_line_cap(LineCap::Round)
            .with_line_join(LineJoin::Round),
    ));
    stroke.move_to(at(curve[0].0, curve[0].1));
    for (x, y) in &curve[1..] {
        stroke.line_to(at(*x, *y));
    }
    if let Ok(path) = stroke.build() {
        window.paint_path(path, colors.curve);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::size;

    fn plot() -> Bounds<Pixels> {
        Bounds {
            origin: point(px(40.), px(300.)),
            size: size(px(300.), px(GRAPH_HEIGHT - LABEL_STRIP)),
        }
    }

    fn at(fraction: (f32, f32)) -> Point<Pixels> {
        let bounds = plot();
        point(
            bounds.origin.x + bounds.size.width * fraction.0,
            bounds.origin.y + bounds.size.height * fraction.1,
        )
    }

    #[test]
    fn plot_axes_round_trip_and_clamp() {
        for hz in [20., 80., 1_000., 3_500., 20_000.] {
            assert!((fraction_to_hz(hz_to_fraction(hz)) - hz).abs() / hz < 1e-4);
        }
        assert_eq!(hz_to_fraction(5.), 0.);
        assert_eq!(hz_to_fraction(40_000.), 1.);
        for db in [-12., -3.5, 0., 6., 12.] {
            assert!((fraction_to_db(db_to_fraction(db)) - db).abs() < 1e-4);
        }
        assert_eq!(db_to_fraction(0.), 0.5);
    }

    #[test]
    fn readouts_are_compact() {
        assert_eq!(format_hz(80.), "80 Hz");
        assert_eq!(format_hz(999.6), "1 kHz");
        assert_eq!(format_hz(3_500.), "3.5 kHz");
        assert_eq!(format_hz(10_000.), "10 kHz");
        assert_eq!(format_gain(1.5), "+1.5 dB");
        assert_eq!(format_gain(-3.04), "\u{2212}3.0 dB");
        assert_eq!(format_gain(-0.04), "0.0 dB");
    }

    #[test]
    fn dragging_a_point_moves_it_under_the_pointer_within_its_band() {
        let bounds = plot();
        let mut voice_eq = VoiceEq::default();
        let handle = VoiceEqHandle::Band(VoiceEqBandKind::HighMid);
        assert!(drag_to(
            &mut voice_eq,
            handle,
            bounds,
            at((hz_to_fraction(4_000.), db_to_fraction(6.)))
        ));
        assert_eq!(voice_eq.high_mid.hz, 4_000.);
        assert_eq!(voice_eq.high_mid.gain_db, 6.);

        drag_to(&mut voice_eq, handle, bounds, at((0., 0.)));
        assert_eq!(voice_eq.high_mid.hz, VoiceEqBandKind::HighMid.hz_range().0);
        assert_eq!(voice_eq.high_mid.gain_db, VoiceEq::GAIN_RANGE.1);

        voice_eq.high_pass = false;
        drag_to(
            &mut voice_eq,
            VoiceEqHandle::HighPass,
            bounds,
            at((hz_to_fraction(150.), 0.95)),
        );
        assert!(voice_eq.high_pass);
        assert_eq!(voice_eq.high_pass_hz, 150.);
        assert_eq!(voice_eq.sanitized(), voice_eq);
    }

    #[test]
    fn hit_testing_finds_the_nearest_point_and_ignores_empty_space() {
        let bounds = plot();
        let voice_eq = VoiceEq::from_preset(VoiceEqPreset::Broadcast);
        for handle in VoiceEqHandle::ALL {
            let position = at(handle_position(&voice_eq, handle));
            assert_eq!(
                hit_test(&voice_eq, bounds, position, VoiceEqHandle::HighPass),
                Some(handle),
                "{handle:?}"
            );
        }
        assert_eq!(
            hit_test(&voice_eq, bounds, at((0.5, 0.02)), VoiceEqHandle::HighPass),
            None
        );
        assert_eq!(
            hit_test(
                &voice_eq,
                Bounds::default(),
                point(px(0.), px(0.)),
                VoiceEqHandle::HighPass
            ),
            None
        );
    }

    #[test]
    fn band_sliders_round_trip_through_the_project_model() {
        let mut voice_eq = VoiceEq::default();
        for kind in VoiceEqBandKind::ALL {
            for slider in [
                VoiceEqSlider::Hz(kind),
                VoiceEqSlider::Gain(kind),
                VoiceEqSlider::Q(kind),
            ] {
                let (min, max, _) = slider.limits();
                for value in [min, (min + max) / 2., max] {
                    slider.write(&mut voice_eq, value);
                    assert!((slider.read(&voice_eq.sanitized()) - value).abs() < 1e-3);
                }
            }
        }
        voice_eq.high_pass = false;
        VoiceEqSlider::HighPassHz.write(&mut voice_eq, 100f32.log2());
        assert!(voice_eq.high_pass);
        assert!((voice_eq.high_pass_hz - 100.).abs() < 1e-3);
    }

    #[test]
    fn the_drawn_curve_is_the_one_that_plays() {
        let voice_eq = VoiceEq::from_preset(VoiceEqPreset::Broadcast);
        let settings = frameever_audio::VoiceToneSettings {
            filters: frameever_editor::voice_eq_filters(&voice_eq),
            declick: None,
        };
        for hz in [30., 110., 380., 2_800., 9_000.] {
            assert_eq!(response(&voice_eq).db(hz), settings.response_db(hz));
        }
        let (_, y) = handle_position(&voice_eq, VoiceEqHandle::Band(VoiceEqBandKind::LowMid));
        assert_eq!(y, db_to_fraction(-3.5));
    }

    #[test]
    fn preset_tiles_turn_the_eq_on_off_and_show_custom_curves_as_no_preset() {
        assert_eq!(selected_tile(None), Some(PresetTile::Off));
        let warm = PresetTile::Preset(VoiceEqPreset::Warm)
            .applied_to(None)
            .unwrap();
        assert_eq!(warm, VoiceEq::from_preset(VoiceEqPreset::Warm));
        assert_eq!(
            selected_tile(Some(warm)),
            Some(PresetTile::Preset(VoiceEqPreset::Warm))
        );

        let declicked = VoiceEq {
            declick: VoiceDeclick::Strong,
            ..warm
        };
        let clear = PresetTile::Preset(VoiceEqPreset::Clear)
            .applied_to(Some(declicked))
            .unwrap();
        assert_eq!(clear.preset, VoiceEqPreset::Clear);
        assert_eq!(clear.declick, VoiceDeclick::Strong);
        assert_eq!(
            clear.high_mid,
            VoiceEq::from_preset(VoiceEqPreset::Clear).high_mid
        );

        let custom = VoiceEq {
            preset: VoiceEqPreset::Custom,
            ..warm
        };
        assert_eq!(selected_tile(Some(custom)), None);
        assert_eq!(PresetTile::Off.applied_to(Some(custom)), None);
        let bypassed = VoiceEq {
            enabled: false,
            ..warm
        };
        assert_eq!(selected_tile(Some(bypassed)), Some(PresetTile::Off));
        let enabled = PresetTile::Preset(VoiceEqPreset::Warm)
            .applied_to(Some(bypassed))
            .unwrap();
        assert!(enabled.enabled);
    }

    #[test]
    fn double_click_puts_a_point_back_where_the_preset_had_it() {
        let broadcast = VoiceEq::from_preset(VoiceEqPreset::Broadcast);
        let mut moved = broadcast;
        moved.low_mid.hz = 1_200.;
        moved.low_mid.gain_db = 9.;
        moved.low_mid.q = 3.;
        moved.high_pass = false;
        moved.high_pass_hz = 200.;
        reset_handle(
            &mut moved,
            VoiceEqHandle::Band(VoiceEqBandKind::LowMid),
            VoiceEqPreset::Broadcast,
        );
        assert_eq!(moved.low_mid, broadcast.low_mid);
        assert!(!moved.high_pass);
        reset_handle(
            &mut moved,
            VoiceEqHandle::HighPass,
            VoiceEqPreset::Broadcast,
        );
        assert_eq!(moved, broadcast);
    }

    #[test]
    fn scrolling_widens_or_narrows_bells_only() {
        let mut voice_eq = VoiceEq::default();
        let before = voice_eq.low_mid.q;
        assert!(widen(&mut voice_eq, VoiceEqBandKind::LowMid, 40.));
        assert!(voice_eq.low_mid.q > before);
        assert!(widen(&mut voice_eq, VoiceEqBandKind::LowMid, -400.));
        assert!(voice_eq.low_mid.q < before);
        for _ in 0..100 {
            widen(&mut voice_eq, VoiceEqBandKind::LowMid, -400.);
        }
        assert_eq!(voice_eq.low_mid.q, VoiceEq::Q_RANGE.0);
        let shelf = voice_eq.low;
        assert!(!widen(&mut voice_eq, VoiceEqBandKind::Low, 40.));
        assert_eq!(voice_eq.low, shelf);
    }

    #[test]
    fn the_scope_tag_names_what_a_change_reaches() {
        assert_eq!(
            VoiceEqScope::of(&[0, 1, 2], 3, false),
            VoiceEqScope::AllClips
        );
        assert_eq!(VoiceEqScope::of(&[0, 1, 2], 3, true), VoiceEqScope::Mixed);
        assert_eq!(VoiceEqScope::of(&[1], 3, true), VoiceEqScope::Clip(1));
        assert_eq!(VoiceEqScope::of(&[0, 2], 3, false), VoiceEqScope::Clips(2));
        assert_eq!(VoiceEqScope::of(&[0], 1, false), VoiceEqScope::AllClips);
        assert_eq!(VoiceEqScope::Clip(1).label(), "Clip 2");
        assert_eq!(VoiceEqScope::Clips(3).label(), "3 clips");
    }

    #[test]
    fn clip_menu_summary_names_the_preset_and_de_click() {
        let mut segment = TimelineSegment::default();
        assert_eq!(voice_eq_summary(&segment), "Off");
        segment.voice_eq = Some(VoiceEq::from_preset(VoiceEqPreset::Clear));
        assert_eq!(voice_eq_summary(&segment), "Clear");
        segment.voice_eq = Some(VoiceEq {
            declick: VoiceDeclick::Light,
            ..VoiceEq::from_preset(VoiceEqPreset::Warm)
        });
        assert_eq!(voice_eq_summary(&segment), "Warm \u{b7} De-click");
        segment.voice_eq = Some(VoiceEq {
            enabled: false,
            declick: VoiceDeclick::Strong,
            ..VoiceEq::default()
        });
        assert_eq!(voice_eq_summary(&segment), "De-click");
    }
}
