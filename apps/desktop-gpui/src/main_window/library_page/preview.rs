use std::{sync::Arc, time::Instant};

use gpui::{
    Context, FontWeight, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Pixels,
    RenderImage, SharedString, StatefulInteractiveElement, Styled, StyledImage as _, Task, Window,
    div, img, prelude::FluentBuilder, px, svg,
};

use super::{DETAIL_WIDTH, MainWindow, focus_ring, t, ui, view};
use crate::library::{
    MediaKind,
    player::{self, Event, PlayError, Player, Start},
    view::Entry,
};

const STAGE_WIDTH: f32 = DETAIL_WIDTH - 25.;
const STAGE_HEIGHT: f32 = STAGE_WIDTH * 9. / 16.;
const BUTTON_SIZE: f32 = 24.;
const SKIP_SECONDS: f64 = 5.;
const ICON_SIZE: f32 = 14.;
const FILLED_ICON_SIZE: f32 = 12.;

#[derive(Default)]
pub(super) struct PreviewState {
    player: Option<Player>,
    pump: Option<Task<()>>,
    playing: bool,
    muted: bool,
    position: f64,
    duration: Option<f64>,
    frame: Option<Arc<RenderImage>>,
    retired: Vec<Arc<RenderImage>>,
    failed: Option<PlayError>,
    track: ui::SliderTrack,
    dragging: bool,
    resume_after_drag: bool,
    bounds: (u32, u32),
    presented: u64,
}

pub(super) fn bounds_at(scale: f32) -> (u32, u32) {
    (
        (STAGE_WIDTH * scale).round() as u32,
        (STAGE_HEIGHT * scale).round() as u32,
    )
}

fn seek_fraction(position: f64, duration: f64) -> f32 {
    if duration > 0. {
        (position / duration).clamp(0., 1.) as f32
    } else {
        0.
    }
}

fn skip_target(position: f64, duration: f64, forward: bool) -> f64 {
    let moved = if forward {
        position + SKIP_SECONDS
    } else {
        position - SKIP_SECONDS
    };
    moved.clamp(0., duration)
}

fn time_label(position: f64, duration: f64) -> String {
    format!(
        "{} / {}",
        view::format_clock(position.min(duration)),
        view::format_clock(duration)
    )
}

fn failure_hint(error: PlayError) -> &'static str {
    match error {
        PlayError::Missing => t!("The file may have been moved or deleted. Check its folder."),
        PlayError::Unreadable => t!("The file may be damaged. Try opening it in another player."),
    }
}

impl PreviewState {
    pub(super) fn dragging(&self) -> bool {
        self.dragging
    }

    pub(super) fn stop(&mut self) {
        if let Some(player) = self.player.take() {
            let stats = player.stats();
            tracing::debug!(
                shown = stats.shown,
                skipped = stats.skipped,
                presented = self.presented,
                "the library preview stopped"
            );
        }
        self.pump = None;
        self.presented = 0;
        self.playing = false;
        self.position = 0.;
        self.duration = None;
        self.failed = None;
        self.dragging = false;
        self.resume_after_drag = false;
        self.retired.extend(self.frame.take());
    }

    pub(super) fn pause(&mut self) {
        if self.playing {
            self.playing = false;
            if let Some(player) = &self.player {
                player.pause();
            }
        }
    }

    pub(super) fn drop_retired(&mut self, window: &mut Window) {
        for image in self.retired.drain(..) {
            if let Err(error) = window.drop_image(image) {
                tracing::debug!("Could not drop a preview frame: {error}");
            }
        }
    }

    pub(super) fn sync_bounds(&mut self, bounds: (u32, u32)) {
        if self.bounds != bounds {
            self.bounds = bounds;
            if let Some(player) = &self.player {
                player.set_bounds(bounds);
            }
        }
    }
}

