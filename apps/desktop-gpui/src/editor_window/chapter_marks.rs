use super::*;
use frameever_utils::storage::ChapterMarker;

const PANEL_WIDTH: f32 = 320.;
const PANEL_LIST_HEIGHT: f32 = 240.;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ChapterMarksPanel {
    left: Pixels,
    bottom: Pixels,
    renaming: Option<usize>,
}

impl EditorWindow {
    pub(crate) fn add_chapter_mark_at_playhead(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let time = self.view.preview_time.unwrap_or(self.playhead);
        let Some(recorded) =
            timeline::recorded_time_at(time, &self.clip_display_durations, &self.timeline.clips)
        else {
            self.show_notice(t!("No recording under the playhead"), window, cx);
            return;
        };
        if self.chapter_marks.add(recorded).is_none() {
            self.show_notice(t!("A chapter mark is already here"), window, cx);
            return;
        }
        self.chapter_marks_changed("chapter-mark-add", cx);
    }

    pub(crate) fn rename_chapter_mark(&mut self, index: usize, name: &str, cx: &mut Context<Self>) {
        if self.chapter_marks.rename(index, name) {
            self.chapter_marks_changed("chapter-mark-rename", cx);
        }
    }

    pub(crate) fn move_chapter_mark_to_playhead(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(recorded) = timeline::recorded_time_at(
            self.playhead,
            &self.clip_display_durations,
            &self.timeline.clips,
        ) else {
            self.show_notice(t!("No recording under the playhead"), window, cx);
            return;
        };
        if self.chapter_marks.move_to(index, recorded).is_none() {
            self.show_notice(t!("A chapter mark is already here"), window, cx);
            return;
        }
        self.chapter_marks_changed("chapter-mark-move", cx);
    }

    pub(crate) fn delete_chapter_mark(&mut self, index: usize, cx: &mut Context<Self>) {
        if !self.chapter_marks.delete(index) {
            return;
        }
        if let Some(panel) = self.chapter_marks_panel.as_mut() {
            panel.renaming = None;
        }
        self.chapter_marks_changed("chapter-mark-delete", cx);
    }

    pub(crate) fn restore_chapter_mark(&mut self, cx: &mut Context<Self>) {
        if self.chapter_marks.restore_deleted().is_some() {
            self.chapter_marks_changed("chapter-mark-restore", cx);
        }
    }

    pub(crate) fn split_at_chapter_marks(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let times = self.chapter_mark_times();
        if self.edit(
            |timeline| edits::split_clips_at(timeline, &times),
            window,
            cx,
        ) {
            self.note_edit("split-at-chapter-marks", Some(TrackKind::Clip));
        } else {
            self.show_notice(t!("No clip to split at the chapter marks"), window, cx);
        }
    }

    fn chapter_marks_changed(&mut self, reason: &'static str, cx: &mut Context<Self>) {
        tracing::info!(
            reason,
            marks = ?self.chapter_marks.recorded_times(),
            "chapter marks changed"
        );
        let project = self.project_path.clone();
        let saved = self.chapter_marks_saved.clone();
        *saved
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            self.chapter_marks.list().to_vec();
        cx.background_executor()
            .spawn(async move {
                let marks = saved
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if let Err(error) = frameever_utils::storage::write_chapter_markers(&project, &marks)
                {
                    tracing::error!(dir = %project.display(), "could not save chapter marks: {error}");
                }
            })
            .detach();
        self.timeline_view.update(cx, |_, cx| cx.notify());
        cx.notify();
    }

    pub(crate) fn toggle_chapter_marks_panel(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.chapter_marks_panel.take().is_some() {
            cx.notify();
            return;
        }
        self.add_track = None;
        self.toolbar_menu = None;
        let viewport = window.viewport_size();
        let timeline_height = self
            .clamp_timeline_height(self.preferred_timeline_height(), f32::from(viewport.height));
        self.chapter_marks_panel = Some(ChapterMarksPanel {
            left: px(TIMELINE_SLOT_PADDING + TIMELINE_CARD_BORDER + TIMELINE_PADDING),
            bottom: px(timeline_height + 4.),
            renaming: None,
        });
        cx.notify();
    }

