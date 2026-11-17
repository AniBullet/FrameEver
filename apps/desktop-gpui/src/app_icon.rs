use gpui::{AnyElement, Div, Hsla, IntoElement, ParentElement, Styled, div, px, rgb, svg};

pub const LOCKUP_ASPECT: f32 = 180. / 40.;

const GAP_LAYER: &str = "icons/logo-record-gap.svg";
const LAYERS: [(&str, u32); 6] = [
    ("icons/logo-tile.svg", 0x1F3FD8),
    ("icons/logo-clip-top.svg", 0xFFFDED),
    ("icons/logo-clip-bottom.svg", 0x87BFFF),
    ("icons/logo-record.svg", 0xFFFFFF),
    (GAP_LAYER, 0x000F5C),
    ("icons/logo-record-dot.svg", 0xFF3B30),
];

const GRID: f32 = 48.;
const RING_WIDTH: f32 = 1.9;
const SHARP_RING_WIDTH: f32 = 1.5;

pub fn shows_record_gap(size: f32) -> bool {
    size / GRID * RING_WIDTH >= SHARP_RING_WIDTH
}

pub fn layer(path: &'static str, color: Hsla, height: f32) -> impl IntoElement {
    svg()
        .absolute()
        .top_0()
        .left_0()
        .path(path)
        .w(px(height * LOCKUP_ASPECT))
        .h(px(height))
        .text_color(color)
}

pub fn mark_layers(size: f32) -> Vec<AnyElement> {
    let gap = shows_record_gap(size);
    LAYERS
        .into_iter()
        .filter(|(path, _)| gap || *path != GAP_LAYER)
        .map(|(path, colour)| layer(path, rgb(colour).into(), size).into_any_element())
        .collect()
}

pub fn app_icon(size: f32) -> Div {
    div()
        .relative()
        .flex_shrink_0()
        .overflow_hidden()
        .size(px(size))
        .children(mark_layers(size))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gap_is_drawn_only_where_the_ring_stays_at_least_a_pixel_and_a_half_wide() {
        for size in [16., 20., 24., 32.] {
            assert!(!shows_record_gap(size), "{size}");
        }
        for size in [40., 44., 72.] {
            assert!(shows_record_gap(size), "{size}");
        }
    }

    #[test]
    fn the_layers_and_colours_are_the_ones_the_brand_script_wrote() {
        let source = include_str!("../packaging/brand/icon-source.svg");
        let written: Vec<String> = source
            .split("<g fill=\"#")
            .skip(1)
            .map(|group| group[..6].to_uppercase())
            .collect();
        let drawn: Vec<String> = LAYERS
            .iter()
            .map(|(_, colour)| format!("{colour:06X}"))
            .collect();
        assert_eq!(drawn, written);

        for (path, _) in LAYERS {
            let name = path
                .trim_start_matches("icons/logo-")
                .trim_end_matches(".svg");
            let shape = std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("assets")
                    .join(path),
            )
            .unwrap();
            let body = shape
                .split_once("\">")
                .and_then(|(_, rest)| rest.split_once(")\">"))
                .map(|(_, rest)| rest.trim_end_matches("</g></svg>").to_string())
                .unwrap_or_else(|| panic!("{name}: {shape}"));
            assert!(source.contains(&body), "{name}: {body}");
        }
    }
}