impl MainWindow {
    fn open_library_preview(
        &mut self,
        entry: &Entry,
        position: f64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let preview = &mut self.library.detail.preview;
        preview.failed = None;
        let load = {
            let entry = entry.clone();
            move || player::segments_of(&entry)
        };
        let start = Start {
            position,
            muted: preview.muted,
            bounds: preview.bounds,
        };
        let Some((player, events)) = Player::open(&entry.path, load, start, None) else {
            preview.failed = Some(PlayError::Unreadable);
            cx.notify();
            return false;
        };
        preview.player = Some(player);
        preview.pump = Some(cx.spawn_in(window, async move |this, cx| {
            while let Ok(event) = events.recv_async().await {
                let open = this.update_in(cx, |this, window, cx| {
                    this.on_library_preview_event(event, window, cx);
                });
                if open.is_err() {
                    return;
                }
            }
            this.update(cx, |this, cx| {
                let preview = &mut this.library.detail.preview;
                if preview.player.is_some() {
                    preview.stop();
                    cx.notify();
                }
            })
            .ok();
        }));
        true
    }

    fn on_library_preview_event(
        &mut self,
        event: Event,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let preview = &mut self.library.detail.preview;
        match event {
            Event::Opened { duration } => preview.duration = Some(duration),
            Event::Frame => {
                let Some(frame) = preview.player.as_ref().and_then(Player::take_frame) else {
                    return;
                };
                if let Some(previous) = preview.frame.replace(frame.image)
                    && let Err(error) = window.drop_image(previous)
                {
                    tracing::debug!("Could not drop a preview frame: {error}");
                }
                preview.presented += 1;
                if !preview.dragging {
                    preview.position = frame.position;
                }
            }
            Event::Tick { position } => {
                if !preview.dragging {
                    preview.position = position;
                }
            }
            Event::Ended => {
                preview.playing = false;
                if let Some(duration) = preview.duration {
                    preview.position = duration;
                }
            }
            Event::Failed(error) => {
                preview.player = None;
                preview.playing = false;
                preview.failed = Some(error);
            }
        }
        cx.notify();
    }

