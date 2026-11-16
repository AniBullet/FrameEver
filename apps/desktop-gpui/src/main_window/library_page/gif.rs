use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use frameever_export::ExportRange;
use gpui::{
    Context, FocusHandle, FontWeight, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Pixels, SharedString, StatefulInteractiveElement, Styled, StyledImage as _, Task, Window,
    canvas, div, img, prelude::FluentBuilder, px, svg,
};

use super::{MainWindow, focus_ring, library, t, ui, view};
use crate::{
    i18n::tc,
    library::gif::{CANCELLED, GifSize, GifSource, estimated_bytes, is_large},
};

const DIALOG_WIDTH: f32 = 420.;
const THUMB_HEIGHT: f32 = 214.;
const TOAST_WIDTH: f32 = 440.;
const TOAST_BOTTOM: f32 = 44.;
const TOAST_ABOVE_UNDO: f32 = 96.;
const SAVED_HOLD: Duration = Duration::from_secs(8);
const SHORTEST_PART: f64 = 0.02;
const SHORTEST_SECONDS: f64 = 0.2;
const KEY_STEP_PART: f64 = 0.01;
const KEY_STEP_SECONDS: f64 = 0.1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Handle {
    Start,
    End,
}

fn hundredths(seconds: f64) -> f64 {
    (seconds * 100.0).round() / 100.0
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Stretch {
    seconds: f64,
    start: f64,
    end: f64,
}

impl Stretch {
    fn whole(seconds: f64) -> Self {
        Self {
            seconds,
            start: 0.0,
            end: seconds,
        }
    }

    fn shortest(self) -> f64 {
        (self.seconds * SHORTEST_PART)
            .max(SHORTEST_SECONDS)
            .min(self.seconds)
    }

    fn fractions(self) -> (f32, f32) {
        (
            (self.start / self.seconds) as f32,
            (self.end / self.seconds) as f32,
        )
    }

    fn nearest(self, fraction: f32) -> Handle {
        let (start, end) = self.fractions();
        if (fraction - start).abs() <= (fraction - end).abs() {
            Handle::Start
        } else {
            Handle::End
        }
    }

    fn set(&mut self, handle: Handle, seconds: f64) {
        let gap = self.shortest();
        match handle {
            Handle::Start => self.start = seconds.min(self.end - gap).max(0.0),
            Handle::End => self.end = seconds.max(self.start + gap).min(self.seconds),
        }
    }

    fn move_to(&mut self, handle: Handle, fraction: f32) {
        self.set(handle, hundredths(f64::from(fraction) * self.seconds));
    }

    fn nudge(&mut self, handle: Handle, forward: bool) {
        let step = (self.seconds * KEY_STEP_PART).max(KEY_STEP_SECONDS);
        let at = match handle {
            Handle::Start => self.start,
            Handle::End => self.end,
        };
        self.set(
            handle,
            hundredths(if forward { at + step } else { at - step }),
        );
    }

    fn range(self) -> ExportRange {
        ExportRange {
            start: self.start,
            end: self.end,
        }
    }

    fn length(self) -> f64 {
        self.end - self.start
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DialogFocus {
    Start,
    End,
    Size,
    Cancel,
    Save,
}

const STOPS: [DialogFocus; 5] = [
    DialogFocus::Start,
    DialogFocus::End,
    DialogFocus::Size,
    DialogFocus::Cancel,
    DialogFocus::Save,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DialogAnswer {
    Cancel,
    Save,
}

#[derive(Debug, Clone, PartialEq)]
struct GifDialog {
    project: PathBuf,
    name: String,
    source: GifSource,
    stretch: Stretch,
    size: GifSize,
    focus: DialogFocus,
    percent: Option<u32>,
}

fn megabytes(bytes: u64) -> String {
    let megabytes = bytes as f64 / 1e6;
    if megabytes >= 10.0 {
        format!("{megabytes:.0}")
    } else {
        format!("{megabytes:.1}")
    }
}

fn size_label(size: GifSize) -> &'static str {
    match size {
        GifSize::Large => tc!("gif size", "Large"),
        GifSize::Medium => tc!("gif size", "Medium"),
        GifSize::Small => tc!("gif size", "Small"),
    }
}

fn failure_message(project: &Path) -> &'static str {
    if library::instant_output_path(project).is_file() {
        t!("Check the folder has space and try again.")
    } else {
        t!("The video of this recording is missing. Put it back and try again.")
    }
}

impl GifDialog {
    fn new(project: PathBuf, name: String, source: GifSource) -> Self {
        Self {
            project,
            name,
            source,
            stretch: Stretch::whole(source.seconds),
            size: GifSize::Medium,
            focus: DialogFocus::Save,
            percent: None,
        }
    }

    fn saving(&self) -> bool {
        self.percent.is_some()
    }

    fn pixels(&self) -> (u32, u32) {
        self.size.pixels(self.source.pixels)
    }

    fn estimate(&self) -> u64 {
        estimated_bytes(self.pixels(), self.stretch.length())
    }

    fn is_large(&self) -> bool {
        is_large(self.estimate())
    }

    fn times_label(&self) -> String {
        format!(
            "{} – {} · {}",
            view::format_clock(self.stretch.start),
            view::format_clock(self.stretch.end),
            t!(
                "{seconds} s",
                seconds = format!("{:.1}", self.stretch.length())
            )
        )
    }

    fn estimate_label(&self) -> String {
        let (width, height) = self.pixels();
        format!(
            "{width} × {height} · {}",
            t!(
                "15 FPS · about {size} MB",
                size = megabytes(self.estimate())
            )
        )
    }

    fn save_label(&self) -> SharedString {
        match self.percent {
            Some(percent) => t!("Saving… {percent}%", percent = percent).into(),
            None => t!("Save as GIF").into(),
        }
    }

    fn tab(&mut self, forward: bool) {
        if self.saving() {
            self.focus = DialogFocus::Cancel;
            return;
        }
        let at = STOPS
            .iter()
            .position(|stop| *stop == self.focus)
            .unwrap_or(0);
        let next = if forward {
            at + 1
        } else {
            at + STOPS.len() - 1
        };
        self.focus = STOPS[next % STOPS.len()];
    }

    fn step(&mut self, forward: bool) {
        if self.saving() {
            return;
        }
        match self.focus {
            DialogFocus::Start => self.stretch.nudge(Handle::Start, forward),
            DialogFocus::End => self.stretch.nudge(Handle::End, forward),
            DialogFocus::Size => {
                let at = GifSize::ALL
                    .iter()
                    .position(|size| *size == self.size)
                    .unwrap_or(0);
                let next = if forward {
                    (at + 1).min(GifSize::ALL.len() - 1)
                } else {
                    at.saturating_sub(1)
                };
                self.size = GifSize::ALL[next];
            }
            DialogFocus::Cancel | DialogFocus::Save => {}
        }
    }

    fn enter(&self) -> Option<DialogAnswer> {
        if self.saving() {
            return None;
        }
        Some(match self.focus {
            DialogFocus::Cancel => DialogAnswer::Cancel,
            _ => DialogAnswer::Save,
        })
    }
}

struct SavedToast {
    file: PathBuf,
    text: SharedString,
    _expires: Task<()>,
}

#[derive(Default)]
pub(super) struct GifState {
    dialog: Option<GifDialog>,
    focus: Option<FocusHandle>,
    track: ui::SliderTrack,
    dragging: Option<Handle>,
    cancel: Option<Arc<AtomicBool>>,
    toast: Option<SavedToast>,
}

impl GifState {
    pub(super) fn dialog_open(&self) -> bool {
        self.dialog.is_some()
    }
}

impl MainWindow {
    pub(in crate::main_window) fn save_library_gif(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(entry) = self
            .library
            .lead_entry()
            .filter(|entry| library::gif::can_save_as_gif(entry))
            .cloned()
        else {
            return;
        };
        cx.spawn_in(window, async move |this, cx| {
            let project = entry.path.clone();
            let source = cx
                .background_executor()
                .spawn(async move { library::gif::source(&project) })
                .await;
            let Some(source) = source else {
                tracing::error!(path = %entry.path.display(), "the video could not be read for a GIF");
                crate::platform::alert_dialog(
                    t!("Couldn't save the GIF"),
                    if library::instant_output_path(&entry.path).is_file() {
                        t!("The file may be damaged. Try opening it in another player.")
                    } else {
                        failure_message(&entry.path)
                    },
                );
                return;
            };
            this.update_in(cx, |this, window, cx| {
                let focus = this
                    .library
                    .gif
                    .focus
                    .get_or_insert_with(|| cx.focus_handle())
                    .clone();
                this.library.gif.dialog = Some(GifDialog::new(entry.path, entry.name, source));
                window.focus(&focus, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(in crate::main_window) fn dismiss_library_dialog(&mut self, cx: &mut Context<Self>) {
        if self.library.gif.dialog_open() {
            self.cancel_library_gif(cx);
        } else {
            self.close_library_dialog(cx);
        }
    }

    fn cancel_library_gif(&mut self, cx: &mut Context<Self>) {
        if let Some(cancel) = &self.library.gif.cancel {
            cancel.store(true, Ordering::Relaxed);
            return;
        }
        self.library.gif.dialog = None;
        self.library.gif.dragging = None;
        self.focus_library_items(cx);
        cx.notify();
    }

    pub(in crate::main_window) fn step_library_gif(
        &mut self,
        forward: bool,
        cx: &mut Context<Self>,
    ) {
        if let Some(dialog) = &mut self.library.gif.dialog {
            dialog.step(forward);
            cx.notify();
        }
    }

    pub(in crate::main_window) fn tab_library_gif(
        &mut self,
        forward: bool,
        cx: &mut Context<Self>,
    ) {
        if let Some(dialog) = &mut self.library.gif.dialog {
            dialog.tab(forward);
            cx.notify();
        }
    }

    pub(in crate::main_window) fn enter_library_gif(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.library.gif.dialog.as_ref().and_then(GifDialog::enter) {
            Some(DialogAnswer::Save) => self.start_library_gif(window, cx),
            Some(DialogAnswer::Cancel) => self.cancel_library_gif(cx),
            None => {}
        }
    }

    fn start_library_gif(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dialog) = self
            .library
            .gif
            .dialog
            .as_mut()
            .filter(|dialog| !dialog.saving())
        else {
            return;
        };
        dialog.percent = Some(0);
        dialog.focus = DialogFocus::Cancel;
        let project = dialog.project.clone();
        let name = dialog.name.clone();
        let range = dialog.stretch.range();
        let size = dialog.size;
        let cancel = Arc::new(AtomicBool::new(false));
        self.library.gif.cancel = Some(cancel.clone());
        self.library.gif.dragging = None;
        cx.notify();
        let (progress_tx, progress_rx) = flume::unbounded::<u32>();
        cx.spawn_in(window, async move |this, cx| {
            let job_project = project.clone();
            let job_cancel = cancel.clone();
            let job = cx.background_executor().spawn(async move {
                let exports = library::storage().exports_dir();
                library::gif::save_as_gif(
                    &job_project,
                    &name,
                    &exports,
                    range,
                    size,
                    |done, total| {
                        progress_tx.send(done * 100 / total.max(1)).is_ok()
                            && !job_cancel.load(Ordering::Relaxed)
                    },
                )
            });
            while let Ok(percent) = progress_rx.recv_async().await {
                let open = this.update(cx, |this, cx| {
                    if let Some(dialog) = &mut this.library.gif.dialog {
                        dialog.percent = Some(percent);
                    }
                    cx.notify();
                });
                if open.is_err() {
                    cancel.store(true, Ordering::Relaxed);
                    break;
                }
            }
            let result = job.await;
            let saved = result.as_ref().ok().cloned();
            this.update_in(cx, |this, window, cx| {
                this.library.gif.cancel = None;
                match saved {
                    Some(file) => {
                        this.library.gif.dialog = None;
                        this.focus_library_items(cx);
                        this.show_gif_toast(file, window, cx);
                        this.refresh_library(window, cx);
                    }
                    None => {
                        if let Some(dialog) = &mut this.library.gif.dialog {
                            dialog.percent = None;
                            dialog.focus = DialogFocus::Save;
                        }
                    }
                }
                cx.notify();
            })
            .ok();
            if let Err(error) = result
                && error != CANCELLED
            {
                tracing::error!(path = %project.display(), "saving the GIF failed: {error}");
                crate::platform::alert_dialog(
                    t!("Couldn't save the GIF"),
                    failure_message(&project),
                );
            }
        })
        .detach();
    }

    fn show_gif_toast(&mut self, file: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let name = file
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let expires = cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(SAVED_HOLD).await;
            this.update(cx, |this, cx| {
                this.library.gif.toast = None;
                cx.notify();
            })
            .ok();
        });
        self.library.gif.toast = Some(SavedToast {
            file,
            text: t!("Saved {file}", file = name).into(),
            _expires: expires,
        });
        cx.notify();
    }

    fn press_library_gif(&mut self, x: Pixels, cx: &mut Context<Self>) {
        let Some(fraction) = self
            .library
            .gif
            .track
            .get()
            .and_then(|track| ui::fraction_from_x(x, track))
        else {
            return;
        };
        let Some(dialog) = self
            .library
            .gif
            .dialog
            .as_mut()
            .filter(|dialog| !dialog.saving())
        else {
            return;
        };
        let handle = dialog.stretch.nearest(fraction);
        dialog.focus = match handle {
            Handle::Start => DialogFocus::Start,
            Handle::End => DialogFocus::End,
        };
        self.library.gif.dragging = Some(handle);
        self.drag_library_gif(x, cx);
    }

    fn drag_library_gif(&mut self, x: Pixels, cx: &mut Context<Self>) {
        let gif = &mut self.library.gif;
        let (Some(handle), Some(dialog), Some(track)) =
            (gif.dragging, gif.dialog.as_mut(), gif.track.get())
        else {
            return;
        };
        if let Some(fraction) = ui::fraction_from_x(x, track) {
            dialog.stretch.move_to(handle, fraction);
            cx.notify();
        }
    }

    fn gif_button(
        &self,
        id: &'static str,
        label: impl Into<SharedString>,
        primary: bool,
        focused: bool,
        disabled: bool,
    ) -> gpui::Stateful<gpui::Div> {
        let theme = self.theme;
        div()
            .id(id)
            .flex()
            .items_center()
            .h(px(crate::theme::CONTROL_HEIGHT))
            .px(px(12.))
            .rounded(px(6.))
            .text_size(px(13.))
            .font_weight(FontWeight::MEDIUM)
            .border_2()
            .border_color(gpui::transparent_black())
            .when(focused, |this| this.border_color(theme.ring))
            .when(disabled, |this| this.opacity(0.5))
            .when(!disabled, |this| this.cursor_pointer())
            .when(primary, |this| {
                this.bg(theme.primary)
                    .text_color(theme.primary_foreground)
                    .when(!disabled, |this| {
                        this.hover(move |style| style.bg(theme.primary_hover))
                    })
            })
            .when(!primary, |this| {
                this.bg(theme.secondary)
                    .text_color(theme.foreground)
                    .hover(move |style| style.bg(theme.accent))
            })
            .child(label.into())
    }

    fn render_gif_range(&self, dialog: &GifDialog, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let (start, end) = dialog.stretch.fractions();
        let saving = dialog.saving();
        let track = self.library.gif.track.clone();
        let handle = |fraction: f32, focused: bool| {
            div()
                .absolute()
                .top(px(4.))
                .left(gpui::relative(fraction))
                .ml(px(-3.))
                .w(px(6.))
                .h(px(16.))
                .rounded(px(3.))
                .bg(theme.foreground)
                .when(focused, |this| this.bg(theme.ring))
        };
        div()
            .id("library-gif-range")
            .relative()
            .h(px(24.))
            .mt(px(10.))
            .when(saving, |this| this.opacity(0.5))
            .when(!saving, |this| {
                this.cursor_pointer().on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                        this.press_library_gif(event.position.x, cx);
                    }),
                )
            })
            .child(
                canvas(move |bounds, _, _| track.set(Some(bounds)), |_, _, _, _| {})
                    .absolute()
                    .size_full(),
            )
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(px(10.))
                    .h(px(4.))
                    .rounded(px(2.))
                    .bg(theme.component.slider_bar),
            )
            .child(
                div()
                    .absolute()
                    .top(px(10.))
                    .h(px(4.))
                    .left(gpui::relative(start))
                    .w(gpui::relative(end - start))
                    .rounded(px(2.))
                    .bg(theme.primary),
            )
            .child(handle(start, dialog.focus == DialogFocus::Start))
            .child(handle(end, dialog.focus == DialogFocus::End))
    }

    fn render_gif_sizes(&self, dialog: &GifDialog, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let saving = dialog.saving();
        div()
            .flex()
            .p(px(2.))
            .gap(px(2.))
            .rounded(px(6.))
            .bg(theme.muted)
            .border_1()
            .border_color(gpui::transparent_black())
            .when(dialog.focus == DialogFocus::Size, |this| {
                this.border_color(theme.ring)
            })
            .when(saving, |this| this.opacity(0.5))
            .children(GifSize::ALL.into_iter().enumerate().map(|(index, size)| {
                let on = dialog.size == size;
                div()
                    .id(("library-gif-size", index))
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .justify_center()
                    .h(px(crate::theme::CONTROL_HEIGHT - 6.))
                    .rounded(px(5.))
                    .text_size(px(12.))
                    .text_color(if on {
                        theme.foreground
                    } else {
                        theme.muted_foreground
                    })
                    .when(on, |this| this.bg(theme.secondary))
                    .when(!saving, |this| {
                        this.cursor_pointer()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(dialog) = &mut this.library.gif.dialog {
                                    dialog.size = size;
                                    dialog.focus = DialogFocus::Size;
                                }
                                cx.notify();
                            }))
                    })
                    .child(size_label(size))
            }))
    }

    fn render_gif_estimate(&self, dialog: &GifDialog) -> impl IntoElement {
        let theme = self.theme;
        let large = dialog.is_large();
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(6.))
            .mt(px(10.))
            .text_size(px(12.))
            .text_color(if large {
                theme.warning_text
            } else {
                theme.muted_foreground
            })
            .when(large, |this| {
                this.child(
                    svg()
                        .path("icons/triangle-alert.svg")
                        .size(px(14.))
                        .flex_shrink_0()
                        .text_color(theme.warning_text),
                )
            })
            .child(dialog.estimate_label())
            .when(large, |this| {
                this.child(t!("Pick a shorter stretch or a smaller size."))
            })
    }

    pub(in crate::main_window) fn render_library_gif(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let theme = self.theme;
        let dialog = self.library.gif.dialog.as_ref()?;
        let focus = self.library.gif.focus.as_ref()?;
        let saving = dialog.saving();
        let poster = self
            .library
            .entry(&dialog.project)
            .and_then(|entry| entry.thumbnail.as_ref())
            .and_then(|source| self.library.thumbnails.images.get(source))
            .map(|held| held.image.clone());
        Some(
            div()
                .id("library-gif-scrim")
                .key_context("LibraryDialog")
                .track_focus(focus)
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .p(px(16.))
                .occlude()
                .bg(theme.overlay)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .w(px(DIALOG_WIDTH))
                        .max_w_full()
                        .max_h_full()
                        .p(px(20.))
                        .rounded(px(10.))
                        .border_1()
                        .border_color(theme.border)
                        .bg(theme.popover)
                        .text_color(theme.foreground)
                        .child(
                            div()
                                .text_size(px(15.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(t!("Save as GIF")),
                        )
                        .child(
                            div()
                                .mt(px(6.))
                                .mb(px(12.))
                                .text_size(px(13.))
                                .text_color(theme.muted_foreground)
                                .child(t!(
                                    "From “{name}”. The recording stays as it is.",
                                    name = dialog.name
                                )),
                        )
                        .child(
                            div()
                                .w_full()
                                .h(px(THUMB_HEIGHT))
                                .min_h_0()
                                .flex_shrink(1.)
                                .rounded(px(8.))
                                .overflow_hidden()
                                .bg(theme.muted)
                                .children(poster.map(|image| {
                                    img(image).size_full().object_fit(gpui::ObjectFit::Contain)
                                })),
                        )
                        .child(self.render_gif_range(dialog, cx))
                        .child(
                            div()
                                .mt(px(2.))
                                .mb(px(12.))
                                .text_center()
                                .text_size(px(12.))
                                .child(dialog.times_label()),
                        )
                        .child(
                            div()
                                .mb(px(6.))
                                .text_size(px(12.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme.muted_foreground)
                                .child(tc!("gif size", "Size")),
                        )
                        .child(self.render_gif_sizes(dialog, cx))
                        .child(self.render_gif_estimate(dialog))
                        .child(
                            div()
                                .flex()
                                .justify_end()
                                .gap(px(8.))
                                .mt(px(16.))
                                .child(
                                    self.gif_button(
                                        "library-gif-cancel",
                                        t!("Cancel"),
                                        false,
                                        dialog.focus == DialogFocus::Cancel,
                                        false,
                                    )
                                    .on_click(cx.listener(
                                        |this, _, _, cx| {
                                            this.cancel_library_gif(cx);
                                        },
                                    )),
                                )
                                .child(
                                    self.gif_button(
                                        "library-gif-save",
                                        dialog.save_label(),
                                        true,
                                        dialog.focus == DialogFocus::Save,
                                        saving,
                                    )
                                    .when(!saving, |this| {
                                        this.on_click(cx.listener(|this, _, window, cx| {
                                            this.start_library_gif(window, cx);
                                        }))
                                    }),
                                ),
                        ),
                )
                .when(self.library.gif.dragging.is_some(), |this| {
                    this.child(ui::Slider::drag_layer(
                        "library-gif-drag",
                        cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                            this.drag_library_gif(event.position.x, cx);
                        }),
                        cx.listener(|this, _: &gpui::MouseUpEvent, _, cx| {
                            this.library.gif.dragging = None;
                            cx.notify();
                        }),
                    ))
                }),
        )
    }

    fn gif_toast_button(&self, id: &'static str, label: &'static str) -> gpui::Stateful<gpui::Div> {
        let theme = self.theme;
        focus_ring(
            div()
                .id(id)
                .tab_index(0)
                .flex()
                .flex_shrink_0()
                .items_center()
                .h(px(28.))
                .px(px(10.))
                .rounded(px(6.))
                .cursor_pointer()
                .font_weight(FontWeight::MEDIUM)
                .hover(move |style| style.bg(theme.accent))
                .child(label),
            theme,
        )
    }

    pub(in crate::main_window) fn render_library_gif_toast(
        &self,
    ) -> Option<impl IntoElement + use<>> {
        let theme = self.theme;
        let toast = self.library.gif.toast.as_ref()?;
        let undo_shown = self.library.delete.toast_shown();
        let (copy, reveal) = (toast.file.clone(), toast.file.clone());
        Some(
            div()
                .absolute()
                .bottom(px(if undo_shown {
                    TOAST_ABOVE_UNDO
                } else {
                    TOAST_BOTTOM
                }))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .px(px(16.))
                .child(
                    div()
                        .id("library-gif-toast")
                        .occlude()
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .w(px(TOAST_WIDTH))
                        .max_w_full()
                        .py(px(8.))
                        .pl(px(12.))
                        .pr(px(8.))
                        .rounded(px(8.))
                        .border_1()
                        .border_color(theme.border)
                        .bg(theme.popover)
                        .text_size(px(13.))
                        .text_color(theme.foreground)
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .child(toast.text.clone()),
                        )
                        .child(
                            self.gif_toast_button("library-gif-copy", t!("Copy"))
                                .on_click(move |_, _, cx| {
                                    super::detail::copy_files(std::slice::from_ref(&copy), cx);
                                }),
                        )
                        .child(
                            self.gif_toast_button("library-gif-reveal", t!("Show in folder"))
                                .on_click(move |_, _, cx| library::reveal_path(&reveal, cx)),
                        ),
                ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dialog(seconds: f64, pixels: (u32, u32)) -> GifDialog {
        GifDialog::new(
            PathBuf::from("Demo.ever"),
            "Demo".to_string(),
            GifSource { pixels, seconds },
        )
    }

    #[test]
    fn the_dialog_opens_on_the_whole_recording_at_medium_with_save_focused() {
        let dialog = dialog(12.0, (1920, 1080));

        assert_eq!(
            dialog.stretch.range(),
            ExportRange {
                start: 0.0,
                end: 12.0
            }
        );
        assert_eq!(dialog.size, GifSize::Medium);
        assert_eq!(dialog.focus, DialogFocus::Save);
        assert_eq!(dialog.times_label(), "0:00 – 0:12 · 12.0 s");
        assert_eq!(dialog.estimate_label(), "540 × 304 · 15 FPS · about 7.4 MB");
        assert!(!dialog.is_large());
    }

    #[test]
    fn a_press_moves_the_nearer_handle_and_a_drag_keeps_the_stretch_from_closing() {
        let mut stretch = Stretch::whole(10.0);

        assert_eq!(stretch.nearest(0.2), Handle::Start);
        assert_eq!(stretch.nearest(0.8), Handle::End);
        assert_eq!(stretch.nearest(0.5), Handle::Start);

        stretch.move_to(Handle::Start, 0.2);
        stretch.move_to(Handle::End, 0.6);
        assert_eq!(
            stretch.range(),
            ExportRange {
                start: 2.0,
                end: 6.0
            }
        );
        assert_eq!(stretch.fractions(), (0.2, 0.6));

        stretch.move_to(Handle::Start, 0.9);
        assert_eq!(
            stretch.range(),
            ExportRange {
                start: 5.8,
                end: 6.0
            }
        );
        stretch.move_to(Handle::End, 0.0);
        assert_eq!(
            stretch.range(),
            ExportRange {
                start: 5.8,
                end: 6.0
            }
        );
        stretch.move_to(Handle::End, 1.5);
        assert_eq!(
            stretch.range(),
            ExportRange {
                start: 5.8,
                end: 10.0
            }
        );
    }

    #[test]
    fn a_long_recording_keeps_at_least_a_fiftieth_of_its_length_between_the_handles() {
        let mut stretch = Stretch::whole(100.0);
        stretch.move_to(Handle::End, 0.0);
        assert_eq!(
            stretch.range(),
            ExportRange {
                start: 0.0,
                end: 2.0
            }
        );

        let mut short = Stretch::whole(0.1);
        short.move_to(Handle::Start, 1.0);
        assert_eq!(
            short.range(),
            ExportRange {
                start: 0.0,
                end: 0.1
            }
        );
    }

    #[test]
    fn the_estimate_follows_the_stretch_and_the_size_and_warns_when_large() {
        let mut dialog = dialog(30.0, (1920, 1080));

        dialog.size = GifSize::Large;
        assert_eq!(dialog.estimate_label(), "1080 × 608 · 15 FPS · about 74 MB");
        assert!(dialog.is_large());

        dialog.size = GifSize::Small;
        assert_eq!(dialog.estimate_label(), "270 × 152 · 15 FPS · about 4.6 MB");
        assert!(!dialog.is_large());

        dialog.size = GifSize::Large;
        dialog.stretch.move_to(Handle::End, 0.1);
        assert_eq!(dialog.times_label(), "0:00 – 0:03 · 3.0 s");
        assert_eq!(
            dialog.estimate_label(),
            "1080 × 608 · 15 FPS · about 7.4 MB"
        );
        assert!(!dialog.is_large());
    }

    #[test]
    fn tab_walks_the_handles_the_sizes_and_the_buttons_and_arrows_change_the_focused_one() {
        let mut dialog = dialog(10.0, (800, 600));

        let mut walked = Vec::new();
        for _ in 0..5 {
            dialog.tab(true);
            walked.push(dialog.focus);
        }
        assert_eq!(
            walked,
            [
                DialogFocus::Start,
                DialogFocus::End,
                DialogFocus::Size,
                DialogFocus::Cancel,
                DialogFocus::Save,
            ]
        );
        dialog.tab(false);
        assert_eq!(dialog.focus, DialogFocus::Cancel);

        dialog.focus = DialogFocus::Start;
        dialog.step(true);
        assert_eq!(
            dialog.stretch.range(),
            ExportRange {
                start: 0.1,
                end: 10.0
            }
        );
        dialog.focus = DialogFocus::End;
        dialog.step(false);
        assert_eq!(
            dialog.stretch.range(),
            ExportRange {
                start: 0.1,
                end: 9.9
            }
        );

        dialog.focus = DialogFocus::Size;
        dialog.step(true);
        assert_eq!(dialog.size, GifSize::Small);
        dialog.step(true);
        assert_eq!(dialog.size, GifSize::Small);
        dialog.step(false);
        dialog.step(false);
        assert_eq!(dialog.size, GifSize::Large);
    }

    #[test]
    fn enter_saves_unless_cancel_is_focused_and_does_nothing_while_saving() {
        let mut dialog = dialog(10.0, (800, 600));
        assert_eq!(dialog.enter(), Some(DialogAnswer::Save));
        dialog.focus = DialogFocus::Size;
        assert_eq!(dialog.enter(), Some(DialogAnswer::Save));
        dialog.focus = DialogFocus::Cancel;
        assert_eq!(dialog.enter(), Some(DialogAnswer::Cancel));

        dialog.percent = Some(40);
        assert_eq!(dialog.enter(), None);
        assert_eq!(dialog.save_label().as_ref(), "Saving… 40%");
        dialog.focus = DialogFocus::Save;
        dialog.tab(true);
        assert_eq!(dialog.focus, DialogFocus::Cancel);
        dialog.tab(true);
        assert_eq!(dialog.focus, DialogFocus::Cancel);
    }

    #[test]
    fn a_failure_says_whether_the_video_is_gone_or_the_folder_is_the_problem() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("Demo.ever");
        std::fs::create_dir_all(project.join("content")).unwrap();

        assert_eq!(
            failure_message(&project),
            "The video of this recording is missing. Put it back and try again."
        );
        std::fs::write(project.join("content/output.mp4"), b"video").unwrap();
        assert_eq!(
            failure_message(&project),
            "Check the folder has space and try again."
        );
    }
}
