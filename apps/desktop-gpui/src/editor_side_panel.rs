use gpui::{
    AnyElement, Context, Div, FontWeight, Hsla, InteractiveElement, IntoElement, ParentElement,
    SharedString, Styled, Window, div, prelude::FluentBuilder, px, svg,
};

use crate::editor_edits::Selection;
use crate::editor_timeline::TrackKind;
use crate::editor_window::EditorWindow;
use crate::i18n::t;
use crate::theme::Theme;
use crate::ui;

pub(crate) const SWITCHER_HEIGHT: f32 = 40.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PanelTab {
    Style,
    Transcript,
    Clips,
}

impl PanelTab {
    pub(crate) const ALL: [PanelTab; 3] = [PanelTab::Style, PanelTab::Transcript, PanelTab::Clips];

    fn label(self) -> &'static str {
        match self {
            PanelTab::Style => t!("Style"),
            PanelTab::Transcript => t!("Transcript"),
            PanelTab::Clips => t!("Clips"),
        }
    }

    pub(crate) fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "style" => Some(PanelTab::Style),
            "transcript" => Some(PanelTab::Transcript),
            "clips" => Some(PanelTab::Clips),
            _ => None,
        }
    }

    pub(crate) fn from_flags(transcript_open: bool, clips_open: bool) -> Self {
        if clips_open {
            PanelTab::Clips
        } else if transcript_open {
            PanelTab::Transcript
        } else {
            PanelTab::Style
        }
    }

    pub(crate) fn escape_target(self) -> Option<Self> {
        (self != PanelTab::Style).then_some(PanelTab::Style)
    }
}

pub(crate) fn panel_header(
    theme: &Theme,
    title: &'static str,
    count: Option<SharedString>,
    actions: impl IntoElement,
) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .flex_none()
        .gap(px(8.))
        .h(px(crate::editor_chrome::PANEL_HEAD_HEIGHT))
        .pl(px(16.))
        .pr(px(8.))
        .border_b_1()
        .border_color(theme.border)
        .child(
            div()
                .flex()
                .flex_row()
                .items_baseline()
                .gap(px(8.))
                .min_w_0()
                .child(
                    div()
                        .flex_none()
                        .text_size(px(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.foreground)
                        .child(title),
                )
                .children(count.map(|count| {
                    div()
                        .min_w_0()
                        .truncate()
                        .text_size(px(12.))
                        .text_color(theme.muted_foreground)
                        .child(count)
                })),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .flex_none()
                .items_center()
                .gap(px(4.))
                .child(actions),
        )
}

pub(crate) fn panel_empty_state(
    theme: &Theme,
    icon: &'static str,
    description: impl Into<SharedString>,
) -> Div {
    let description = description.into();
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(10.))
        .p(px(24.))
        .child(
            svg()
                .path(icon)
                .size(px(24.))
                .text_color(theme.muted_foreground),
        )
        .when(!description.is_empty(), |empty| {
            empty.child(
                div()
                    .max_w(px(280.))
                    .text_size(px(13.))
                    .line_height(px(19.))
                    .text_center()
                    .text_color(theme.muted_foreground)
                    .child(description),
            )
        })
}

impl EditorWindow {
    pub(crate) fn panel_tab(&self) -> PanelTab {
        PanelTab::from_flags(self.transcript.open, self.clips.open)
    }

    pub(crate) fn set_panel_tab(
        &mut self,
        tab: PanelTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if tab != PanelTab::Style && !self.project_ready() {
            return;
        }
        let current = self.panel_tab();
        if current == tab {
            return;
        }
        match current {
            PanelTab::Clips => {
                if self.sidebar_selection().is_none() {
                    self.set_selection(None, cx);
                }
                self.close_clips(window, cx);
            }
            PanelTab::Transcript => self.close_transcript_panel(window, cx),
            PanelTab::Style => {}
        }
        match tab {
            PanelTab::Clips => {
                self.set_selection(None, cx);
                self.clips.open = true;
            }
            PanelTab::Transcript => self.open_transcript_panel(window, cx),
            PanelTab::Style => {}
        }
        tracing::debug!(from = ?current, to = ?tab, "editor panel tab");
        cx.notify();
        window.refresh();
    }

