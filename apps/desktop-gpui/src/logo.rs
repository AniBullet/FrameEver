use gpui::{Div, Hsla, IntoElement, ParentElement, Styled, div, px, rgb, svg};

use crate::theme::Theme;

const ASPECT: f32 = 180. / 40.;
const PLATE: u32 = 0x1F3FD8;
const CLIP_TOP: u32 = 0xFFFDED;
const CLIP_BOTTOM: u32 = 0x87BFFF;
const RECORD_RING: u32 = 0xFFFFFF;
const RECORD_GAP: u32 = 0x000F5C;
const RECORD_DOT: u32 = 0xFF3B30;

fn layer(path: &'static str, color: Hsla, height: f32) -> impl IntoElement {
    svg()
        .absolute()
        .top_0()
        .left_0()
        .path(path)
        .w(px(height * ASPECT))
        .h(px(height))
        .text_color(color)
}

fn frame(height: f32) -> Div {
    div()
        .relative()
        .flex_shrink_0()
        .w(px(height * ASPECT))
        .h(px(height))
}

pub fn logo(theme: &Theme, height: f32) -> Div {
    let wordmark = if theme.is_dark() {
        rgb(0xF4F4F0)
    } else {
        rgb(0x1A1B24)
    };
    frame(height)
        .child(layer("icons/logo-tile.svg", rgb(PLATE).into(), height))
        .child(layer(
            "icons/logo-clip-top.svg",
            rgb(CLIP_TOP).into(),
            height,
        ))
        .child(layer(
            "icons/logo-clip-bottom.svg",
            rgb(CLIP_BOTTOM).into(),
            height,
        ))
        .child(layer(
            "icons/logo-record.svg",
            rgb(RECORD_RING).into(),
            height,
        ))
        .child(layer(
            "icons/logo-record-gap.svg",
            rgb(RECORD_GAP).into(),
            height,
        ))
        .child(layer(
            "icons/logo-record-dot.svg",
            rgb(RECORD_DOT).into(),
            height,
        ))
        .child(layer("icons/logo-wordmark.svg", wordmark.into(), height))
}

pub fn logo_mono(color: Hsla, height: f32) -> Div {
    frame(height)
        .child(layer("icons/logo-clip-top.svg", color, height))
        .child(layer("icons/logo-clip-bottom.svg", color, height))
        .child(layer("icons/logo-record.svg", color, height))
        .child(layer("icons/logo-wordmark.svg", color, height))
}
