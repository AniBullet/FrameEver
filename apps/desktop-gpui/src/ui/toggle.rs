use gpui::{
    App, ClickEvent, ElementId, Hsla, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};

use crate::theme::Theme;

#[derive(IntoElement)]
pub struct Toggle {
    id: ElementId,
    checked: bool,
    off_fill: Hsla,
    on_fill: Hsla,
    thumb: Hsla,
    on_click: Option<crate::ui::button::ClickHandler>,
}

impl Toggle {
    pub fn plain(theme: &Theme, id: impl Into<ElementId>, checked: bool) -> Self {
        Self {
            id: id.into(),
            checked,
            off_fill: Hsla::from(theme.component.switch),
            on_fill: Hsla::from(theme.editor.accent),
            thumb: gpui::white(),
            on_click: None,
        }
    }

    pub fn settings(theme: &Theme, id: impl Into<ElementId>, checked: bool) -> Self {
        Self {
            on_fill: Hsla::from(theme.primary),
            ..Self::plain(theme, id, checked)
        }
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for Toggle {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let Toggle {
            id,
            checked,
            off_fill,
            on_fill,
            thumb,
            on_click,
        } = self;

        div()
            .id(id)
            .w(px(34.))
            .h(px(20.))
            .p(px(2.))
            .flex()
            .flex_row()
            .flex_shrink_0()
            .rounded_full()
            .when(checked, |this| this.justify_end())
            .cursor_pointer()
            .bg(if checked { on_fill } else { off_fill })
            .child(div().size(px(16.)).rounded_full().bg(thumb))
            .when_some(on_click, |this, handler| {
                this.tab_index(0)
                    .on_click(move |event, window, cx| handler(event, window, cx))
            })
    }
}