    pub(crate) fn follow_selection_to_style(
        &mut self,
        selection: Option<&Selection>,
        cx: &mut Context<Self>,
    ) {
        let segment = selection.is_some_and(|selection| selection.track != TrackKind::Clip);
        if !segment || self.panel_tab() == PanelTab::Style {
            return;
        }
        let handle = self.panel_window;
        let editor = cx.entity().downgrade();
        cx.defer(move |cx| {
            let switched = handle.update(cx, |_, window, cx| {
                if let Some(editor) = editor.upgrade() {
                    editor.update(cx, |this, cx| {
                        if this.sidebar_selection().is_some() {
                            this.set_panel_tab(PanelTab::Style, window, cx);
                        }
                    });
                }
            });
            if switched.is_err() {
                tracing::debug!("editor window closed before the panel could follow a selection");
            }
        });
    }

    pub(crate) fn panel_escape(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(target) = self.panel_tab().escape_target() else {
            return false;
        };
        self.set_panel_tab(target, window, cx);
        true
    }

    fn render_panel_switcher(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let active = self.panel_tab();
        let ready = self.project_ready();
        let options = PanelTab::ALL
            .into_iter()
            .map(|tab| ui::SegmentOption::new(tab.label(), tab == active).disabled(!ready))
            .collect();
        div()
            .flex()
            .flex_row()
            .items_center()
            .flex_none()
            .h(px(SWITCHER_HEIGHT))
            .px(px(8.))
            .border_b_1()
            .border_color(theme.border)
            .child(
                div().flex_1().min_w_0().child(
                    ui::SegmentedControl::editor(&theme, "editor-panel-tabs", options)
                        .stretch()
                        .item_height(px(26.))
                        .text_size(px(12.))
                        .on_select(cx.listener(|this, index: &usize, window, cx| {
                            if let Some(tab) = ui::option_at(&PanelTab::ALL, *index) {
                                this.set_panel_tab(tab, window, cx);
                            }
                        })),
                ),
            )
    }

    pub(crate) fn render_side_panel(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let body = match self.panel_tab() {
            PanelTab::Style => {
                self.with_style_controls(|editor| editor.render_sidebar(cx).into_any_element())
            }
            PanelTab::Transcript => self.render_transcript(window, cx),
            PanelTab::Clips => self.render_clips_body(cx).into_any_element(),
        };
        div()
            .id("editor-panel")
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .bg(Hsla::from(theme.background))
            .border_l_1()
            .border_color(theme.border)
            .child(self.render_panel_switcher(cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(body),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tab_follows_the_mode_flags() {
        assert_eq!(PanelTab::from_flags(false, false), PanelTab::Style);
        assert_eq!(PanelTab::from_flags(true, false), PanelTab::Transcript);
        assert_eq!(PanelTab::from_flags(false, true), PanelTab::Clips);
    }

    #[test]
    fn escape_returns_to_style_and_stops_there() {
        assert_eq!(PanelTab::Transcript.escape_target(), Some(PanelTab::Style));
        assert_eq!(PanelTab::Clips.escape_target(), Some(PanelTab::Style));
        assert_eq!(PanelTab::Style.escape_target(), None);
    }

    #[test]
    fn auto_panel_names_parse() {
        assert_eq!(PanelTab::from_name("style"), Some(PanelTab::Style));
        assert_eq!(
            PanelTab::from_name(" Transcript "),
            Some(PanelTab::Transcript)
        );
        assert_eq!(PanelTab::from_name("CLIPS"), Some(PanelTab::Clips));
        assert_eq!(PanelTab::from_name("captions"), None);
    }
}
