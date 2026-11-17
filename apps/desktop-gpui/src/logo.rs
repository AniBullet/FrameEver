use gpui::{Div, Hsla, ParentElement, Styled, div, px};

pub use crate::app_icon::app_icon;
use crate::app_icon::{LOCKUP_ASPECT, layer, mark_layers};
use crate::theme::Theme;

fn frame(height: f32) -> Div {
    div()
        .relative()
        .flex_shrink_0()
        .w(px(height * LOCKUP_ASPECT))
        .h(px(height))
}

pub fn logo(theme: &Theme, height: f32) -> Div {
    frame(height).children(mark_layers(height)).child(layer(
        "icons/logo-wordmark.svg",
        theme.foreground.into(),
        height,
    ))
}

pub fn logo_mono(color: Hsla, height: f32) -> Div {
    frame(height)
        .child(layer("icons/logo-clip-top.svg", color, height))
        .child(layer("icons/logo-clip-bottom.svg", color, height))
        .child(layer("icons/logo-record.svg", color, height))
        .child(layer("icons/logo-wordmark.svg", color, height))
}
