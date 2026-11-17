use gpui::{Div, ParentElement, Styled, div, px};

use crate::theme::Theme;

const RING_SIZE: f32 = 18.;
const RING_WIDTH: f32 = 1.5;
const GAP_SIZE: f32 = RING_SIZE - 2. * RING_WIDTH;
const SQUARE_SIZE: f32 = 8.;
const SQUARE_RADIUS: f32 = 2.;

fn disc(size: f32) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(size))
        .rounded_full()
}

fn ring(theme: &Theme, centre: Div) -> Div {
    disc(RING_SIZE)
        .bg(theme.primary_foreground)
        .child(disc(GAP_SIZE).bg(theme.record_gap).child(centre))
}

pub fn record_mark(theme: &Theme) -> Div {
    ring(
        theme,
        div()
            .size(px(SQUARE_SIZE))
            .rounded_full()
            .bg(theme.recording),
    )
}

pub fn stop_mark(theme: &Theme) -> Div {
    ring(
        theme,
        div()
            .size(px(SQUARE_SIZE))
            .rounded(px(SQUARE_RADIUS))
            .bg(theme.recording),
    )
}
