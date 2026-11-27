use std::path::{Path, PathBuf};

use gpui::{
    AnyElement, Context, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, Task, Window, div, prelude::FluentBuilder,
    px, svg,
};

use super::{
    Clock, DETAIL_WIDTH, JobState, LibraryWindow, app_windows, day_label, focus_ring, kind_label,
    library, preview, t, ui, view,
};
use crate::library::{
    MediaKind, ProjectDetails, ProjectTracks, VideoVersions, compress_queue, view::Entry,
};

pub(super) struct DetailState {
    name_input: Entity<ui::TextInputState>,
    project: Option<PathBuf>,
    shown_name: String,
    latest_export: Option<PathBuf>,
    details: ProjectDetails,
    task: Option<Task<()>>,
    pub(super) stale: bool,
    pub(super) preview: preview::PreviewState,
}

impl DetailState {
    pub(super) fn new(name_input: Entity<ui::TextInputState>) -> Self {
        Self {
            name_input,
            project: None,
            shown_name: String::new(),
            latest_export: None,
            details: ProjectDetails::default(),
            task: None,
            stale: false,
            preview: preview::PreviewState::default(),
        }
    }

    pub(super) fn clear(&mut self) {
        self.project = None;
        self.latest_export = None;
        self.details = ProjectDetails::default();
        self.task = None;
        self.preview.stop();
    }
}

fn tracks_label(tracks: ProjectTracks) -> String {
    let mut names = vec![t!("Screen")];
    if tracks.camera {
        names.push(t!("Camera"));
    }
    if tracks.microphone {
        names.push(t!("Microphone"));
    }
    if tracks.system_audio {
        names.push(t!("System audio"));
    }
    names.join(" · ")
}

fn resolution_label(details: &ProjectDetails) -> Option<String> {
    let (width, height) = details.resolution?;
    Some(match details.fps {
        Some(fps) => format!("{width} × {height} · {fps} fps"),
        None => format!("{width} × {height}"),
    })
}

fn saved_percent(original: u64, compressed: u64) -> u32 {
    if original == 0 {
        return 0;
    }
    let saved = original.saturating_sub(compressed) as f64 / original as f64;
    (saved * 100.).round() as u32
}

