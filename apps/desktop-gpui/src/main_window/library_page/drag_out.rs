use std::path::{Path, PathBuf};

use gpui::{
    AppContext as _, Context, Empty, IntoElement, ParentElement, Pixels, Point, Render,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};
#[cfg(not(target_os = "windows"))]
use gpui::{ExternalDragPayload, FileDragPaths};

use super::MainWindow;
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

impl MainWindow {
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
            cx.spawn_in(window, async move |_, cx| {
                if let Err(error) = platform::drag_files_out(source, &files) {
                    tracing::warn!("library drag-out failed: {error}");
                }
                cx.update(|window, cx| {
                    cx.stop_active_drag(window);
                })
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
