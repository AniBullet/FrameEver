use gpui::{IntoElement, ParentElement as _, Styled, div, px, svg};

use crate::theme::Theme;

pub(crate) fn camera_loading(theme: &Theme, radius: f32) -> impl IntoElement {
    div()
        .absolute()
        .inset_0()
        .rounded(px(radius))
        .bg(theme.preview)
        .flex()
        .items_center()
        .justify_center()
        .child(
            svg()
                .path("icons/webcam.svg")
                .size(px(24.))
                .text_color(theme.faint_foreground),
        )
}