    fn start_chapter_mark_rename(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(mark) = self.chapter_marks.list().get(index) else {
            return;
        };
        let name = chapter_mark_name(mark, index);
        let Some(panel) = self.chapter_marks_panel.as_mut() else {
            return;
        };
        panel.renaming = Some(index);
        self.chapter_mark_name_input.update(cx, |input, cx| {
            input.set_text(name, cx);
            input.focus_and_select_all(window, cx);
        });
        cx.notify();
    }

    pub(crate) fn finish_chapter_mark_rename(
        &mut self,
        commit: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self
            .chapter_marks_panel
            .as_mut()
            .and_then(|panel| panel.renaming.take())
        else {
            return;
        };
        if commit {
            let name = self.chapter_mark_name_input.read(cx).text().to_string();
            let unchanged_number = self
                .chapter_marks
                .list()
                .get(index)
                .is_some_and(|mark| mark.name.is_none() && name.trim() == numbered_name(index));
            if !unchanged_number {
                self.rename_chapter_mark(index, &name, cx);
            }
        }
        let focus = self.focus.clone();
        window.focus(&focus, cx);
        cx.notify();
    }

    pub(crate) fn render_chapter_marks_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let count = self.chapter_marks.list().len();
        let label: SharedString = if count == 0 {
            t!("Chapter marks").into()
        } else {
            t!("Chapter marks · {count}", count = count).into()
        };
        ui::EditorButton::plain(&theme, "timeline-chapter-marks")
            .left_icon("icons/flag.svg")
            .label(label)
            .pressed(self.chapter_marks_panel.is_some())
            .disabled(!self.project_ready())
            .on_click(
                cx.listener(|this, _, window, cx| this.toggle_chapter_marks_panel(window, cx)),
            )
    }

    pub(crate) fn render_chapter_marks_panel(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let panel = self.chapter_marks_panel?;
        let theme = self.theme;
        let editor = theme.editor;
        let marks = self.chapter_marks.list();
        let rows: Vec<gpui::AnyElement> = marks
            .iter()
            .enumerate()
            .map(|(index, mark)| {
                let time = timeline::chapter_mark_time(
                    mark.recorded_at,
                    &self.clip_display_durations,
                    &self.timeline.clips,
                );
                self.render_chapter_mark_row(index, mark, time, panel.renaming == Some(index), cx)
            })
            .collect();
        let deleted = self.chapter_marks.deleted().map(|mark| {
            let name = mark
                .name
                .clone()
                .unwrap_or_else(|| t!("Chapter mark").to_string());
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(8.))
                .h(px(crate::theme::ROW_HEIGHT))
                .px(px(12.))
                .border_t_1()
                .border_color(Hsla::from(editor.line))
                .text_size(px(12.))
                .text_color(Hsla::from(editor.text_2))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .child(SharedString::from(t!("Deleted “{name}”", name = name))),
                )
                .child(
                    ui::EditorButton::plain(&theme, "chapter-mark-undo")
                        .label(t!("Undo"))
                        .on_click(cx.listener(|this, _, _, cx| this.restore_chapter_mark(cx))),
                )
        });
        let body: gpui::AnyElement = if marks.is_empty() {
            div()
                .px(px(12.))
                .py(px(16.))
                .text_size(px(12.))
                .text_color(Hsla::from(editor.text_2))
                .child(t!(
                    "No chapter marks yet. Press M to add one at the playhead."
                ))
                .into_any_element()
        } else {
            div()
                .id("chapter-marks-list")
                .max_h(px(PANEL_LIST_HEIGHT))
                .overflow_y_scroll()
                .py(px(4.))
                .children(rows)
                .into_any_element()
        };
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .h(px(36.))
            .pl(px(12.))
            .pr(px(6.))
            .border_b_1()
            .border_color(Hsla::from(editor.line))
            .child(
                div()
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(Hsla::from(editor.text_1))
                    .child(t!("Chapter marks")),
            )
            .child(
                div()
                    .id("chapter-mark-add-tooltip")
                    .tooltip(move |_, cx| {
                        ui::Tooltip::new(&theme, t!("Add chapter mark"))
                            .keys(["M"])
                            .view(cx)
                    })
                    .child(
                        ui::IconButton::new(
                            "chapter-mark-add",
                            "icons/plus.svg",
                            t!("Add chapter mark"),
                        )
                        .size(px(24.))
                        .icon_size(px(16.))
                        .color(Hsla::from(editor.text_2))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.add_chapter_mark_at_playhead(window, cx)
                        })),
                    ),
            );
        let footer = div()
            .flex()
            .items_center()
            .h(px(40.))
            .px(px(6.))
            .border_t_1()
            .border_color(Hsla::from(editor.line))
            .child(
                ui::EditorButton::plain(&theme, "chapter-marks-split")
                    .left_icon("icons/scissors.svg")
                    .label(t!("Split at marks"))
                    .disabled(marks.is_empty())
                    .on_click(
                        cx.listener(|this, _, window, cx| this.split_at_chapter_marks(window, cx)),
                    ),
            );
        Some(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .child(
                    div()
                        .id("chapter-marks-backdrop")
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .occlude()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.chapter_marks_panel = None;
                            cx.notify();
                        })),
                )
                .child(
                    div()
                        .id("chapter-marks-panel")
                        .absolute()
                        .left(panel.left)
                        .bottom(panel.bottom)
                        .w(px(PANEL_WIDTH))
                        .occlude()
                        .flex()
                        .flex_col()
                        .rounded(px(12.))
                        .border_1()
                        .border_color(Hsla::from(editor.line))
                        .bg(Hsla::from(editor.card))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(header)
                        .child(body)
                        .children(deleted)
                        .child(footer),
                )
                .into_any_element(),
        )
    }

    fn render_chapter_mark_row(
        &self,
        index: usize,
        mark: &ChapterMarker,
        time: Option<f64>,
        renaming: bool,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let theme = self.theme;
        let editor = theme.editor;
        let time_label: SharedString = match time {
            Some(time) => timeline::format_time(time).into(),
            None => t!("Cut from the timeline").into(),
        };
        let name: gpui::AnyElement = if renaming {
            div()
                .flex_1()
                .min_w_0()
                .child(
                    ui::TextInput::plain(
                        &theme,
                        "chapter-mark-name",
                        &self.chapter_mark_name_input,
                    )
                    .padding_x(px(8.))
                    .height(px(26.))
                    .radius(px(6.))
                    .text_size(px(12.)),
                )
                .into_any_element()
        } else {
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(12.))
                .text_color(Hsla::from(editor.text_1))
                .child(SharedString::from(chapter_mark_name(mark, index)))
                .into_any_element()
        };
        let action =
            |id: &'static str,
             icon: &'static str,
             label: &'static str,
             on_click: Box<dyn Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App)>| {
                div()
                    .id(SharedString::from(format!("{id}-{index}")))
                    .tooltip(move |_, cx| ui::Tooltip::new(&theme, label).view(cx))
                    .child(
                        ui::IconButton::new(
                            SharedString::from(format!("{id}-icon-{index}")),
                            icon,
                            label,
                        )
                        .size(px(24.))
                        .icon_size(px(14.))
                        .color(Hsla::from(editor.text_2))
                        .on_click(on_click),
                    )
            };
        div()
            .id(SharedString::from(format!("chapter-mark-{index}")))
            .role(gpui::Role::Button)
            .aria_label(SharedString::from(chapter_mark_name(mark, index)))
            .flex()
            .items_center()
            .gap(px(8.))
            .h(px(32.))
            .pl(px(12.))
            .pr(px(6.))
            .when_some(time.filter(|_| !renaming), |row, time| {
                row.cursor_pointer()
                    .hover(|style| style.bg(Hsla::from(editor.ctl_hover)))
                    .on_click(cx.listener(move |this, _, _, cx| this.jump_to(time, cx)))
            })
            .child(
                div()
                    .flex_none()
                    .min_w(px(40.))
                    .text_size(px(12.))
                    .text_color(Hsla::from(editor.text_2))
                    .child(time_label),
            )
            .child(name)
            .when(!renaming, |row| {
                row.child(action(
                    "chapter-mark-rename",
                    "icons/pencil.svg",
                    t!("Rename"),
                    Box::new(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.start_chapter_mark_rename(index, window, cx);
                    })),
                ))
                .child(action(
                    "chapter-mark-move",
                    "icons/arrow-left-right.svg",
                    t!("Move to playhead"),
                    Box::new(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.move_chapter_mark_to_playhead(index, window, cx);
                    })),
                ))
                .child(action(
                    "chapter-mark-delete",
                    "icons/trash.svg",
                    t!("Delete"),
                    Box::new(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.delete_chapter_mark(index, cx);
                    })),
                ))
            })
            .into_any_element()
    }
}

