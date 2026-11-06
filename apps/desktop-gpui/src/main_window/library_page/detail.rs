use std::path::{Path, PathBuf};

use gpui::{
    AnyElement, Context, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, Task, Window, div, prelude::FluentBuilder,
    px, svg,
};

use super::{
    Clock, DETAIL_WIDTH, JobState, MainWindow, app_windows, day_label, focus_ring, kind_label,
    library, preview, t, ui, view,
};
use crate::library::{
    MediaKind, ProjectDetails, ProjectTracks, VideoVersions, compress_queue::QueueSummary,
    view::Entry,
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

fn saved_size(original: u64, compressed: u64) -> String {
    view::format_size(original.saturating_sub(compressed))
}

fn alert_copy_failed(error: String, cx: &mut gpui::App) {
    tracing::warn!("copying the file failed: {error}");
    cx.spawn(async move |_| {
        crate::platform::alert_dialog(
            t!("Couldn't copy the file"),
            t!("It may have been moved. Try again."),
        );
    })
    .detach();
}

fn copy_files(files: &[PathBuf], cx: &mut gpui::App) {
    if let Err(error) = crate::platform::copy_files_to_clipboard(files, cx) {
        alert_copy_failed(error, cx);
    }
}

fn report_compression(summary: &QueueSummary) {
    if crate::store::GeneralSettings::load().enable_notifications {
        if let Some(notice) = crate::auto_compress::queue_notice(summary) {
            crate::auto_compress::show(notice);
        }
    } else if !summary.failed.is_empty() {
        crate::platform::alert_dialog(
            t!("Couldn't compress the recording"),
            t!("Check the folder has space and try again."),
        );
    }
}

impl MainWindow {
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

    pub(in crate::main_window) fn rename_library_selection(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.library.selection.single().is_some() {
            self.library
                .detail
                .name_input
                .update(cx, |input, cx| input.focus_and_select_all(window, cx));
        }
    }

    pub(in crate::main_window) fn on_library_name_event(
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
        let main = cx.global::<app_windows::AppWindows>().main;
        cx.defer(move |cx| {
            main.update(cx, |_, window, cx| window.focus(&focus, cx))
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

    pub(in crate::main_window) fn copy_library_file(&self, cx: &mut Context<Self>) {
        let files = self.library_files_to_copy();
        if !files.is_empty() {
            copy_files(&files, cx);
        }
    }

    pub(super) fn library_files_to_copy(&self) -> Vec<PathBuf> {
        library::files_to_share(self.library.selected_entries())
    }

    pub(super) fn library_compressible(&self) -> Vec<PathBuf> {
        self.library
            .selected_entries()
            .into_iter()
            .filter(|entry| entry.compressible)
            .filter(|entry| self.library.compress.state(&entry.path).is_none())
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
                        &frameever_utils::storage::ExportRecord {
                            path: dest,
                            format: "mp4".to_string(),
                            codec: None,
                            exported_at: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs(),
                        },
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

    pub(super) fn compress_library_selection(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let projects = self.library_compressible();
        self.library.compress.add(projects);
        cx.notify();
        self.run_library_compression(window, cx);
    }

    fn run_library_compression(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.library.compress.start_next() else {
            return;
        };
        let (progress_tx, progress_rx) = flume::unbounded::<u64>();
        cx.spawn_in(window, async move |this, cx| {
            let bundle = path.clone();
            let job = cx.background_executor().spawn(async move {
                library::compress_video(&bundle, |done, total| {
                    progress_tx.send(done * 100 / total.max(1)).is_ok()
                })
                .map(|_| library::video_versions(&bundle).map_or(0, VideoVersions::saved))
            });
            while let Ok(percent) = progress_rx.recv_async().await {
                let open = this.update(cx, |this, cx| {
                    this.library.compress.progress(percent);
                    cx.notify();
                });
                if open.is_err() {
                    break;
                }
            }
            let result = job.await;
            if let Err(error) = &result {
                tracing::error!(path = %path.display(), "compressing the recording failed: {error}");
            }
            this.update_in(cx, |this, window, cx| {
                if let Some(summary) = this.library.compress.finish(result) {
                    cx.background_executor()
                        .spawn(async move {
                            report_compression(&summary);
                        })
                        .detach();
                }
                this.refresh_library(window, cx);
                this.run_library_compression(window, cx);
            })
            .ok();
        })
        .detach();
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
            this.update_in(cx, |this, window, cx| this.refresh_library(window, cx))
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
                            this.hover(move |style| style.bg(theme.accent))
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
                            let copied = crate::platform::copy_image_to_clipboard(png.clone(), cx);
                            cx.spawn(async move |cx| {
                                if let Err(error) = copied.await {
                                    cx.update(|cx| alert_copy_failed(error, cx));
                                }
                            })
                            .detach();
                        }),
                )
            }
            (None, MediaKind::Instant) => {
                let job = self.library.compress.state(&entry.path);
                let label: SharedString = match job {
                    Some(JobState::Running { percent }) => {
                        t!("Compressing… {percent}%", percent = percent).into()
                    }
                    Some(JobState::Waiting) => t!("Waiting").into(),
                    None => t!("Compress").into(),
                };
                let idle = entry.compressible && job.is_none();
                row.child(export(self, true, cx)).child(
                    self.detail_button("library-detail-compress", label, false, !idle)
                        .when(idle, |this| {
                            this.on_click(cx.listener(|this, _, window, cx| {
                                this.compress_library_selection(window, cx);
                            }))
                        }),
                )
            }
            _ => row.child(open_button).child(export(self, false, cx)),
        }
    }

    fn render_detail_info(&self, entry: &Entry) -> gpui::Div {
        let theme = self.theme;
        let details = &self.library.detail.details;
        let clock = Clock::now();
        let created = format!(
            "{} {}",
            day_label(&clock, entry.created_millis),
            clock.time_of(entry.created_millis)
        );
        let mut rows: Vec<(&'static str, String)> = vec![
            (t!("Type"), kind_label(entry.kind).to_string()),
            (t!("Created"), created),
        ];
        if let Some(duration) = entry.duration {
            rows.push((t!("Duration"), view::format_clock(duration)));
        }
        if let Some(resolution) = resolution_label(details) {
            rows.push((t!("Resolution"), resolution));
        }
        rows.push((t!("Size"), view::format_size(entry.size)));
        if let Some(tracks) = details.tracks {
            rows.push((t!("Tracks"), tracks_label(tracks)));
        }
        div()
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
            }))
    }

    fn subhead(&self, label: &'static str) -> gpui::Div {
        div()
            .mb(px(6.))
            .text_size(px(12.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(self.theme.muted_foreground)
            .child(label)
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
                      size: u64,
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
                    .child(
                        div()
                            .text_color(theme.muted_foreground)
                            .child(view::format_size(size)),
                    ),
                this.theme,
            )
            .when(!on, |this| {
                this.on_click(cx.listener(move |this, _, window, cx| {
                    this.switch_library_version(change, window, cx);
                }))
            })
        };
        Some(
            div()
                .child(self.subhead(t!("Version")))
                .child(
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
                            original,
                            !using_compressed,
                            library::restore_original,
                            cx,
                        ))
                        .child(option(
                            self,
                            "library-version-compressed",
                            t!("Compressed"),
                            compressed,
                            using_compressed,
                            library::use_compressed,
                            cx,
                        )),
                )
                .child(
                    div()
                        .mt(px(6.))
                        .text_size(px(12.))
                        .text_color(theme.muted_foreground)
                        .child(t!("Saves {size}", size = saved_size(original, compressed))),
                ),
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

    pub(super) fn render_library_detail(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let theme = self.theme;
        if self.library.selection.items().len() > 1 {
            return Some(self.render_library_batch(cx).into_any_element());
        }
        let entry = self.library.single_entry()?.clone();
        let copyable = library::file_to_copy(&entry).is_some();
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
                                .mx(px(-6.))
                                .rounded(px(4.))
                                .border_1()
                                .border_color(gpui::transparent_black())
                                .hover(move |style| style.border_color(theme.input))
                                .child(name),
                        )
                        .child(self.render_detail_actions(&entry, cx))
                        .child(self.render_detail_info(&entry))
                        .children(self.render_detail_versions(cx))
                        .children(self.render_detail_exports(&entry, cx)),
                )
                .child(
                    div()
                        .flex()
                        .flex_shrink_0()
                        .gap(px(4.))
                        .p(px(8.))
                        .border_t_1()
                        .border_color(theme.border)
                        .child(
                            self.foot_button("library-detail-reveal", t!("Show in folder"), false)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.reveal_library_selection(cx);
                                })),
                        )
                        .child(
                            self.foot_button("library-detail-copy", t!("Copy file"), !copyable)
                                .when(copyable, |this| {
                                    this.on_click(cx.listener(|this, _, _, cx| {
                                        this.copy_library_file(cx);
                                    }))
                                }),
                        )
                        .child(
                            self.foot_button("library-detail-delete", t!("Delete…"), false)
                                .text_color(theme.danger_text)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.delete_library_selection(window, cx);
                                })),
                        ),
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
        let copyable = !self.library_files_to_copy().is_empty();
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
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .p(px(8.))
                    .border_t_1()
                    .border_color(theme.border)
                    .child(
                        self.foot_button("library-batch-clear", t!("Clear selection"), false)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.clear_library_selection(cx);
                            })),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
