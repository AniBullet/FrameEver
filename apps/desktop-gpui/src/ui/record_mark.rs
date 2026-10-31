use gpui::{Div, ParentElement, Styled, div, px};

use crate::theme::Theme;

const RING_SIZE: f32 = 18.;
const RING_WIDTH: f32 = 1.5;
const SQUARE_SIZE: f32 = 8.;
const SQUARE_RADIUS: f32 = 2.;

pub fn stop_mark(theme: &Theme) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(RING_SIZE))
        .rounded_full()
        .border(px(RING_WIDTH))
        .border_color(theme.primary_foreground)
        .bg(theme.record_gap)
        .child(
            div()
                .size(px(SQUARE_SIZE))
                .rounded(px(SQUARE_RADIUS))
                .bg(theme.recording),
        )
}