fn meta_line(kind: &str, created: &str, duration: Option<String>, size: String) -> String {
    [
        Some(kind.to_string()),
        Some(created.to_string()),
        duration,
        Some(size),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ")
}

fn copy_failed_alert() -> (&'static str, &'static str) {
    (
        t!("Couldn't copy the file"),
        t!("It may have been moved. Try again."),
    )
}

fn alert_copy_failed(error: String, cx: &mut gpui::App) {
    tracing::warn!("copying the file failed: {error}");
    cx.spawn(async move |_| {
        let (title, message) = copy_failed_alert();
        crate::platform::alert_dialog(title, message);
    })
    .detach();
}

pub(super) fn copy_files(files: &[PathBuf], cx: &mut gpui::App) {
    if let Err(error) = crate::platform::copy_files_to_clipboard(files, cx) {
        alert_copy_failed(error, cx);
    }
}

impl LibraryWindow {
    pub(super) fn sync_library_detail(&mut self, cx: &mut Context<Self>) {
        let Some(entry) = self.library.single_entry().cloned() else {
            self.library.detail.clear();
            return;
        };
        let detail = &mut self.library.detail;
        let new_project = detail.project.as_ref() != Some(&entry.path);
        let untouched = detail.name_input.read(cx).text() == detail.shown_name;
        let latest_export = entry.latest_export().map(|export| export.path.clone());
        if new_project || latest_export != detail.latest_export {
            detail.preview.stop();
            detail.latest_export = latest_export;
        }
        if new_project {
            detail.project = Some(entry.path.clone());
            detail.details = ProjectDetails::default();
        }
        if (new_project || untouched) && entry.name != detail.shown_name {
            detail.shown_name = entry.name.clone();
            let name = entry.name.clone();
            detail
                .name_input
                .update(cx, |input, cx| input.set_text(name, cx));
        }
        if !new_project && !std::mem::take(&mut detail.stale) {
            return;
        }
        detail.task = Some(cx.spawn(async move |this, cx| {
            let project = entry.path.clone();
            let details = cx
                .background_executor()
                .spawn(async move { library::project_details(&entry) })
                .await;
            this.update(cx, |this, cx| {
                let detail = &mut this.library.detail;
                if detail.project.as_ref() == Some(&project) {
                    detail.details = details;
                    cx.notify();
                }
            })
            .ok();
        }));
    }

    pub(super) fn rename_library_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.library.selection.single().is_some() {
            self.library
                .detail
                .name_input
                .update(cx, |input, cx| input.focus_and_select_all(window, cx));
        }
    }

    pub(super) fn on_library_name_event(
        &mut self,
        input: Entity<ui::TextInputState>,
        event: &ui::TextInputEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            ui::TextInputEvent::Confirmed => {
                self.commit_library_name(&input, cx);
                input.update(cx, |input, cx| input.move_to_end(cx));
                self.focus_library_items(cx);
            }
            ui::TextInputEvent::Blurred => self.commit_library_name(&input, cx),
            ui::TextInputEvent::Cancelled => {
                let name = self.library.detail.shown_name.clone();
                input.update(cx, |input, cx| {
                    input.set_text(name, cx);
                    input.move_to_end(cx);
                });
                self.focus_library_items(cx);
            }
            ui::TextInputEvent::Changed => {}
        }
    }

    pub(super) fn focus_library_items(&self, cx: &mut Context<Self>) {
        let focus = self.library.items_focus.clone();
        let Some(library) = cx.global::<app_windows::AppWindows>().library else {
            return;
        };
        cx.defer(move |cx| {
            library
                .update(cx, |_, window, cx| window.focus(&focus, cx))
                .ok();
        });
    }

    fn commit_library_name(&mut self, input: &Entity<ui::TextInputState>, cx: &mut Context<Self>) {
        let typed = input.read(cx).text().trim().to_string();
        let Some(project) = self.library.detail.project.clone() else {
            return;
        };
        let Some(entry) = self
            .library
            .entries
            .as_mut()
            .and_then(|entries| entries.iter_mut().find(|entry| entry.path == project))
        else {
            return;
        };
        if typed == self.library.detail.shown_name {
            return;
        }
        if typed.is_empty() {
            let name = self.library.detail.shown_name.clone();
            input.update(cx, |input, cx| input.set_text(name, cx));
            return;
        }
        entry.name = typed.clone();
        self.library.detail.shown_name = typed.clone();
        self.rebuild_library(cx);
        cx.spawn(async move |this, cx| {
            let saved = cx
                .background_executor()
                .spawn(async move { library::rename_project(&project, &typed) })
                .await;
            if let Err(error) = saved {
                tracing::error!("renaming the project failed: {error}");
                crate::platform::alert_dialog(
                    t!("Couldn't rename this project"),
                    t!("Its files may be in use. Try again."),
                );
            }
            this.update(cx, |_, cx| {
                cx.defer(app_windows::refresh_library_everywhere)
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn reveal_library_selection(&self, cx: &mut Context<Self>) {
        if self.library.selection.items().len() > 1 {
            let items = self.library.selection.items().to_vec();
            cx.background_executor()
                .spawn(async move {
                    if let Err(error) = crate::platform::reveal_items(&items) {
                        tracing::warn!("showing the items in File Explorer failed: {error}");
                    }
                })
                .detach();
            return;
        }
        let Some(entry) = self.library.lead_entry() else {
            return;
        };
        match entry.kind {
            MediaKind::Studio => {
                library::open_recording_folder(&entry.path, library::RecordingMode::Studio, cx);
            }
            MediaKind::Instant => {
                library::open_recording_folder(&entry.path, library::RecordingMode::Instant, cx);
            }
            MediaKind::Screenshot => library::reveal_path(&entry.path, cx),
        }
    }

    pub(super) fn copy_library_file(&self, cx: &mut Context<Self>) {
        let files = self.library_files_to_share();
        if !files.is_empty() {
            copy_files(&files, cx);
        }
    }

    pub(super) fn open_library_file_externally(&mut self, cx: &mut Context<Self>) {
        let file = self.library.lead_entry().and_then(library::file_to_share);
        if let Some(file) = file {
            self.pause_library_preview(cx);
            library::open_with_system(&file, cx);
        }
    }

    pub(super) fn library_files_to_share(&self) -> Vec<PathBuf> {
        library::files_to_share(self.library.selected_entries())
    }

    pub(super) fn library_compressible(&self) -> Vec<PathBuf> {
        self.library
            .selected_entries()
            .into_iter()
            .filter(|entry| entry.compressible)
            .filter(|entry| compress_queue::compressions().state(&entry.path).is_none())
            .map(|entry| entry.path.clone())
            .collect()
    }

    pub(super) fn export_library_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(entry) = self.library.lead_entry().cloned() else {
            return;
        };
        match (&entry.screenshot, entry.kind) {
            (Some(png), _) => self.save_screenshot(png.clone(), entry.name, window, cx),
            (None, MediaKind::Instant) => self.export_library_instant(entry, window, cx),
            (None, _) if entry.opens_editor => {
                cx.defer(move |cx| app_windows::open_editor_at_export(entry.path, cx));
            }
            _ => {}
        }
    }

    fn export_library_instant(
        &mut self,
        entry: Entry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.spawn_in(window, async move |this, cx| {
            let file_name = format!("{}.mp4", entry.name);
            let Some(dest) = crate::platform::save_file_panel_async(&file_name, &["mp4"], cx).await
            else {
                return;
            };
            let source = library::instant_output_path(&entry.path);
            let project = entry.path;
            let copied = cx
                .background_executor()
                .spawn(async move {
                    library::copy_file_to_path(&source, &dest)?;
                    library::record_export(
                        &project,
                        &library::export_record(dest, "mp4", None),
                        true,
                    );
                    Ok::<(), String>(())
                })
                .await;
            if let Err(error) = copied {
                tracing::error!("exporting the recording failed: {error}");
                crate::platform::alert_dialog(
                    t!("Couldn't export the recording"),
                    t!("Check the folder has space and try again."),
                );
            }
            this.update_in(cx, |this, window, cx| this.refresh_library(window, cx))
                .ok();
        })
        .detach();
    }

    pub(super) fn compress_library_place(&mut self, cx: &mut Context<Self>) {
        let projects: Vec<PathBuf> = self
            .library
            .order
            .iter()
            .filter_map(|path| self.library.entry(path))
            .filter(|entry| entry.compressible)
            .filter(|entry| compress_queue::compressions().state(&entry.path).is_none())
            .map(|entry| entry.path.clone())
            .collect();
        compress_queue::compress(projects, compress_queue::Request::MANUAL);
        cx.notify();
    }

    pub(super) fn compress_library_selection(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        compress_queue::compress(self.library_compressible(), compress_queue::Request::MANUAL);
        cx.notify();
    }

    fn switch_library_version(
        &mut self,
        change: fn(&Path) -> std::io::Result<()>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = self.library.selection.single().map(Path::to_path_buf) else {
            return;
        };
        self.library.detail.preview.stop();
        cx.spawn_in(window, async move |this, cx| {
            let changed = cx
                .background_executor()
                .spawn(async move { change(&path) })
                .await;
            if let Err(error) = changed {
                tracing::error!("changing the video version failed: {error}");
                crate::platform::alert_dialog(
                    t!("Couldn't change the video version"),
                    t!("The file may be in use. Close what is using it and try again."),
                );
            }
            this.update_in(cx, |this, window, cx| {
                this.library.detail.stale = true;
                this.sync_library_detail(cx);
                this.refresh_library(window, cx);
            })
            .ok();
        })
        .detach();
    }

    fn remove_library_export(
        &mut self,
        export: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(project) = self.library.selection.single().map(Path::to_path_buf) else {
            return;
        };
        cx.spawn_in(window, async move |this, cx| {
            let removed = cx
                .background_executor()
                .spawn(async move { library::remove_export(&project, &export) })
                .await;
            if let Err(error) = removed {
                tracing::error!("removing the export record failed: {error}");
            }
            this.update_in(cx, |this, window, cx| this.refresh_library(window, cx))
                .ok();
        })
        .detach();
    }

    pub(super) fn detail_button(
        &self,
        id: &'static str,
        label: impl Into<SharedString>,
        primary: bool,
        disabled: bool,
    ) -> gpui::Stateful<gpui::Div> {
        let theme = self.theme;
        focus_ring(
            div()
                .id(id)
                .when(!disabled, |this| this.tab_index(0))
                .flex()
                .flex_1()
                .min_w_0()
                .items_center()
                .justify_center()
                .h(px(crate::theme::CONTROL_HEIGHT))
                .px(px(6.))
                .rounded(px(6.))
                .text_size(px(12.))
                .font_weight(FontWeight::MEDIUM)
                .when(disabled, |this| this.opacity(0.5))
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
                        .when(!disabled, |this| {
                            this.hover(move |style| style.bg(theme.secondary_hover))
                        })
                })
                .child(div().truncate().child(label.into())),
            theme,
        )
    }

    fn foot_button(
        &self,
        id: &'static str,
        label: &'static str,
        disabled: bool,
    ) -> gpui::Stateful<gpui::Div> {
        self.detail_button(id, label, false, disabled)
            .bg(self.theme.background)
    }

    fn render_detail_actions(&self, entry: &Entry, cx: &mut Context<Self>) -> gpui::Div {
        let row = div().flex().gap(px(8.));
        let export = |this: &Self, primary: bool, cx: &mut Context<Self>| {
            this.detail_button("library-detail-export", t!("Export"), primary, false)
                .on_click(cx.listener(|this, _, window, cx| {
                    this.export_library_selection(window, cx);
                }))
        };
        let open = entry.clone();
        let open_button = self
            .detail_button(
                "library-detail-open",
                t!("Open in editor"),
                true,
                !entry.opens_editor,
            )
            .when(entry.opens_editor, |this| {
                this.on_click(cx.listener(move |this, _, window, cx| {
                    this.open_library_entry(&open, window, cx);
                }))
            });
        match (&entry.screenshot, entry.kind) {
            (Some(png), _) => {
                let png = png.clone();
                row.child(open_button).child(
                    self.detail_button("library-detail-copy-image", t!("Copy image"), false, false)
                        .on_click(move |_, _, cx| {
                            crate::platform::copy_image_to_clipboard(
                                png.clone(),
                                Some(copy_failed_alert()),
                                cx,
                            );
                        }),
                )
            }
            (None, MediaKind::Instant) => {
                let saves_gif = library::gif::can_save_as_gif(entry);
                row.child(export(self, true, cx)).child(
                    self.detail_button("library-detail-gif", t!("Save as GIF"), false, !saves_gif)
                        .when(saves_gif, |this| {
                            this.on_click(cx.listener(|this, _, window, cx| {
                                this.save_library_gif(window, cx);
                            }))
                        }),
                )
            }
            _ => row.child(open_button).child(export(self, false, cx)),
        }
    }

    fn render_detail_meta(&self, entry: &Entry) -> gpui::Div {
        let clock = Clock::now();
        let created = format!(
            "{} {}",
            day_label(&clock, entry.created_millis),
            clock.time_of(entry.created_millis)
        );
        div()
            .truncate()
            .text_size(px(12.))
            .text_color(self.theme.muted_foreground)
            .child(meta_line(
                kind_label(entry.kind),
                &created,
                entry.duration.map(view::format_clock),
                view::format_size(entry.size),
            ))
    }

    fn render_detail_info(&self) -> Option<gpui::Div> {
        let theme = self.theme;
        let details = &self.library.detail.details;
        let mut rows: Vec<(&'static str, String)> = Vec::new();
        if let Some(resolution) = resolution_label(details) {
            rows.push((t!("Resolution"), resolution));
        }
        if let Some(tracks) = details.tracks {
            rows.push((t!("Tracks"), tracks_label(tracks)));
        }
        if rows.is_empty() {
            return None;
        }
        Some(
            div()
                .flex()
                .flex_col()
                .child(self.subhead(t!("Details")))
                .child(
                    div()
                        .w_full()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .text_size(px(12.))
                        .children(rows.into_iter().map(move |(label, value)| {
                            div()
                                .flex()
                                .gap(px(10.))
                                .child(
                                    div()
                                        .w(px(80.))
                                        .flex_shrink_0()
                                        .text_color(theme.muted_foreground)
                                        .child(label),
                                )
                                .child(div().flex_1().min_w_0().child(value))
                        })),
                ),
        )
    }

    fn subhead(&self, label: &'static str) -> gpui::Div {
        div()
            .mb(px(6.))
            .text_size(px(12.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(self.theme.muted_foreground)
            .child(label)
    }

    fn render_detail_compress(&self, entry: &Entry, cx: &mut Context<Self>) -> Option<gpui::Div> {
        if entry.kind != MediaKind::Instant || entry.screenshot.is_some() {
            return None;
        }
        let theme = self.theme;
        let job = compress_queue::compressions().state(&entry.path);
        if !entry.compressible && job.is_none() {
            return None;
        }
        let (status, percent): (SharedString, Option<u64>) = match job {
            Some(JobState::Running { percent, left }) => (
                match left {
                    Some(left) => format!(
                        "{} · {}",
                        super::job_label(JobState::Running {
                            percent,
                            left: None
                        }),
                        super::time_left_label(left)
                    )
                    .into(),
                    None => super::job_label(JobState::Running { percent, left }).into(),
                },
                Some(percent),
            ),
            Some(JobState::Waiting) => (t!("Queued").into(), Some(0)),
            None => (t!("Not made yet").into(), None),
        };
        let idle = job.is_none();
        let button: SharedString = if idle {
            t!("Compress").into()
        } else {
            t!("Cancel").into()
        };
        let project = entry.path.clone();
        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(div().text_size(px(13.)).child(t!("Compressed copy")))
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(theme.muted_foreground)
                                        .child(status),
                                ),
                        )
                        .child(
                            self.detail_button("library-detail-compress", button, false, false)
                                .flex_none()
                                .px(px(12.))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    if idle {
                                        this.compress_library_selection(window, cx);
                                    } else {
                                        compress_queue::compressions().cancel(&project);
                                        cx.notify();
                                    }
                                })),
                        ),
                )
                .children(percent.map(|percent| super::progress_bar(&theme, percent))),
        )
    }

    fn render_detail_versions(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let theme = self.theme;
        let (original, compressed, using_compressed) = match self.library.detail.details.versions? {
            VideoVersions::Single { .. } => return None,
            VideoVersions::CompressedCopy {
                original,
                compressed,
            } => (original, compressed, false),
            VideoVersions::UsingCompressed {
                original,
                compressed,
            } => (original, compressed, true),
        };
        let option = |this: &Self,
                      id: &'static str,
                      label: &'static str,
                      size: String,
                      on: bool,
                      change: fn(&Path) -> std::io::Result<()>,
                      cx: &mut Context<Self>| {
            focus_ring(
                div()
                    .id(id)
                    .tab_index(0)
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .justify_center()
                    .gap(px(4.))
                    .h(px(crate::theme::CONTROL_HEIGHT - 6.))
                    .rounded(px(5.))
                    .text_size(px(12.))
                    .text_color(if on {
                        theme.foreground
                    } else {
                        theme.muted_foreground
                    })
                    .when(on, |this| this.bg(theme.secondary))
                    .child(label)
                    .child(div().text_color(theme.muted_foreground).child(size)),
                this.theme,
            )
            .when(!on, |this| {
                this.on_click(cx.listener(move |this, _, window, cx| {
                    this.switch_library_version(change, window, cx);
                }))
            })
        };
        Some(
            div().child(self.subhead(t!("Version"))).child(
                div()
                    .flex()
                    .p(px(2.))
                    .gap(px(2.))
                    .rounded(px(6.))
                    .bg(theme.muted)
                    .child(option(
                        self,
                        "library-version-original",
                        t!("Original"),
                        view::format_size(original),
                        !using_compressed,
                        library::restore_original,
                        cx,
                    ))
                    .child(option(
                        self,
                        "library-version-compressed",
                        t!("Compressed"),
                        format!(
                            "{} · −{}%",
                            view::format_size(compressed),
                            saved_percent(original, compressed)
                        ),
                        using_compressed,
                        library::use_compressed,
                        cx,
                    )),
            ),
        )
    }

    fn foot_icon(
        &self,
        id: &'static str,
        icon: &'static str,
        label: &'static str,
        disabled: bool,
    ) -> gpui::Stateful<gpui::Div> {
        let theme = self.theme;
        focus_ring(
            div()
                .id(id)
                .when(!disabled, |this| this.tab_index(0))
                .flex()
                .flex_shrink_0()
                .items_center()
                .justify_center()
                .size(px(crate::theme::CONTROL_HEIGHT))
                .rounded(px(6.))
                .when(disabled, |this| this.opacity(0.5))
                .when(!disabled, |this| {
                    this.hover(move |style| style.bg(theme.accent))
                })
                .tooltip(move |_, cx| ui::Tooltip::new(&theme, label).view(cx))
                .tooltip_show_delay(ui::TOOLTIP_SHOW_DELAY)
                .child(svg().path(icon).size(px(16.)).text_color(theme.foreground)),
            theme,
        )
    }

    fn export_action(
        &self,
        id: (&'static str, usize),
        icon: &'static str,
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
                .size(px(28.))
                .rounded(px(4.))
                .hover(move |style| style.bg(theme.secondary))
                .tooltip(move |_, cx| ui::Tooltip::new(&theme, label).view(cx))
                .tooltip_show_delay(ui::TOOLTIP_SHOW_DELAY)
                .child(svg().path(icon).size(px(14.)).text_color(theme.foreground)),
            theme,
        )
    }

    fn render_detail_exports(&self, entry: &Entry, cx: &mut Context<Self>) -> Option<gpui::Div> {
        if entry.exports.is_empty() {
            return None;
        }
        let theme = self.theme;
        let clock = Clock::now();
        let rows = entry.exports.iter().enumerate().map(|(index, export)| {
            let size = export.size;
            let note: SharedString = match size {
                Some(bytes) => format!(
                    "{} · {}",
                    day_label(&clock, export.exported_millis),
                    view::format_size(bytes)
                )
                .into(),
                None => t!("Not found").into(),
            };
            let path = export.path.clone();
            let actions: Vec<gpui::AnyElement> = if size.is_some() {
                let (reveal, copy) = (path.clone(), path);
                vec![
                    self.export_action(
                        ("library-export-reveal", index),
                        "icons/folder-open.svg",
                        t!("Show in folder"),
                    )
                    .on_click(move |_, _, cx| library::reveal_path(&reveal, cx))
                    .into_any_element(),
                    self.export_action(
                        ("library-export-copy", index),
                        "icons/copy.svg",
                        t!("Copy"),
                    )
                    .on_click(move |_, _, cx| copy_files(std::slice::from_ref(&copy), cx))
                    .into_any_element(),
                ]
            } else {
                vec![
                    self.export_action(
                        ("library-export-remove", index),
                        "icons/x.svg",
                        t!("Remove from list"),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.remove_library_export(path.clone(), window, cx);
                    }))
                    .into_any_element(),
                ]
            };
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .min_h(px(40.))
                .py(px(4.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .truncate()
                                .text_size(px(13.))
                                .text_color(if size.is_some() {
                                    theme.foreground
                                } else {
                                    theme.muted_foreground
                                })
                                .child(export.name.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(if size.is_some() {
                                    theme.muted_foreground
                                } else {
                                    theme.danger_text
                                })
                                .child(note),
                        ),
                )
                .children(actions)
        });
        Some(
            div()
                .child(self.subhead(t!("Exports")))
                .children(rows.collect::<Vec<_>>()),
        )
    }

    fn render_detail_rebuilt(&self, entry: &Entry) -> Option<gpui::Div> {
        let theme = self.theme;
        let copy = entry.rebuilt.clone()?;
        Some(
            div()
                .flex()
                .items_start()
                .gap(px(8.))
                .p(px(10.))
                .rounded(px(8.))
                .bg(theme.warning_tint)
                .text_color(theme.warning_text)
                .child(
                    svg()
                        .path("icons/triangle-alert.svg")
                        .size(px(14.))
                        .mt(px(2.))
                        .flex_shrink_0()
                        .text_color(theme.warning_text),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(t!("Rebuilt from a damaged project file")),
                        )
                        .child(
                            div()
                                .mt(px(2.))
                                .text_size(px(12.))
                                .text_color(theme.foreground)
                                .child(t!(
                                    "Its recording is intact. Edits saved in the old file may be missing; FrameEver kept a copy of it."
                                )),
                        ),
                )
                .child(
                    self.export_action(
                        ("library-rebuilt-copy", 0),
                        "icons/folder-open.svg",
                        t!("Show the copy in folder"),
                    )
                    .on_click(move |_, _, cx| library::reveal_path(&copy, cx)),
                ),
        )
    }

    fn render_detail_folders(&self, entry: &Entry, cx: &mut Context<Self>) -> gpui::Div {
        let theme = self.theme;
        let file = &self.library.organizer.file;
        let holders: Vec<(String, String)> = entry.id.as_deref().map_or_else(Vec::new, |id| {
            file.folders
                .iter()
                .filter(|folder| folder.members.iter().any(|member| member == id))
                .map(|folder| (folder.id.clone(), folder.name.clone()))
                .collect()
        });
        let empty = holders.is_empty();
        let chips = holders.into_iter().enumerate().map(|(index, (id, name))| {
            let path = entry.path.clone();
            div()
                .flex()
                .items_center()
                .gap(px(4.))
                .h(px(24.))
                .pl(px(8.))
                .pr(px(4.))
                .rounded(px(12.))
                .border_1()
                .border_color(theme.border)
                .bg(theme.card)
                .text_size(px(12.))
                .child(
                    svg()
                        .path("icons/folder.svg")
                        .size(px(14.))
                        .text_color(theme.muted_foreground),
                )
                .child(name)
                .child(
                    focus_ring(
                        div()
                            .id(("library-folder-chip-remove", index))
                            .tab_index(0)
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(18.))
                            .rounded_full()
                            .cursor_pointer()
                            .hover(move |style| style.bg(theme.accent))
                            .tooltip(move |_, cx| {
                                ui::Tooltip::new(&theme, t!("Remove from this folder")).view(cx)
                            })
                            .tooltip_show_delay(ui::TOOLTIP_SHOW_DELAY)
                            .child(
                                svg()
                                    .path("icons/x.svg")
                                    .size(px(12.))
                                    .text_color(theme.muted_foreground),
                            ),
                        theme,
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.remove_from_folder(vec![path.clone()], id.clone(), window, cx);
                    })),
                )
        });
        let add = focus_ring(
            div()
                .id("library-folder-chip-add")
                .tab_index(0)
                .flex()
                .items_center()
                .gap(px(4.))
                .h(px(24.))
                .px(px(8.))
                .rounded(px(12.))
                .border_1()
                .border_dashed()
                .border_color(theme.border)
                .cursor_pointer()
                .text_size(px(12.))
                .text_color(theme.muted_foreground)
                .hover(move |style| style.bg(theme.accent))
                .child(
                    svg()
                        .path("icons/plus.svg")
                        .size(px(14.))
                        .text_color(theme.muted_foreground),
                )
                .when(empty, |this| this.child(t!("Add to folder"))),
            theme,
        )
        .on_click(cx.listener(|this, event: &gpui::ClickEvent, _, cx| {
            this.open_library_folders_menu(event.position(), cx);
        }));
        div().child(self.subhead(t!("Folders"))).child(
            div()
                .flex()
                .flex_wrap()
                .gap(px(6.))
                .children(chips)
                .child(add),
        )
    }

    pub(super) fn render_library_detail(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let theme = self.theme;
        if self.library.state.detail_hidden {
            return None;
        }
        if self.library.selection.items().len() > 1 {
            return Some(self.render_library_batch(cx).into_any_element());
        }
        let Some(entry) = self.library.single_entry().cloned() else {
            return Some(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .justify_center()
                    .w(px(DETAIL_WIDTH))
                    .min_h_0()
                    .border_l_1()
                    .border_color(theme.border)
                    .text_size(px(12.))
                    .text_color(theme.muted_foreground)
                    .child(t!("Select an item to see its details"))
                    .into_any_element(),
            );
        };
        let copyable = library::file_to_share(&entry).is_some();
        let name = ui::TextInput::bare(
            &theme,
            "library-detail-name",
            &self.library.detail.name_input,
        )
        .height(px(28.))
        .padding_x(px(6.))
        .radius(px(4.))
        .text_size(px(15.))
        .font_weight(FontWeight::SEMIBOLD);
        let tools = div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap(px(2.))
            .mt(px(-4.))
            .mx(px(-6.))
            .child(
                self.foot_icon(
                    "library-detail-copy",
                    "icons/copy.svg",
                    t!("Copy file"),
                    !copyable,
                )
                .when(copyable, |this| {
                    this.on_click(cx.listener(|this, _, _, cx| {
                        this.copy_library_file(cx);
                    }))
                }),
            )
            .child(
                self.foot_icon(
                    "library-detail-reveal",
                    "icons/folder-open.svg",
                    t!("Show in folder"),
                    false,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.reveal_library_selection(cx);
                })),
            )
            .child(
                self.foot_icon(
                    "library-detail-external",
                    "icons/external-link.svg",
                    t!("Open in default app"),
                    !copyable,
                )
                .when(copyable, |this| {
                    this.on_click(cx.listener(|this, _, _, cx| {
                        this.open_library_file_externally(cx);
                    }))
                }),
            )
            .child(div().flex_1())
            .child(
                self.foot_icon(
                    "library-detail-delete",
                    "icons/trash.svg",
                    t!("Delete…"),
                    false,
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    this.delete_library_selection(window, cx);
                })),
            );
        Some(
            div()
                .flex()
                .flex_col()
                .flex_shrink_0()
                .w(px(DETAIL_WIDTH))
                .min_h_0()
                .border_l_1()
                .border_color(theme.border)
                .child(
                    div()
                        .id("library-detail")
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .gap(px(12.))
                        .p(px(12.))
                        .child(self.render_library_preview(&entry, cx))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .child(
                                    div()
                                        .mx(px(-6.))
                                        .rounded(px(4.))
                                        .border_1()
                                        .border_color(gpui::transparent_black())
                                        .hover(move |style| style.border_color(theme.input))
                                        .child(name),
                                )
                                .child(self.render_detail_meta(&entry)),
                        )
                        .child(self.render_detail_actions(&entry, cx))
                        .child(tools)
                        .children(self.render_detail_rebuilt(&entry))
                        .children(self.render_detail_compress(&entry, cx))
                        .children(self.render_detail_versions(cx))
                        .children(self.render_detail_exports(&entry, cx))
                        .child(self.render_detail_folders(&entry, cx))
                        .children(self.render_detail_info()),
                )
                .into_any_element(),
        )
    }

    fn batch_button(
        &self,
        id: &'static str,
        icon: &'static str,
        label: impl Into<SharedString>,
        disabled: bool,
    ) -> gpui::Stateful<gpui::Div> {
        let theme = self.theme;
        focus_ring(
            div()
                .id(id)
                .when(!disabled, |this| {
                    this.tab_index(0)
                        .cursor_pointer()
                        .hover(move |style| style.bg(theme.accent))
                })
                .when(disabled, |this| this.opacity(0.5))
                .flex()
                .items_center()
                .gap(px(8.))
                .h(px(crate::theme::CONTROL_HEIGHT))
                .px(px(10.))
                .rounded(px(6.))
                .bg(theme.secondary)
                .text_size(px(13.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.foreground)
                .child(
                    svg()
                        .path(icon)
                        .size(px(16.))
                        .flex_shrink_0()
                        .text_color(theme.muted_foreground),
                )
                .child(div().min_w_0().truncate().child(label.into())),
            theme,
        )
    }

    fn render_library_batch(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = self.theme;
        let entries = self.library.selected_entries();
        let size: u64 = entries.iter().map(|entry| entry.size).sum();
        let kinds: Vec<(MediaKind, usize)> =
            [MediaKind::Studio, MediaKind::Instant, MediaKind::Screenshot]
                .into_iter()
                .map(|kind| {
                    let count = entries.iter().filter(|entry| entry.kind == kind).count();
                    (kind, count)
                })
                .filter(|(_, count)| *count > 0)
                .collect();
        let compressible = self.library_compressible().len();
        let copyable = !self.library_files_to_share().is_empty();
        div()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .w(px(DETAIL_WIDTH))
            .min_h_0()
            .border_l_1()
            .border_color(theme.border)
            .child(
                div()
                    .id("library-batch")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .gap(px(12.))
                    .p(px(12.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(px(15.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(t!("{count} selected", count = entries.len())),
                            )
                            .child(
                                div()
                                    .mt(px(2.))
                                    .text_size(px(12.))
                                    .text_color(theme.muted_foreground)
                                    .child(view::format_size(size)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .children(kinds.into_iter().map(|(kind, count)| {
                                div()
                                    .flex()
                                    .justify_between()
                                    .text_size(px(12.))
                                    .child(
                                        div()
                                            .text_color(theme.muted_foreground)
                                            .child(kind_label(kind)),
                                    )
                                    .child(count.to_string())
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child(
                                self.batch_button(
                                    "library-batch-compress",
                                    "icons/download.svg",
                                    t!("Compress {count}", count = compressible),
                                    compressible == 0,
                                )
                                .when(compressible > 0, |this| {
                                    this.on_click(cx.listener(|this, _, window, cx| {
                                        this.compress_library_selection(window, cx);
                                    }))
                                }),
                            )
                            .child(
                                self.batch_button(
                                    "library-batch-reveal",
                                    "icons/folder-open.svg",
                                    t!("Show in folder"),
                                    false,
                                )
                                .on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.reveal_library_selection(cx);
                                    },
                                )),
                            )
                            .child(
                                self.batch_button(
                                    "library-batch-copy",
                                    "icons/copy.svg",
                                    t!("Copy files"),
                                    !copyable,
                                )
                                .when(copyable, |this| {
                                    this.on_click(cx.listener(|this, _, _, cx| {
                                        this.copy_library_file(cx);
                                    }))
                                }),
                            )
                            .child(
                                self.batch_button(
                                    "library-batch-delete",
                                    "icons/trash.svg",
                                    t!("Delete…"),
                                    false,
                                )
                                .on_click(cx.listener(
                                    |this, _, window, cx| {
                                        this.delete_library_selection(window, cx);
                                    },
                                )),
                            )
                            .child(
                                self.foot_button(
                                    "library-batch-clear",
                                    t!("Clear selection"),
                                    false,
                                )
                                .flex_none()
                                .on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.clear_library_selection(cx);
                                    },
                                )),
                            ),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_meta_line_leaves_out_a_missing_duration() {
        assert_eq!(
            meta_line(
                "Instant",
                "Today 09:30",
                Some("0:48".into()),
                "38 MB".into()
            ),
            "Instant · Today 09:30 · 0:48 · 38 MB"
        );
        assert_eq!(
            meta_line("Screenshot", "Yesterday 18:02", None, "1.2 MB".into()),
            "Screenshot · Yesterday 18:02 · 1.2 MB"
        );
    }

    #[test]
    fn the_saving_is_a_share_of_the_original() {
        assert_eq!(saved_percent(38_000_000, 9_600_000), 75);
        assert_eq!(saved_percent(100, 100), 0);
        assert_eq!(saved_percent(100, 150), 0);
        assert_eq!(saved_percent(0, 0), 0);
    }

    #[test]
    fn tracks_always_start_with_the_screen() {
        assert_eq!(tracks_label(ProjectTracks::default()), "Screen");
        assert_eq!(
            tracks_label(ProjectTracks {
                camera: true,
                microphone: true,
                system_audio: true,
            }),
            "Screen · Camera · Microphone · System audio"
        );
    }

    #[test]
    fn resolution_shows_the_frame_rate_when_there_is_one() {
        assert_eq!(resolution_label(&ProjectDetails::default()), None);
        assert_eq!(
            resolution_label(&ProjectDetails {
                resolution: Some((1920, 1080)),
                fps: Some(30),
                ..ProjectDetails::default()
            })
            .as_deref(),
            Some("1920 × 1080 · 30 fps")
        );
        assert_eq!(
            resolution_label(&ProjectDetails {
                resolution: Some((640, 480)),
                ..ProjectDetails::default()
            })
            .as_deref(),
            Some("640 × 480")
        );
    }
}