    pub(super) fn toggle_library_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(entry) = self.library.single_entry().cloned() else {
            return;
        };
        let preview = &mut self.library.detail.preview;
        if preview.playing {
            preview.pause();
            cx.notify();
            return;
        }
        if preview.player.is_none() {
            let position = preview.position;
            if !self.open_library_preview(&entry, position, window, cx) {
                return;
            }
        }
        let preview = &mut self.library.detail.preview;
        preview.playing = true;
        if let Some(player) = &preview.player {
            player.play();
        }
        cx.notify();
    }

    pub(in crate::main_window) fn pause_library_preview(&mut self, cx: &mut Context<Self>) {
        self.library.detail.preview.pause();
        cx.notify();
    }

    fn seek_library_preview(&mut self, position: f64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(entry) = self.library.single_entry().cloned() else {
            return;
        };
        self.library.detail.preview.position = position;
        if self.library.detail.preview.player.is_none() {
            self.open_library_preview(&entry, position, window, cx);
        } else if let Some(player) = &self.library.detail.preview.player {
            player.seek(position);
        }
        cx.notify();
    }

    fn toggle_library_mute(&mut self, cx: &mut Context<Self>) {
        let preview = &mut self.library.detail.preview;
        preview.muted = !preview.muted;
        if let Some(player) = &preview.player {
            player.set_muted(preview.muted);
        }
        cx.notify();
    }

    fn library_preview_length(&self, entry: &Entry) -> f64 {
        self.library
            .detail
            .preview
            .duration
            .or(entry.duration)
            .unwrap_or(0.)
    }

    fn drag_library_preview(&mut self, x: Pixels, window: &mut Window, cx: &mut Context<Self>) {
        let Some(entry) = self.library.single_entry() else {
            return;
        };
        let duration = self.library_preview_length(entry);
        let preview = &self.library.detail.preview;
        let Some(fraction) = preview
            .track
            .get()
            .and_then(|track| ui::fraction_from_x(x, track))
        else {
            return;
        };
        self.seek_library_preview(f64::from(fraction) * duration, window, cx);
    }

    fn finish_library_drag(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let preview = &mut self.library.detail.preview;
        preview.dragging = false;
        if std::mem::take(&mut preview.resume_after_drag) {
            self.toggle_library_preview(window, cx);
        }
        cx.notify();
    }

    fn on_library_seek_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(entry) = self.library.single_entry() else {
            return;
        };
        let duration = self.library_preview_length(entry);
        let position = self.library.detail.preview.position;
        let target = match event.keystroke.key.as_str() {
            "left" => skip_target(position, duration, false),
            "right" => skip_target(position, duration, true),
            "home" => 0.,
            "end" => duration,
            _ => return,
        };
        cx.stop_propagation();
        self.seek_library_preview(target, window, cx);
    }

    pub fn auto_library_play(&mut self, spec: String, window: &mut Window, cx: &mut Context<Self>) {
        self.expand_library(window, cx);
        let (seconds, name) = spec.split_once(':').unwrap_or((spec.as_str(), ""));
        let seconds = seconds.parse().unwrap_or(5.);
        let name = name.to_string();
        cx.spawn_in(window, async move |this, cx| {
            let tick = std::time::Duration::from_millis(100);
            loop {
                cx.background_executor().timer(tick).await;
                let started = this.update_in(cx, |this, window, cx| {
                    let path = this
                        .library
                        .entries
                        .as_ref()?
                        .iter()
                        .find(|entry| {
                            entry.kind != MediaKind::Screenshot && entry.name.contains(&name)
                        })?
                        .path
                        .clone();
                    this.change_selection(cx, |selection, _| selection.click(&path));
                    this.library.detail.preview.muted = true;
                    this.toggle_library_preview(window, cx);
                    Some(Instant::now())
                });
                let Ok(started) = started else { return };
                let Some(started) = started else { continue };
                cx.background_executor()
                    .timer(std::time::Duration::from_secs_f64(seconds))
                    .await;
                this.update(cx, |this, cx| {
                    let preview = &this.library.detail.preview;
                    if let Some(player) = &preview.player {
                        let stats = player.stats();
                        tracing::info!(
                            seconds = started.elapsed().as_secs_f64(),
                            shown = stats.shown,
                            skipped = stats.skipped,
                            presented = preview.presented,
                            position = preview.position,
                            "library preview playback"
                        );
                    }
                    this.pause_library_preview(cx);
                })
                .ok();
                return;
            }
        })
        .detach();
    }

    fn preview_button(
        &self,
        id: &'static str,
        icon: &'static str,
        icon_size: f32,
        label: &'static str,
    ) -> gpui::Stateful<gpui::Div> {
        let theme = self.theme;
        focus_ring(
            div()
                .id(id)
                .tab_index(0)
                .flex()
                .flex_shrink_0()
                .items_center()
                .justify_center()
                .size(px(BUTTON_SIZE))
                .rounded(px(4.))
                .hover(move |style| style.bg(theme.accent))
                .tooltip(move |_, cx| ui::Tooltip::new(&theme, label).view(cx))
                .tooltip_show_delay(ui::TOOLTIP_SHOW_DELAY)
                .child(
                    svg()
                        .path(icon)
                        .size(px(icon_size))
                        .text_color(theme.foreground),
                ),
            theme,
        )
    }

    fn render_preview_stage(&self, entry: &Entry, cx: &mut Context<Self>) -> gpui::Div {
        let theme = self.theme;
        let preview = &self.library.detail.preview;
        let poster = entry
            .thumbnail
            .as_ref()
            .and_then(|source| self.library.thumbnails.images.get(source))
            .map(|held| held.image.clone());
        let image = preview.frame.clone().or(poster);
        let unedited = entry.kind == MediaKind::Studio && !entry.exported();
        let body = if let Some(error) = preview.failed {
            div()
                .flex()
                .flex_col()
                .size_full()
                .items_center()
                .justify_center()
                .gap(px(4.))
                .px(px(16.))
                .text_center()
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .child(t!("Couldn't play this video")),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(theme.muted_foreground)
                        .child(failure_hint(error)),
                )
                .child(
                    self.detail_button("library-preview-retry", t!("Try again"), false, false)
                        .flex_none()
                        .mt(px(6.))
                        .px(px(12.))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.toggle_library_preview(window, cx);
                        })),
                )
                .into_any_element()
        } else if let Some(image) = image {
            img(image)
                .size_full()
                .object_fit(gpui::ObjectFit::Contain)
                .into_any_element()
        } else {
            div()
                .flex()
                .size_full()
                .items_center()
                .justify_center()
                .child(
                    svg()
                        .path(super::kind_icon(entry.kind))
                        .size(px(24.))
                        .text_color(theme.muted_foreground),
                )
                .into_any_element()
        };
        div()
            .relative()
            .w_full()
            .h(px(STAGE_HEIGHT))
            .flex_shrink_0()
            .rounded(px(6.))
            .overflow_hidden()
            .bg(theme.muted)
            .child(body)
            .when(unedited && preview.failed.is_none(), |this| {
                this.child(
                    super::tile_corner(&theme)
                        .top(px(4.))
                        .left(px(4.))
                        .child(t!("Unedited")),
                )
            })
    }

    fn render_preview_bar(&self, entry: &Entry, cx: &mut Context<Self>) -> gpui::Div {
        let theme = self.theme;
        let preview = &self.library.detail.preview;
        let duration = self.library_preview_length(entry);
        let (play_icon, play_label) = if preview.playing {
            ("icons/pause.svg", t!("Pause"))
        } else {
            ("icons/play.svg", t!("Play"))
        };
        let (mute_icon, mute_label) = if preview.muted {
            ("icons/volume-x.svg", t!("Unmute"))
        } else {
            ("icons/volume-2.svg", t!("Mute"))
        };
        let slider = ui::Slider::new(
            "library-preview-slider",
            seek_fraction(preview.position, duration),
            preview.track.clone(),
        )
        .flex()
        .track(px(4.), theme.component.slider_bar.into())
        .fill(theme.foreground.into())
        .thumb(px(14.), theme.foreground.into(), None)
        .on_drag_start(
            cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                let preview = &mut this.library.detail.preview;
                preview.dragging = true;
                preview.resume_after_drag = preview.playing;
                preview.pause();
                this.drag_library_preview(event.position.x, window, cx);
            }),
        );
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .text_size(px(12.))
            .text_color(theme.muted_foreground)
            .child(
                self.preview_button(
                    "library-preview-play",
                    play_icon,
                    FILLED_ICON_SIZE,
                    play_label,
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    this.toggle_library_preview(window, cx);
                })),
            )
            .child(focus_ring(
                div()
                    .id("library-preview-seek")
                    .tab_index(0)
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .rounded(px(4.))
                    .on_key_down(cx.listener(Self::on_library_seek_key))
                    .child(slider),
                theme,
            ))
            .child(
                div()
                    .flex_shrink_0()
                    .child(SharedString::from(time_label(preview.position, duration))),
            )
            .child(
                self.preview_button("library-preview-mute", mute_icon, ICON_SIZE, mute_label)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.toggle_library_mute(cx);
                    })),
            )
    }

    pub(super) fn render_library_preview(
        &self,
        entry: &Entry,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let stage = self.render_preview_stage(entry, cx);
        div()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .gap(px(6.))
            .child(stage)
            .when(entry.kind != MediaKind::Screenshot, |this| {
                this.child(self.render_preview_bar(entry, cx))
            })
    }

    pub(in crate::main_window) fn library_preview_dragging(&self) -> bool {
        self.library.detail.preview.dragging()
    }

    pub(in crate::main_window) fn render_library_preview_layer(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        ui::Slider::drag_layer(
            "library-preview-drag",
            cx.listener(|this, event: &gpui::MouseMoveEvent, window, cx| {
                this.drag_library_preview(event.position.x, window, cx);
            }),
            cx.listener(|this, _: &gpui::MouseUpEvent, window, cx| {
                this.finish_library_drag(window, cx);
            }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stage_decodes_at_the_pixels_it_covers() {
        assert_eq!(bounds_at(1.), (315, 177));
        assert_eq!(bounds_at(2.), (630, 354));
    }

    #[test]
    fn the_fraction_stays_on_the_track() {
        assert_eq!(seek_fraction(5., 10.), 0.5);
        assert_eq!(seek_fraction(20., 10.), 1.);
        assert_eq!(seek_fraction(5., 0.), 0.);
    }

    #[test]
    fn skipping_stops_at_either_end() {
        assert_eq!(skip_target(2., 60., false), 0.);
        assert_eq!(skip_target(2., 60., true), 7.);
        assert_eq!(skip_target(58., 60., true), 60.);
    }

    #[test]
    fn the_time_never_runs_past_the_length() {
        assert_eq!(time_label(61.2, 60.), "1:00 / 1:00");
    }
}