fn numbered_name(index: usize) -> String {
    t!("Chapter mark {number}", number = index + 1)
}

fn chapter_mark_name(mark: &ChapterMarker, index: usize) -> String {
    mark.name.clone().unwrap_or_else(|| numbered_name(index))
}

#[cfg(test)]
mod tests {
    use super::*;
    use frameever_utils::storage::{ChapterMarker, chapter_markers, save_chapter_markers};

    #[gpui::test]
    fn chapter_marks_are_added_named_moved_split_at_and_deleted_in_the_editor(
        cx: &mut gpui::TestAppContext,
    ) {
        let project = tempfile::tempdir().unwrap();
        let path = project.path().to_path_buf();
        save_chapter_markers(&path, &[Duration::from_secs(9)]).unwrap();
        let opened = path.clone();
        let (editor, cx) =
            cx.add_window_view(move |window, cx| EditorWindow::new(opened.clone(), window, cx));
        editor.update_in(cx, |editor, window, cx| {
            editor.set_error("chapter marks fixture".into(), window, cx);
            editor.project.timeline = Some(
                serde_json::from_value(serde_json::json!({
                    "segments": [
                        { "recordingSegment": 0, "timescale": 1.0, "start": 0.0, "end": 10.0 },
                        { "recordingSegment": 1, "timescale": 1.0, "start": 0.0, "end": 8.0 }
                    ],
                    "zoomSegments": []
                }))
                .unwrap(),
            );
            editor.timeline = timeline::TimelineModel::build(&editor.project, false, true);
            editor.clip_display_durations = vec![10.0, 8.0];
            editor.chapter_marks =
                crate::editor_chapter_marks::ChapterMarks::new(chapter_markers(&path));

            editor.playhead = 4.0;
            editor.add_chapter_mark_at_playhead(window, cx);
            assert_eq!(editor.chapter_marks.recorded_times(), vec![4.0, 9.0]);
            editor.add_chapter_mark_at_playhead(window, cx);
            assert_eq!(editor.chapter_marks.recorded_times(), vec![4.0, 9.0]);
            assert!(editor.notice.is_some());

            editor.rename_chapter_mark(0, "Intro", cx);
            editor.playhead = 14.0;
            editor.move_chapter_mark_to_playhead(1, window, cx);
            assert_eq!(editor.chapter_mark_times(), vec![4.0, 14.0]);

            editor.split_at_chapter_marks(window, cx);
            let segments = &editor.project.timeline.as_ref().unwrap().segments;
            assert_eq!(segments.len(), 4);

            editor.delete_chapter_mark(0, cx);
            assert_eq!(editor.chapter_marks.recorded_times(), vec![14.0]);
            editor.restore_chapter_mark(cx);
        });
        cx.run_until_parked();

        assert_eq!(
            chapter_markers(&path),
            vec![
                ChapterMarker {
                    recorded_at: 4.0,
                    name: Some("Intro".into()),
                },
                ChapterMarker {
                    recorded_at: 14.0,
                    name: None,
                },
            ]
        );
    }
}
