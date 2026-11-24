use std::path::{Path, PathBuf};

use gpui::{
    AppContext as _, Context, Empty, IntoElement, ParentElement, Pixels, Point, Render,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};
#[cfg(not(target_os = "windows"))]
use gpui::{ExternalDragPayload, FileDragPaths};

use super::LibraryWindow;
use crate::{
    i18n::t,
    library::{self, ShareGap},
    platform,
    theme::Theme,
};

pub(super) struct DraggedItem(PathBuf);

struct DragNotice(Option<(SharedString, Theme)>);

impl Render for DragNotice {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let Some((reason, theme)) = self.0.clone() else {
            return Empty.into_any_element();
        };
        div()
            .ml(px(12.))
            .mt(px(12.))
            .py(px(6.))
            .px(px(10.))
            .rounded(px(8.))
            .border_1()
            .border_color(theme.border)
            .bg(theme.popover)
            .text_size(px(12.))
            .text_color(theme.foreground)
            .child(reason)
            .into_any_element()
    }
}

impl LibraryWindow {
    pub(super) fn library_item_drag(
        &self,
        item: gpui::Stateful<gpui::Div>,
        path: &Path,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let start = cx.entity().downgrade();
        #[cfg(not(target_os = "windows"))]
        let offer = cx.entity().downgrade();
        let item = item.on_drag(
            DraggedItem(path.to_path_buf()),
            move |dragged: &DraggedItem, _: Point<Pixels>, window, cx| {
                let notice = start
                    .update(cx, |this, cx| {
                        this.start_library_drag(&dragged.0, window, cx)
                    })
                    .ok()
                    .flatten();
                cx.new(|_| DragNotice(notice))
            },
        );
        #[cfg(not(target_os = "windows"))]
        let item = item.external_drag_payload(move |_: &DraggedItem, _, cx| {
            let files = offer
                .update(cx, |this, _| this.library_files_to_share())
                .ok()?;
            (!files.is_empty()).then(|| {
                ExternalDragPayload::Files(FileDragPaths::new(
                    files.into_iter().map(|file| (file, false)),
                ))
            })
        });
        item
    }

    fn start_library_drag(
        &mut self,
        path: &Path,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<(SharedString, Theme)> {
        self.change_selection(cx, |selection, _| selection.context_click(path));
        let files = self.library_files_to_share();
        let notice = library::share_gap(self.library.selected_entries())
            .map(|gap| (drag_notice_text(gap), self.theme));
        if files.is_empty() {
            return notice;
        }
        #[cfg(target_os = "windows")]
        {
            let source = platform::native_window(window);
            self.library.organizer.dragging_out = true;
            cx.spawn_in(window, async move |this, cx| {
                if let Err(error) = platform::drag_files_out(source, &files) {
                    tracing::warn!("library drag-out failed: {error}");
                }
                cx.update(|window, cx| {
                    cx.stop_active_drag(window);
                })
                .ok();
                this.update(cx, |this, _| this.library.organizer.dragging_out = false)
                    .ok();
            })
            .detach();
        }
        notice
    }
}

fn drag_notice_text(gap: ShareGap) -> SharedString {
    match gap {
        ShareGap::Unexported => t!("Export this project first to drag it out").into(),
        ShareGap::MissingFile => t!("Its file is missing, so it can't be dragged").into(),
        ShareGap::LeftOut(count) => {
            t!("Left out {count} with no file to drag", count = count).into()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::{MediaKind, view::Entry};

    #[gpui::test]
    fn a_drag_with_nothing_to_share_does_not_stay_marked_as_ours(cx: &mut gpui::TestAppContext) {
        let session = cx.update(crate::session::RecordingSession::init);
        let (library, cx) =
            cx.add_window_view(move |window, cx| LibraryWindow::new(session, window, cx));
        let path = PathBuf::from("Unexported.ever");
        let entry = Entry {
            path: path.clone(),
            kind: MediaKind::Studio,
            name: "Unexported".into(),
            size: 0,
            duration: None,
            created_millis: 0.,
            exports: Vec::new(),
            compression: crate::library::view::Compression::None,
            compressible: false,
            clip_count: 1,
            thumbnail: None,
            thumbnail_version: None,
            screenshot: None,
            opens_editor: true,
            audio_only: false,
            id: None,
            rebuilt: None,
        };
        library.update_in(cx, |library, _, cx| {
            library.shown = true;
            library.library.entries = Some(vec![entry]);
            library.rebuild_library(cx);
        });

        let notice = library.update_in(cx, |library, window, cx| {
            library.start_library_drag(&path, window, cx)
        });

        assert!(notice.is_some());
        library.read_with(cx, |library, _| {
            assert!(!library.library.organizer.dragging_out);
        });
    }
}
