use std::{path::PathBuf, time::Duration};

use gpui::{
    Context, FocusHandle, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Task, Window, div, prelude::FluentBuilder, px,
};

use super::{LibraryWindow, app_windows, focus_ring, library, t};
use crate::library::{
    Deleted, MediaKind, Version,
    view::{Compression, Entry},
};

const UNDO_HOLD: Duration = Duration::from_secs(8);
const RECYCLE_RETRIES: u32 = 3;
const RECYCLE_RETRY_DELAY: Duration = Duration::from_millis(250);
const DIALOG_WIDTH: f32 = 420.;
const TOAST_WIDTH: f32 = 340.;
const TOAST_BOTTOM: f32 = 44.;
const CHOICES: [Option<Version>; 3] = [Some(Version::Compressed), Some(Version::Original), None];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ButtonTone {
    Plain,
    Primary,
    Danger,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Item {
    path: PathBuf,
    name: String,
}

fn has_two_versions(entry: &Entry) -> bool {
    entry.kind == MediaKind::Instant && entry.compression == Compression::CompressedCopy
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DialogFocus {
    Choices,
    Cancel,
    Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DialogAnswer {
    Cancel,
    Delete(Option<Version>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DeleteDialog {
    items: Vec<Item>,
    three_way: bool,
    choice: usize,
    focus: DialogFocus,
}

impl DeleteDialog {
    fn new(items: Vec<Item>, three_way: bool) -> Self {
        Self {
            items,
            three_way,
            choice: 0,
            focus: if three_way {
                DialogFocus::Choices
            } else {
                DialogFocus::Delete
            },
        }
    }

    fn stops(&self) -> &'static [DialogFocus] {
        if self.three_way {
            &[
                DialogFocus::Choices,
                DialogFocus::Cancel,
                DialogFocus::Delete,
            ]
        } else {
            &[DialogFocus::Cancel, DialogFocus::Delete]
        }
    }

    fn tab(&mut self, forward: bool) {
        let stops = self.stops();
        let at = stops
            .iter()
            .position(|stop| *stop == self.focus)
            .unwrap_or(0);
        let next = if forward {
            at + 1
        } else {
            at + stops.len() - 1
        };
        self.focus = stops[next % stops.len()];
    }

    fn step(&mut self, forward: bool) {
        if self.three_way {
            let count = CHOICES.len();
            self.choice = (self.choice + if forward { 1 } else { count - 1 }) % count;
            self.focus = DialogFocus::Choices;
        }
    }

    fn part(&self) -> Option<Version> {
        if self.three_way {
            CHOICES[self.choice]
        } else {
            None
        }
    }

    fn enter(&self) -> DialogAnswer {
        match self.focus {
            DialogFocus::Cancel => DialogAnswer::Cancel,
            DialogFocus::Choices | DialogFocus::Delete => DialogAnswer::Delete(self.part()),
        }
    }

    fn title(&self) -> SharedString {
        match self.items.as_slice() {
            [item] => t!("Delete “{name}”?", name = item.name).into(),
            many => t!("Delete {count} items?", count = many.len()).into(),
        }
    }
}

struct UndoToast {
    text: SharedString,
    deleted: Vec<Deleted>,
    button: FocusHandle,
    _expires: Task<()>,
}

#[derive(Default)]
pub(super) struct DeleteState {
    dialog: Option<DeleteDialog>,
    focus: Option<FocusHandle>,
    toast: Option<UndoToast>,
}

impl DeleteState {
    pub(super) fn dialog_open(&self) -> bool {
        self.dialog.is_some()
    }

    pub(super) fn toast_shown(&self) -> bool {
        self.toast.is_some()
    }
}

async fn recycle_item(
    item: &Item,
    part: Option<Version>,
    executor: &gpui::BackgroundExecutor,
) -> Result<Option<Deleted>, String> {
    match part {
        Some(version) => library::recycle::recycle_version(&item.path, version).map(Some),
        None => {
            let mut attempt = 0;
            loop {
                match library::recycle_project(&item.path) {
                    Err(_) if attempt < RECYCLE_RETRIES => {
                        attempt += 1;
                        executor.timer(RECYCLE_RETRY_DELAY).await;
                    }
                    result => return result,
                }
            }
        }
    }
}

fn deleted_text(deleted: &[Item], part: Option<Version>) -> SharedString {
    match (deleted, part) {
        ([item], Some(Version::Compressed)) => {
            t!("Deleted the compressed copy of “{name}”", name = item.name).into()
        }
        ([item], Some(Version::Original)) => {
            t!("Deleted the original of “{name}”", name = item.name).into()
        }
        ([item], None) => t!("Deleted “{name}”", name = item.name).into(),
        (many, _) => t!("Deleted {count} items", count = many.len()).into(),
    }
}

fn failure_title(failed: &[Item], total: usize) -> String {
    match failed {
        [item] if total == 1 => t!("Couldn't delete “{name}”", name = item.name),
        many => t!(
            "Couldn't delete {failed} of {total} items",
            failed = many.len(),
            total = total
        ),
    }
}

impl LibraryWindow {
    fn selected_items(&self) -> Vec<Item> {
        self.library
            .selected_entries()
            .into_iter()
            .map(|entry| Item {
                path: entry.path.clone(),
                name: entry.name.clone(),
            })
            .collect()
    }

    pub(super) fn delete_library_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let items = self.selected_items();
        if items.is_empty() {
            return;
        }
        let three_way = items.len() == 1
            && self
                .library
                .entry(&items[0].path)
                .is_some_and(has_two_versions);
        let focus = self
            .library
            .delete
            .focus
            .get_or_insert_with(|| cx.focus_handle())
            .clone();
        self.library.delete.dialog = Some(DeleteDialog::new(items, three_way));
        window.focus(&focus, cx);
        cx.notify();
    }

    pub(super) fn delete_library_now(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let items = self.selected_items();
        if !items.is_empty() {
            self.run_library_delete(items, None, window, cx);
        }
    }

    pub(super) fn step_library_dialog(&mut self, forward: bool, cx: &mut Context<Self>) {
        if let Some(dialog) = &mut self.library.delete.dialog {
            dialog.step(forward);
            cx.notify();
        }
    }

    pub(super) fn tab_library_dialog(&mut self, forward: bool, cx: &mut Context<Self>) {
        if let Some(dialog) = &mut self.library.delete.dialog {
            dialog.tab(forward);
            cx.notify();
        }
    }

    fn answer_library_delete(
        &mut self,
        answer: DialogAnswer,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(dialog) = self.library.delete.dialog.take() else {
            return;
        };
        self.close_library_dialog(cx);
        if let DialogAnswer::Delete(part) = answer {
            self.run_library_delete(dialog.items, part, window, cx);
        }
    }

    pub(super) fn enter_library_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(answer) = self.library.delete.dialog.as_ref().map(DeleteDialog::enter) {
            self.answer_library_delete(answer, window, cx);
        }
    }

    pub(super) fn close_library_dialog(&mut self, cx: &mut Context<Self>) {
        self.library.delete.dialog = None;
        self.focus_library_items(cx);
        cx.notify();
    }

    fn run_library_delete(
        &mut self,
        items: Vec<Item>,
        part: Option<Version>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if part.is_none() {
            self.library
                .hide_entries(items.iter().map(|item| &item.path));
        }
        self.clear_library_selection(cx);
        self.rebuild_library(cx);
        cx.spawn_in(window, async move |this, cx| {
            let total = items.len();
            let mut accepted = Vec::new();
            let mut refused: Vec<(Item, String)> = Vec::new();
            let mut held = Vec::new();
            let mut releases = Vec::new();
            let mut joined = Vec::new();
            let app: &gpui::AsyncApp = cx;
            for item in items {
                if part.is_some() {
                    accepted.push(item);
                    continue;
                }
                match app.update(|cx| app_windows::hold_for_library_delete(&item.path, cx)) {
                    Ok(app_windows::LibraryDeleteHold::Held { key, release }) => {
                        releases.extend(release);
                        held.push((key, item.path.clone()));
                        accepted.push(item);
                    }
                    Ok(app_windows::LibraryDeleteHold::Joined(outcome)) => {
                        joined.push((item, outcome));
                    }
                    Err(reason) => refused.push((item, reason)),
                }
            }
            let items = accepted;
            for release in releases {
                release.await;
            }
            let executor = cx.background_executor().clone();
            let (deleted, failed) = cx
                .background_executor()
                .spawn(async move {
                    let mut deleted = Vec::new();
                    let mut failed = Vec::new();
                    for item in items {
                        match recycle_item(&item, part, &executor).await {
                            Ok(Some(record)) => deleted.push((item, record)),
                            Ok(None) => {}
                            Err(error) => {
                                tracing::error!(path = %item.path.display(), "deleting the project failed: {error}");
                                failed.push(item);
                            }
                        }
                    }
                    (deleted, failed)
                })
                .await;
            let app: &gpui::AsyncApp = cx;
            app.update(|cx| {
                for (key, path) in &held {
                    let outcome = if failed.iter().any(|item: &Item| item.path == *path) {
                        Err(t!("Its files may be in use. Close what is using them and try again.").to_string())
                    } else {
                        Ok(())
                    };
                    app_windows::finish_library_delete(key, outcome, cx);
                }
            });
            for (item, outcome) in joined {
                if let Err(reason) = outcome.await.unwrap_or_else(|_| {
                    Err(t!("Its files may be in use. Close what is using them and try again.").to_string())
                }) {
                    refused.push((item, reason));
                }
            }
            this.update(cx, |this, cx| {
                let gone: Vec<PathBuf> = deleted.iter().map(|(item, _)| item.path.clone()).collect();
                this.library.compress.forget_waiting(&gone);
                cx.defer(app_windows::refresh_library_everywhere);
            })
            .ok();
            let reason = match (refused.as_slice(), failed.is_empty()) {
                ([(_, reason)], true) => reason.clone(),
                _ => t!("Its files may be in use. Close what is using them and try again.").to_string(),
            };
            let mut failed = failed;
            failed.extend(refused.into_iter().map(|(item, _)| item));
            if !failed.is_empty() {
                crate::platform::alert_dialog(&failure_title(&failed, total), &reason);
            }
            if deleted.is_empty() {
                return;
            }
            let (gone, deleted): (Vec<Item>, Vec<Deleted>) = deleted.into_iter().unzip();
            this.update_in(cx, |this, window, cx| {
                this.show_undo_toast(deleted_text(&gone, part), deleted, window, cx);
            })
            .ok();
        })
        .detach();
    }

    fn show_undo_toast(
        &mut self,
        text: SharedString,
        deleted: Vec<Deleted>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let button = cx.focus_handle().tab_index(0).tab_stop(true);
        let held = button.clone();
        let expires = cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(UNDO_HOLD).await;
            this.update_in(cx, |this, window, cx| {
                if held.is_focused(window) {
                    this.focus_library_items(cx);
                }
                this.library.delete.toast = None;
                cx.notify();
            })
            .ok();
        });
        self.library.delete.toast = Some(UndoToast {
            text,
            deleted,
            button,
            _expires: expires,
        });
        cx.notify();
    }

    pub(super) fn undo_library_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(toast) = self.library.delete.toast.take() else {
            return;
        };
        self.focus_library_items(cx);
        cx.notify();
        let deleted = toast.deleted;
        cx.spawn_in(window, async move |this, cx| {
            let restored = cx
                .background_executor()
                .spawn(async move { library::recycle::undo(&deleted) })
                .await;
            this.update(cx, |_, cx| {
                cx.defer(app_windows::refresh_library_everywhere);
            })
            .ok();
            if let Err(error) = restored {
                tracing::error!("restoring from the Recycle Bin failed: {error}");
                crate::platform::alert_dialog(
                    t!("Couldn't restore what you deleted"),
                    t!("Open the Recycle Bin and restore it from there."),
                );
            }
        })
        .detach();
    }

    pub(super) fn dialog_button(
        &self,
        id: &'static str,
        label: impl Into<SharedString>,
        tone: ButtonTone,
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
            .map(|this| match tone {
                ButtonTone::Plain => this
                    .bg(theme.secondary)
                    .text_color(theme.foreground)
                    .when(!disabled, |this| {
                        this.hover(move |style| style.bg(theme.secondary_hover))
                    }),
                ButtonTone::Primary => this
                    .bg(theme.primary)
                    .text_color(theme.primary_foreground)
                    .when(!disabled, |this| {
                        this.hover(move |style| style.bg(theme.primary_hover))
                    }),
                ButtonTone::Danger => this
                    .bg(theme.danger)
                    .text_color(theme.danger_foreground)
                    .when(!disabled, |this| {
                        this.hover(move |style| style.bg(theme.danger_hover))
                    }),
            })
            .child(label.into())
    }

    fn render_delete_choice(
        &self,
        index: usize,
        title: &'static str,
        note: &'static str,
        dialog: &DeleteDialog,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let theme = self.theme;
        let on = dialog.choice == index;
        let focused = on && dialog.focus == DialogFocus::Choices;
        div()
            .id(("library-delete-choice", index))
            .flex()
            .gap(px(10.))
            .p(px(10.))
            .rounded(px(8.))
            .border_1()
            .cursor_pointer()
            .border_color(if focused {
                theme.ring
            } else if on {
                theme.selection
            } else {
                theme.border
            })
            .when(on, |this| this.bg(theme.selection))
            .on_click(cx.listener(move |this, _, _, cx| {
                if let Some(dialog) = this.library.delete.dialog.as_mut() {
                    dialog.choice = index;
                    dialog.focus = DialogFocus::Choices;
                }
                cx.notify();
            }))
            .child(
                div()
                    .flex_shrink_0()
                    .mt(px(2.))
                    .size(px(16.))
                    .rounded_full()
                    .border_1()
                    .border_color(theme.input)
                    .when(on, |this| {
                        this.border(px(5.))
                            .border_color(theme.primary)
                            .bg(theme.primary_foreground)
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .text_size(px(13.))
                    .text_color(if on {
                        theme.selection_foreground
                    } else {
                        theme.foreground
                    })
                    .child(title)
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(theme.muted_foreground)
                            .child(note),
                    ),
            )
    }

    pub(super) fn render_library_dialog(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let theme = self.theme;
        let dialog = self.library.delete.dialog.as_ref()?;
        let focus = self.library.delete.focus.as_ref()?;
        Some(
            div()
                .id("library-delete-scrim")
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
                                .child(dialog.title()),
                        )
                        .child(
                            div()
                                .mt(px(6.))
                                .mb(px(12.))
                                .text_size(px(13.))
                                .text_color(theme.muted_foreground)
                                .child(t!("Files go to the Recycle Bin.")),
                        )
                        .when(dialog.three_way, |this| {
                            this.child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.))
                                    .mb(px(12.))
                                    .child(self.render_delete_choice(
                                        0,
                                        t!("Only the compressed copy"),
                                        t!("The original stays."),
                                        dialog,
                                        cx,
                                    ))
                                    .child(self.render_delete_choice(
                                        1,
                                        t!("Only the original"),
                                        t!("The compressed copy stays."),
                                        dialog,
                                        cx,
                                    ))
                                    .child(self.render_delete_choice(
                                        2,
                                        t!("The whole project"),
                                        t!("Original and compressed copy."),
                                        dialog,
                                        cx,
                                    )),
                            )
                        })
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(theme.muted_foreground)
                                .child(t!("Exports stay where they are.")),
                        )
                        .child(
                            div()
                                .flex()
                                .justify_end()
                                .gap(px(8.))
                                .mt(px(16.))
                                .child(
                                    self.dialog_button(
                                        "library-delete-cancel",
                                        t!("Cancel"),
                                        ButtonTone::Plain,
                                        dialog.focus == DialogFocus::Cancel,
                                        false,
                                    )
                                    .on_click(cx.listener(
                                        |this, _, _, cx| {
                                            this.close_library_dialog(cx);
                                        },
                                    )),
                                )
                                .child(
                                    self.dialog_button(
                                        "library-delete-confirm",
                                        t!("Delete"),
                                        ButtonTone::Danger,
                                        dialog.focus == DialogFocus::Delete,
                                        false,
                                    )
                                    .on_click(cx.listener(
                                        |this, _, window, cx| {
                                            let answer =
                                                this.library.delete.dialog.as_ref().map(|dialog| {
                                                    DialogAnswer::Delete(dialog.part())
                                                });
                                            if let Some(answer) = answer {
                                                this.answer_library_delete(answer, window, cx);
                                            }
                                        },
                                    )),
                                ),
                        ),
                ),
        )
    }

    pub(super) fn render_library_undo(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let theme = self.theme;
        let toast = self.library.delete.toast.as_ref()?;
        Some(
            div()
                .absolute()
                .bottom(px(TOAST_BOTTOM))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .px(px(16.))
                .child(
                    div()
                        .id("library-undo-toast")
                        .occlude()
                        .flex()
                        .items_center()
                        .gap(px(10.))
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
                        .child(focus_ring(
                            div()
                                .id("library-undo")
                                .track_focus(&toast.button)
                                .flex()
                                .flex_shrink_0()
                                .items_center()
                                .h(px(28.))
                                .px(px(10.))
                                .rounded(px(6.))
                                .cursor_pointer()
                                .font_weight(FontWeight::MEDIUM)
                                .hover(move |style| style.bg(theme.accent))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.undo_library_delete(window, cx);
                                }))
                                .child(t!("Undo")),
                            theme,
                        )),
                ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items(count: usize) -> Vec<Item> {
        (0..count)
            .map(|index| Item {
                path: PathBuf::from(format!("{index}.ever")),
                name: format!("Demo {index}"),
            })
            .collect()
    }

    #[test]
    fn enter_deletes_the_whole_project_when_there_is_no_choice() {
        let dialog = DeleteDialog::new(items(1), false);
        assert_eq!(dialog.enter(), DialogAnswer::Delete(None));
    }

    #[test]
    fn the_three_way_choice_starts_on_the_compressed_copy_and_arrows_wrap() {
        let mut dialog = DeleteDialog::new(items(1), true);
        assert_eq!(
            dialog.enter(),
            DialogAnswer::Delete(Some(Version::Compressed))
        );
        dialog.step(true);
        assert_eq!(
            dialog.enter(),
            DialogAnswer::Delete(Some(Version::Original))
        );
        dialog.step(true);
        assert_eq!(dialog.enter(), DialogAnswer::Delete(None));
        dialog.step(true);
        assert_eq!(
            dialog.enter(),
            DialogAnswer::Delete(Some(Version::Compressed))
        );
        dialog.step(false);
        assert_eq!(dialog.enter(), DialogAnswer::Delete(None));
    }

    #[test]
    fn tab_stays_inside_the_dialog_and_enter_on_cancel_cancels() {
        let mut dialog = DeleteDialog::new(items(1), true);
        dialog.tab(true);
        assert_eq!(dialog.enter(), DialogAnswer::Cancel);
        dialog.tab(true);
        assert_eq!(
            dialog.enter(),
            DialogAnswer::Delete(Some(Version::Compressed))
        );
        dialog.tab(true);
        assert_eq!(dialog.focus, DialogFocus::Choices);
        dialog.tab(false);
        assert_eq!(dialog.focus, DialogFocus::Delete);

        let mut plain = DeleteDialog::new(items(2), false);
        plain.tab(true);
        assert_eq!(plain.enter(), DialogAnswer::Cancel);
        plain.tab(true);
        assert_eq!(plain.enter(), DialogAnswer::Delete(None));
    }

    #[test]
    fn arrows_do_nothing_without_a_choice_to_make() {
        let mut dialog = DeleteDialog::new(items(1), false);
        dialog.step(true);
        assert_eq!(dialog.focus, DialogFocus::Delete);
        assert_eq!(dialog.enter(), DialogAnswer::Delete(None));
    }

    #[test]
    fn the_text_names_one_item_and_counts_several() {
        assert_eq!(
            DeleteDialog::new(items(1), false).title(),
            "Delete “Demo 0”?"
        );
        assert_eq!(
            DeleteDialog::new(items(3), false).title(),
            "Delete 3 items?"
        );
        assert_eq!(deleted_text(&items(1), None), "Deleted “Demo 0”");
        assert_eq!(
            deleted_text(&items(1), Some(Version::Compressed)),
            "Deleted the compressed copy of “Demo 0”"
        );
        assert_eq!(deleted_text(&items(2), None), "Deleted 2 items");
    }

    #[test]
    fn a_failed_delete_names_the_item_or_counts_what_failed() {
        assert_eq!(failure_title(&items(1), 1), "Couldn't delete “Demo 0”");
        assert_eq!(failure_title(&items(2), 5), "Couldn't delete 2 of 5 items");
    }
}
