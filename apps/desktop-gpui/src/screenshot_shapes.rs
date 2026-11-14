use frameever_project::{Annotation, AnnotationType};

use crate::screenshot_annotations::{Rect, normalized_rect, parse_css_rgba};

pub const HIGHLIGHTER_ALPHA: f64 = 0.4;
pub const HIGHLIGHTER_COLOR: &str = "#F5A524";
pub const STEP_DEFAULT_SIZE: f64 = 42.;
pub const STEP_LABEL_RATIO: f64 = 0.6;
pub const STEP_SIZE_RANGE: (f64, f64) = (12., 120.);
pub const BUBBLE_DEFAULT_FONT_SIZE: f64 = 22.;
pub const BUBBLE_FONT_RANGE: (f64, f64) = (12., 60.);
pub const BUBBLE_RADIUS: f64 = 10.;
pub const MAGNIFIER_DEFAULT_ZOOM: f64 = 2.;
pub const MAGNIFIER_ZOOM_RANGE: (f64, f64) = (1.5, 4.);
pub const MAGNIFIER_BORDER_COLOR: &str = "#FFFFFF";
pub const MAGNIFIER_BORDER_WIDTH: f64 = 3.;
pub const SPOTLIGHT_DEFAULT_DARKNESS: f64 = 0.55;
pub const IMAGE_DEFAULT_RADIUS: f64 = 8.;
pub const IMAGE_RADIUS_RANGE: (f64, f64) = (0., 40.);
pub const IMAGE_PLACED_SHARE: f64 = 0.5;
pub const CLICK_SIZE: (f64, f64) = (120., 80.);
pub const LINE_CLICK_EXTENT: (f64, f64) = (90., -60.);
const BASELINE_RATIO: f64 = 0.35;
const MIN_SIZE: f64 = 1.;

pub type Rgba = ([u8; 3], f32);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Seg {
    Move(f64, f64),
    Line(f64, f64),
    Quad(f64, f64, f64, f64),
    Cubic(f64, f64, f64, f64, f64, f64),
    Close,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ShapeOp {
    Fill {
        path: Vec<Seg>,
        color: Rgba,
    },
    Stroke {
        path: Vec<Seg>,
        width: f64,
        color: Rgba,
        round: bool,
    },
    Label {
        text: String,
        font_size: f64,
        center: (f64, f64),
        color: Rgba,
    },
    Magnify {
        rect: Rect,
        center: (f64, f64),
        zoom: f64,
    },
    Image {
        rect: Rect,
        radius: f64,
        alpha: f32,
        file: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Top,
    Right,
    Bottom,
    Left,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tail {
    pub edge: Edge,
    pub points: [(f64, f64); 3],
}

pub fn is_shape(kind: AnnotationType) -> bool {
    matches!(
        kind,
        AnnotationType::Line
            | AnnotationType::Highlighter
            | AnnotationType::Step
            | AnnotationType::Bubble
            | AnnotationType::Spotlight
            | AnnotationType::Magnifier
            | AnnotationType::Image
    )
}

pub fn has_end_points(kind: AnnotationType) -> bool {
    matches!(kind, AnnotationType::Arrow | AnnotationType::Line)
}

pub fn has_text(kind: AnnotationType) -> bool {
    matches!(kind, AnnotationType::Text | AnnotationType::Bubble)
}

pub fn next_step_number(annotations: &[Annotation]) -> u32 {
    let used: Vec<u32> = annotations
        .iter()
        .filter(|annotation| annotation.annotation_type == AnnotationType::Step)
        .filter_map(|annotation| annotation.number)
        .collect();
    (1..).find(|number| !used.contains(number)).unwrap_or(1)
}

pub fn magnifier_zoom(annotation: &Annotation) -> f64 {
    annotation
        .zoom
        .filter(|zoom| zoom.is_finite() && *zoom > 0.)
        .unwrap_or(MAGNIFIER_DEFAULT_ZOOM)
}

pub fn bubble_font_size(annotation: &Annotation) -> f64 {
    annotation
        .font_size
        .filter(|size| size.is_finite() && *size > 0.)
        .unwrap_or(BUBBLE_DEFAULT_FONT_SIZE)
}

pub fn step_font_size(annotation: &Annotation) -> f64 {
    let rect = normalized_rect(annotation);
    rect.width.min(rect.height) * STEP_LABEL_RATIO
}

pub fn image_radius(annotation: &Annotation) -> f64 {
    let rect = normalized_rect(annotation);
    annotation
        .corner_radius
        .unwrap_or(0.)
        .clamp(0., (rect.width.min(rect.height) / 2.).max(0.))
}

pub fn image_alpha(annotation: &Annotation) -> f32 {
    annotation.opacity.clamp(0., 1.) as f32
}

pub fn place_image(
    annotation: &mut Annotation,
    natural: (u32, u32),
    content: Rect,
    at: (f64, f64),
) {
    let (width, height) = (f64::from(natural.0.max(1)), f64::from(natural.1.max(1)));
    let scale = (content.width * IMAGE_PLACED_SHARE / width)
        .min(content.height * IMAGE_PLACED_SHARE / height)
        .min(1.);
    annotation.width = (width * scale).max(MIN_SIZE);
    annotation.height = (height * scale).max(MIN_SIZE);
    annotation.x = at.0 - annotation.width / 2.;
    annotation.y = at.1 - annotation.height / 2.;
}

pub fn rounded_rect_path(rect: Rect, radius: f64) -> Vec<Seg> {
    if radius <= 0. {
        return rect_path(rect);
    }
    let (left, top) = (rect.x, rect.y);
    let (right, bottom) = (rect.x + rect.width, rect.y + rect.height);
    let bend = radius * (1. - 0.552_284_75);
    vec![
        Seg::Move(left + radius, top),
        Seg::Line(right - radius, top),
        Seg::Cubic(right - bend, top, right, top + bend, right, top + radius),
        Seg::Line(right, bottom - radius),
        Seg::Cubic(
            right,
            bottom - bend,
            right - bend,
            bottom,
            right - radius,
            bottom,
        ),
        Seg::Line(left + radius, bottom),
        Seg::Cubic(
            left + bend,
            bottom,
            left,
            bottom - bend,
            left,
            bottom - radius,
        ),
        Seg::Line(left, top + radius),
        Seg::Cubic(left, top + bend, left + bend, top, left + radius, top),
        Seg::Close,
    ]
}

pub fn center(rect: Rect) -> (f64, f64) {
    (rect.x + rect.width / 2., rect.y + rect.height / 2.)
}

pub fn label_baseline(center_y: f64, font_size: f64) -> f64 {
    center_y + font_size * BASELINE_RATIO
}

pub fn contrast_text(fill: Option<Rgba>) -> [u8; 3] {
    let Some((rgb, alpha)) = fill else {
        return [0, 0, 0];
    };
    if alpha < 0.5 {
        return [0, 0, 0];
    }
    let luminance =
        0.2126 * f64::from(rgb[0]) + 0.7152 * f64::from(rgb[1]) + 0.0722 * f64::from(rgb[2]);
    if luminance > 160. {
        [0, 0, 0]
    } else {
        [255, 255, 255]
    }
}

pub fn default_tail(rect: Rect) -> [f64; 2] {
    [rect.x + rect.width * 0.25 - 8., rect.y + rect.height + 34.]
}

pub fn bubble_tail(annotation: &Annotation) -> Option<Tail> {
    let rect = normalized_rect(annotation);
    let [tip_x, tip_y] = annotation.tail?;
    if rect.width <= 0. || rect.height <= 0. {
        return None;
    }
    let (left, top) = (rect.x, rect.y);
    let (right, bottom) = (rect.x + rect.width, rect.y + rect.height);
    if tip_x >= left && tip_x <= right && tip_y >= top && tip_y <= bottom {
        return None;
    }
    let radius = bubble_radius(rect);
    let (cx, cy) = center(rect);
    let (dx, dy) = (tip_x - cx, tip_y - cy);
    let horizontal = dx.abs() * rect.height <= dy.abs() * rect.width;
    let edge = match (horizontal, dx < 0., dy < 0.) {
        (true, _, true) => Edge::Top,
        (true, _, false) => Edge::Bottom,
        (false, true, _) => Edge::Left,
        (false, false, _) => Edge::Right,
    };
    let length = if horizontal { rect.width } else { rect.height };
    let half = (rect.width.min(rect.height) * 0.2)
        .clamp(4., 24.)
        .min(length / 2. - radius);
    if half <= 0.5 {
        return None;
    }
    let along = |start: f64, span: f64, toward: f64| {
        toward.clamp(start + radius + half, start + span - radius - half)
    };
    let tip = (tip_x, tip_y);
    let points = match edge {
        Edge::Top | Edge::Bottom => {
            let edge_y = if edge == Edge::Top { top } else { bottom };
            let base = along(left, rect.width, cx + dx * ((edge_y - cy) / dy));
            if edge == Edge::Top {
                [(base - half, edge_y), tip, (base + half, edge_y)]
            } else {
                [(base + half, edge_y), tip, (base - half, edge_y)]
            }
        }
        Edge::Left | Edge::Right => {
            let edge_x = if edge == Edge::Left { left } else { right };
            let base = along(top, rect.height, cy + dy * ((edge_x - cx) / dx));
            if edge == Edge::Right {
                [(edge_x, base - half), tip, (edge_x, base + half)]
            } else {
                [(edge_x, base + half), tip, (edge_x, base - half)]
            }
        }
    };
    Some(Tail { edge, points })
}

fn bubble_radius(rect: Rect) -> f64 {
    BUBBLE_RADIUS
        .min(rect.width / 2.)
        .min(rect.height / 2.)
        .max(0.)
}

pub fn bubble_outline(annotation: &Annotation) -> Vec<Seg> {
    let rect = normalized_rect(annotation);
    let radius = bubble_radius(rect);
    let tail = bubble_tail(annotation);
    let (left, top) = (rect.x, rect.y);
    let (right, bottom) = (rect.x + rect.width, rect.y + rect.height);
    let mut path = vec![Seg::Move(left + radius, top)];
    let insert = |path: &mut Vec<Seg>, edge: Edge| {
        if let Some(tail) = tail.filter(|tail| tail.edge == edge) {
            path.extend(tail.points.iter().map(|(x, y)| Seg::Line(*x, *y)));
        }
    };
    insert(&mut path, Edge::Top);
    path.push(Seg::Line(right - radius, top));
    path.push(Seg::Quad(right, top, right, top + radius));
    insert(&mut path, Edge::Right);
    path.push(Seg::Line(right, bottom - radius));
    path.push(Seg::Quad(right, bottom, right - radius, bottom));
    insert(&mut path, Edge::Bottom);
    path.push(Seg::Line(left + radius, bottom));
    path.push(Seg::Quad(left, bottom, left, bottom - radius));
    insert(&mut path, Edge::Left);
    path.push(Seg::Line(left, top + radius));
    path.push(Seg::Quad(left, top, left + radius, top));
    path.push(Seg::Close);
    path
}

pub fn rect_path(rect: Rect) -> Vec<Seg> {
    let (right, bottom) = (rect.x + rect.width, rect.y + rect.height);
    vec![
        Seg::Move(rect.x, rect.y),
        Seg::Line(right, rect.y),
        Seg::Line(right, bottom),
        Seg::Line(rect.x, bottom),
        Seg::Close,
    ]
}

fn ellipse_path(rect: Rect) -> Vec<Seg> {
    const KAPPA: f64 = 0.552_284_749_8;
    let (cx, cy) = center(rect);
    let (rx, ry) = (rect.width / 2., rect.height / 2.);
    let (ox, oy) = (rx * KAPPA, ry * KAPPA);
    vec![
        Seg::Move(cx + rx, cy),
        Seg::Cubic(cx + rx, cy + oy, cx + ox, cy + ry, cx, cy + ry),
        Seg::Cubic(cx - ox, cy + ry, cx - rx, cy + oy, cx - rx, cy),
        Seg::Cubic(cx - rx, cy - oy, cx - ox, cy - ry, cx, cy - ry),
        Seg::Cubic(cx + ox, cy - ry, cx + rx, cy - oy, cx + rx, cy),
        Seg::Close,
    ]
}

pub fn magnified_frame(frame: Rect, center: (f64, f64), zoom: f64) -> Rect {
    Rect {
        x: center.0 + (frame.x - center.0) * zoom,
        y: center.1 + (frame.y - center.1) * zoom,
        width: frame.width * zoom,
        height: frame.height * zoom,
    }
}

pub fn intersect(a: Rect, b: Rect) -> Option<Rect> {
    let left = a.x.max(b.x);
    let top = a.y.max(b.y);
    let right = (a.x + a.width).min(b.x + b.width);
    let bottom = (a.y + a.height).min(b.y + b.height);
    (right > left && bottom > top).then_some(Rect {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    })
}

pub fn shape_ops(annotation: &Annotation, frame: Rect) -> Vec<ShapeOp> {
    let rect = normalized_rect(annotation);
    let stroke = parse_css_rgba(&annotation.stroke_color, annotation.opacity);
    match annotation.annotation_type {
        AnnotationType::Line => match stroke {
            Some(color) if annotation.stroke_width > 0. => vec![ShapeOp::Stroke {
                path: vec![
                    Seg::Move(annotation.x, annotation.y),
                    Seg::Line(
                        annotation.x + annotation.width,
                        annotation.y + annotation.height,
                    ),
                ],
                width: annotation.stroke_width,
                color,
                round: true,
            }],
            _ => Vec::new(),
        },
        AnnotationType::Highlighter => parse_css_rgba(
            &annotation.stroke_color,
            annotation.opacity * HIGHLIGHTER_ALPHA,
        )
        .filter(|_| rect.width > 0. && rect.height > 0.)
        .map(|color| ShapeOp::Fill {
            path: rect_path(rect),
            color,
        })
        .into_iter()
        .collect(),
        AnnotationType::Step => {
            let side = rect.width.min(rect.height);
            let Some(color) = stroke.filter(|_| side > 0.) else {
                return Vec::new();
            };
            let (cx, cy) = center(rect);
            let disc = Rect {
                x: cx - side / 2.,
                y: cy - side / 2.,
                width: side,
                height: side,
            };
            let mut ops = vec![ShapeOp::Fill {
                path: ellipse_path(disc),
                color,
            }];
            if let Some(number) = annotation.number {
                ops.push(ShapeOp::Label {
                    text: number.to_string(),
                    font_size: step_font_size(annotation),
                    center: (cx, cy),
                    color: (contrast_text(Some(color)), color.1),
                });
            }
            ops
        }
        AnnotationType::Bubble => {
            if rect.width <= 0. || rect.height <= 0. {
                return Vec::new();
            }
            let fill = parse_css_rgba(&annotation.fill_color, annotation.opacity);
            let mut ops = Vec::new();
            if let Some(color) = fill {
                ops.push(ShapeOp::Fill {
                    path: bubble_outline(annotation),
                    color,
                });
            }
            if let Some(text) = annotation.text.as_deref().filter(|text| !text.is_empty()) {
                ops.push(ShapeOp::Label {
                    text: text.replace('\n', " "),
                    font_size: bubble_font_size(annotation),
                    center: center(rect),
                    color: (contrast_text(fill), annotation.opacity.clamp(0., 1.) as f32),
                });
            }
            ops
        }
        AnnotationType::Spotlight => {
            let darkness = annotation.opacity.clamp(0., 1.) as f32;
            if darkness <= 0. {
                return Vec::new();
            }
            let mut path = rect_path(frame);
            if rect.width > 0. && rect.height > 0. {
                path.extend(rect_path(rect));
            }
            vec![ShapeOp::Fill {
                path,
                color: ([0, 0, 0], darkness),
            }]
        }
        AnnotationType::Magnifier => {
            if rect.width <= 0. || rect.height <= 0. {
                return Vec::new();
            }
            let mut ops = vec![ShapeOp::Magnify {
                rect,
                center: center(rect),
                zoom: magnifier_zoom(annotation),
            }];
            if let Some(color) = parse_css_rgba(&annotation.stroke_color, 1.)
                && annotation.stroke_width > 0.
            {
                ops.push(ShapeOp::Stroke {
                    path: rect_path(rect),
                    width: annotation.stroke_width,
                    color,
                    round: false,
                });
            }
            ops
        }
        AnnotationType::Image => match &annotation.image {
            Some(file) if rect.width > 0. && rect.height > 0. => vec![ShapeOp::Image {
                rect,
                radius: image_radius(annotation),
                alpha: image_alpha(annotation),
                file: file.clone(),
            }],
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

pub fn shape_extent(annotation: &Annotation) -> Rect {
    let rect = normalized_rect(annotation);
    let grow = |rect: Rect, by: f64| Rect {
        x: rect.x - by,
        y: rect.y - by,
        width: rect.width + by * 2.,
        height: rect.height + by * 2.,
    };
    match annotation.annotation_type {
        AnnotationType::Line => grow(rect, (annotation.stroke_width / 2.).max(0.)),
        AnnotationType::Magnifier => grow(rect, (annotation.stroke_width / 2.).max(0.)),
        AnnotationType::Bubble => match bubble_tail(annotation) {
            Some(tail) => {
                let (x, y) = tail.points[1];
                let left = rect.x.min(x);
                let top = rect.y.min(y);
                Rect {
                    x: left,
                    y: top,
                    width: (rect.x + rect.width).max(x) - left,
                    height: (rect.y + rect.height).max(y) - top,
                }
            }
            None => rect,
        },
        _ => rect,
    }
}

pub fn hit_shape(annotation: &Annotation, point: (f64, f64), handle_size: f64) -> bool {
    let rect = normalized_rect(annotation);
    let inside = |rect: Rect, slop: f64| {
        point.0 >= rect.x - slop
            && point.0 <= rect.x + rect.width + slop
            && point.1 >= rect.y - slop
            && point.1 <= rect.y + rect.height + slop
    };
    match annotation.annotation_type {
        AnnotationType::Line => {
            let tolerance = (annotation.stroke_width / 2.).max(handle_size / 2.);
            distance_to_segment(
                point,
                (annotation.x, annotation.y),
                (
                    annotation.x + annotation.width,
                    annotation.y + annotation.height,
                ),
            ) <= tolerance
        }
        AnnotationType::Step => {
            let (cx, cy) = center(rect);
            let radius = rect.width.min(rect.height) / 2.;
            radius > 0. && (point.0 - cx).hypot(point.1 - cy) <= radius
        }
        AnnotationType::Bubble => {
            inside(rect, 0.)
                || bubble_tail(annotation).is_some_and(|tail| point_in_triangle(point, tail.points))
        }
        AnnotationType::Magnifier => inside(rect, (annotation.stroke_width / 2.).max(0.)),
        AnnotationType::Highlighter | AnnotationType::Spotlight | AnnotationType::Image => {
            inside(rect, 0.)
        }
        _ => false,
    }
}

pub fn distance_to_segment(point: (f64, f64), start: (f64, f64), end: (f64, f64)) -> f64 {
    let (dx, dy) = (end.0 - start.0, end.1 - start.1);
    let length_squared = dx * dx + dy * dy;
    let t = if length_squared <= f64::EPSILON {
        0.
    } else {
        (((point.0 - start.0) * dx + (point.1 - start.1) * dy) / length_squared).clamp(0., 1.)
    };
    (point.0 - (start.0 + t * dx)).hypot(point.1 - (start.1 + t * dy))
}

fn point_in_triangle(point: (f64, f64), triangle: [(f64, f64); 3]) -> bool {
    let sign = |a: (f64, f64), b: (f64, f64), c: (f64, f64)| {
        (a.0 - c.0) * (b.1 - c.1) - (b.0 - c.0) * (a.1 - c.1)
    };
    let d1 = sign(point, triangle[0], triangle[1]);
    let d2 = sign(point, triangle[1], triangle[2]);
    let d3 = sign(point, triangle[2], triangle[0]);
    let negative = d1 < 0. || d2 < 0. || d3 < 0.;
    let positive = d1 > 0. || d2 > 0. || d3 > 0.;
    !(negative && positive)
}

pub fn apply_defaults(annotation: &mut Annotation, annotations: &[Annotation]) {
    match annotation.annotation_type {
        AnnotationType::Highlighter => {
            annotation.stroke_color = HIGHLIGHTER_COLOR.to_string();
        }
        AnnotationType::Step => {
            annotation.number = Some(next_step_number(annotations));
            annotation.x -= STEP_DEFAULT_SIZE / 2.;
            annotation.y -= STEP_DEFAULT_SIZE / 2.;
            annotation.width = STEP_DEFAULT_SIZE;
            annotation.height = STEP_DEFAULT_SIZE;
        }
        AnnotationType::Bubble => {
            annotation.fill_color = std::mem::replace(
                &mut annotation.stroke_color,
                crate::screenshot_annotations::TRANSPARENT.to_string(),
            );
            annotation.font_size = Some(BUBBLE_DEFAULT_FONT_SIZE);
        }
        AnnotationType::Spotlight => {
            annotation.stroke_color = crate::screenshot_annotations::TRANSPARENT.to_string();
            annotation.opacity = SPOTLIGHT_DEFAULT_DARKNESS;
        }
        AnnotationType::Magnifier => {
            annotation.stroke_color = MAGNIFIER_BORDER_COLOR.to_string();
            annotation.stroke_width = MAGNIFIER_BORDER_WIDTH;
            annotation.opacity = 1.;
            annotation.zoom = Some(MAGNIFIER_DEFAULT_ZOOM);
        }
        AnnotationType::Image => {
            annotation.stroke_color = crate::screenshot_annotations::TRANSPARENT.to_string();
            annotation.stroke_width = 0.;
            annotation.opacity = 1.;
            annotation.corner_radius = Some(IMAGE_DEFAULT_RADIUS);
        }
        _ => {}
    }
}

pub fn takes_click_size(kind: AnnotationType) -> bool {
    matches!(
        kind,
        AnnotationType::Highlighter
            | AnnotationType::Bubble
            | AnnotationType::Spotlight
            | AnnotationType::Magnifier
    )
}

pub fn give_default_size(annotation: &mut Annotation, at: (f64, f64)) {
    match annotation.annotation_type {
        AnnotationType::Arrow | AnnotationType::Line => {
            annotation.x = at.0 - LINE_CLICK_EXTENT.0 / 2.;
            annotation.y = at.1 - LINE_CLICK_EXTENT.1 / 2.;
            annotation.width = LINE_CLICK_EXTENT.0;
            annotation.height = LINE_CLICK_EXTENT.1;
        }
        AnnotationType::Text | AnnotationType::Step | AnnotationType::Draw => {}
        _ => {
            annotation.x = at.0 - CLICK_SIZE.0 / 2.;
            annotation.y = at.1 - CLICK_SIZE.1 / 2.;
            annotation.width = CLICK_SIZE.0;
            annotation.height = CLICK_SIZE.1;
        }
    }
}

pub fn finish_bubble(annotation: &mut Annotation) {
    if annotation.annotation_type == AnnotationType::Bubble && annotation.tail.is_none() {
        annotation.tail = Some(default_tail(normalized_rect(annotation)));
    }
}

pub fn normalize_box(annotation: &mut Annotation) {
    if annotation.width < 0. {
        annotation.x += annotation.width;
        annotation.width = -annotation.width;
    }
    if annotation.height < 0. {
        annotation.y += annotation.height;
        annotation.height = -annotation.height;
    }
}

pub fn translate(annotation: &mut Annotation, dx: f64, dy: f64) {
    annotation.x += dx;
    annotation.y += dy;
    if let Some(tail) = annotation.tail.as_mut() {
        tail[0] += dx;
        tail[1] += dy;
    }
}

pub fn keyboard_resize(annotation: &mut Annotation, dx: f64, dy: f64) {
    match annotation.annotation_type {
        AnnotationType::Arrow | AnnotationType::Line => {
            annotation.width += dx;
            annotation.height += dy;
        }
        AnnotationType::Step => {
            let delta = if dx != 0. { dx } else { -dy };
            set_step_size(annotation, annotation.width.abs() + delta);
        }
        AnnotationType::Text => {
            annotation.height = (annotation.height - dy + dx).max(MIN_SIZE);
        }
        _ => {
            normalize_box(annotation);
            annotation.width = (annotation.width + dx).max(MIN_SIZE);
            annotation.height = (annotation.height + dy).max(MIN_SIZE);
        }
    }
}

pub fn set_step_size(annotation: &mut Annotation, size: f64) {
    let size = size.clamp(STEP_SIZE_RANGE.0, STEP_SIZE_RANGE.1);
    let (cx, cy) = center(normalized_rect(annotation));
    annotation.x = cx - size / 2.;
    annotation.y = cy - size / 2.;
    annotation.width = size;
    annotation.height = size;
}

pub fn scale_extras(
    annotation: &mut Annotation,
    map: impl Fn(f64, f64) -> (f64, f64),
    scalar: f64,
) {
    if let Some(tail) = annotation.tail.as_mut() {
        let (x, y) = map(tail[0], tail[1]);
        *tail = [x, y];
    }
    if let Some(size) = annotation.font_size.as_mut() {
        *size *= scalar;
    }
    if let Some(radius) = annotation.corner_radius.as_mut() {
        *radius *= scalar;
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use frameever_project::ProjectConfiguration;

    pub(crate) fn shape(
        kind: AnnotationType,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> Annotation {
        Annotation {
            id: format!("{kind:?}"),
            annotation_type: kind,
            x,
            y,
            width,
            height,
            stroke_color: "#F05656".into(),
            stroke_width: 4.,
            fill_color: "transparent".into(),
            opacity: 1.,
            rotation: 0.,
            text: None,
            mask_type: None,
            mask_level: None,
            points: None,
            number: None,
            font_size: None,
            tail: None,
            zoom: None,
            image: None,
            corner_radius: None,
        }
    }

    pub(crate) fn one_of_each() -> Vec<Annotation> {
        let line = shape(AnnotationType::Line, 10., 20., 60., -15.);
        let highlighter = shape(AnnotationType::Highlighter, 5., 5., 80., 20.);
        let mut step = shape(AnnotationType::Step, 100., 40., 42., 42.);
        step.number = Some(3);
        let mut bubble = shape(AnnotationType::Bubble, 20., 60., 120., 50.);
        bubble.fill_color = "#2CB67D".into();
        bubble.text = Some("Look here".into());
        bubble.font_size = Some(18.);
        bubble.tail = Some([30., 150.]);
        let mut spotlight = shape(AnnotationType::Spotlight, 40., 40., 50., 30.);
        spotlight.opacity = 0.7;
        let mut magnifier = shape(AnnotationType::Magnifier, 150., 90., 40., 40.);
        magnifier.zoom = Some(3.);
        vec![line, highlighter, step, bubble, spotlight, magnifier]
    }

    fn frame() -> Rect {
        Rect {
            x: 0.,
            y: 0.,
            width: 200.,
            height: 160.,
        }
    }

    #[test]
    fn each_new_tool_survives_save_and_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let config = ProjectConfiguration {
            annotations: one_of_each(),
            ..ProjectConfiguration::default()
        };
        config.write(dir.path()).unwrap();
        let reopened = ProjectConfiguration::load(dir.path()).unwrap();
        assert_eq!(
            serde_json::to_value(&reopened.annotations).unwrap(),
            serde_json::to_value(&config.annotations).unwrap()
        );
        let types: Vec<String> = reopened
            .annotations
            .iter()
            .map(|annotation| serde_json::to_value(annotation.annotation_type).unwrap())
            .map(|value| value.as_str().unwrap().to_string())
            .collect();
        assert_eq!(
            types,
            [
                "line",
                "highlighter",
                "step",
                "bubble",
                "spotlight",
                "magnifier"
            ]
        );
    }

    #[test]
    fn an_older_project_still_opens_and_writes_no_new_keys() {
        let dir = tempfile::tempdir().unwrap();
        let mut document = serde_json::to_value(ProjectConfiguration::default()).unwrap();
        document["annotations"] = serde_json::json!([{
            "id": "old",
            "type": "rectangle",
            "x": 1.0, "y": 2.0, "width": 30.0, "height": 40.0,
            "strokeColor": "#F05656", "strokeWidth": 4.0,
            "fillColor": "transparent", "opacity": 1.0, "rotation": 0.0,
            "text": null
        }]);
        std::fs::write(
            dir.path().join("project-config.json"),
            serde_json::to_string(&document).unwrap(),
        )
        .unwrap();

        let config = ProjectConfiguration::load(dir.path()).unwrap();
        assert_eq!(config.annotations.len(), 1);
        assert_eq!(
            config.annotations[0].annotation_type,
            AnnotationType::Rectangle
        );
        assert_eq!(config.annotations[0].width, 30.);

        let written = serde_json::to_value(&config.annotations[0]).unwrap();
        for key in ["number", "fontSize", "tail", "zoom"] {
            assert!(written.get(key).is_none(), "{key} written for an old shape");
        }
    }

    #[test]
    fn a_step_takes_the_lowest_unused_number() {
        let mut list = Vec::new();
        assert_eq!(next_step_number(&list), 1);
        for number in [1, 2, 4] {
            let mut step = shape(AnnotationType::Step, 0., 0., 10., 10.);
            step.number = Some(number);
            list.push(step);
        }
        assert_eq!(next_step_number(&list), 3);
        list[1].number = Some(3);
        assert_eq!(next_step_number(&list), 2);
    }

    #[test]
    fn a_line_strokes_between_its_end_points_with_round_caps() {
        let line = shape(AnnotationType::Line, 10., 20., 60., -15.);
        assert_eq!(
            shape_ops(&line, frame()),
            vec![ShapeOp::Stroke {
                path: vec![Seg::Move(10., 20.), Seg::Line(70., 5.)],
                width: 4.,
                color: ([0xF0, 0x56, 0x56], 1.),
                round: true,
            }]
        );
        assert!(hit_shape(&line, (40., 12.5), 10.));
        assert!(!hit_shape(&line, (40., 40.), 10.));
    }

    #[test]
    fn a_highlighter_fills_its_box_at_forty_percent() {
        let mut highlighter = shape(AnnotationType::Highlighter, 5., 5., 80., -20.);
        highlighter.opacity = 0.5;
        let ops = shape_ops(&highlighter, frame());
        let ShapeOp::Fill { path, color } = &ops[0] else {
            panic!("{ops:?}");
        };
        assert_eq!(path[0], Seg::Move(5., -15.));
        assert!((color.1 - 0.2).abs() < 1e-6);
    }

    #[test]
    fn a_step_is_a_disc_with_its_number_in_a_readable_color() {
        let mut step = shape(AnnotationType::Step, 100., 40., 42., 42.);
        step.number = Some(7);
        let ops = shape_ops(&step, frame());
        assert_eq!(ops.len(), 2);
        assert_eq!(
            ops[1],
            ShapeOp::Label {
                text: "7".into(),
                font_size: 42. * STEP_LABEL_RATIO,
                center: (121., 61.),
                color: ([255, 255, 255], 1.),
            }
        );
        step.stroke_color = "#FFFFFF".into();
        let ShapeOp::Label { color, .. } = &shape_ops(&step, frame())[1] else {
            panic!();
        };
        assert_eq!(color.0, [0, 0, 0]);
        assert!(hit_shape(&step, (121., 61.), 10.));
        assert!(!hit_shape(&step, (101., 41.), 10.));
    }

    #[test]
    fn a_bubble_tail_leaves_the_edge_facing_its_tip() {
        let mut bubble = shape(AnnotationType::Bubble, 20., 60., 120., 50.);
        bubble.tail = Some([30., 150.]);
        let tail = bubble_tail(&bubble).unwrap();
        assert_eq!(tail.edge, Edge::Bottom);
        assert_eq!(tail.points[1], (30., 150.));
        assert!(tail.points.iter().all(|point| point.1 <= 150.));
        assert_eq!(tail.points[0].1, 110.);
        assert!(tail.points[2].0 >= 20. + BUBBLE_RADIUS);

        bubble.tail = Some([300., 80.]);
        assert_eq!(bubble_tail(&bubble).unwrap().edge, Edge::Right);
        bubble.tail = Some([80., 0.]);
        assert_eq!(bubble_tail(&bubble).unwrap().edge, Edge::Top);
        bubble.tail = Some([-50., 90.]);
        assert_eq!(bubble_tail(&bubble).unwrap().edge, Edge::Left);
        bubble.tail = Some([80., 80.]);
        assert_eq!(bubble_tail(&bubble), None);
    }

    #[test]
    fn a_bubble_is_one_outline_through_its_tip_with_centred_text() {
        let mut bubble = shape(AnnotationType::Bubble, 20., 60., 120., 50.);
        bubble.fill_color = "#F05656".into();
        bubble.text = Some("Note".into());
        bubble.tail = Some([30., 150.]);
        let ops = shape_ops(&bubble, frame());
        let ShapeOp::Fill { path, .. } = &ops[0] else {
            panic!("{ops:?}");
        };
        assert!(path.contains(&Seg::Line(30., 150.)));
        assert_eq!(
            path.iter()
                .filter(|seg| matches!(seg, Seg::Move(..)))
                .count(),
            1
        );
        let ShapeOp::Label {
            center, font_size, ..
        } = &ops[1]
        else {
            panic!("{ops:?}");
        };
        assert_eq!(*center, (80., 85.));
        assert_eq!(*font_size, BUBBLE_DEFAULT_FONT_SIZE);
        assert!(hit_shape(&bubble, (37., 140.), 10.));
        assert!(!hit_shape(&bubble, (25., 140.), 10.));
        assert_eq!(shape_extent(&bubble).height, 90.);
    }

    #[test]
    fn a_spotlight_darkens_the_frame_around_a_hole() {
        let spotlight = shape(AnnotationType::Spotlight, 40., 40., 50., 30.);
        let ops = shape_ops(&spotlight, frame());
        let ShapeOp::Fill { path, color } = &ops[0] else {
            panic!("{ops:?}");
        };
        assert_eq!(color.0, [0, 0, 0]);
        assert_eq!(
            path.iter()
                .filter(|seg| matches!(seg, Seg::Move(..)))
                .count(),
            2
        );
        assert_eq!(path[0], Seg::Move(0., 0.));
        assert!(path.contains(&Seg::Move(40., 40.)));
    }

    #[test]
    fn a_magnifier_enlarges_about_its_centre_and_draws_its_border() {
        let mut magnifier = shape(AnnotationType::Magnifier, 150., 90., 40., 40.);
        magnifier.zoom = Some(3.);
        magnifier.stroke_color = "#FFFFFF".into();
        let ops = shape_ops(&magnifier, frame());
        assert_eq!(
            ops[0],
            ShapeOp::Magnify {
                rect: normalized_rect(&magnifier),
                center: (170., 110.),
                zoom: 3.,
            }
        );
        assert!(matches!(ops[1], ShapeOp::Stroke { round: false, .. }));
        let zoomed = magnified_frame(frame(), (170., 110.), 3.);
        assert_eq!((zoomed.x, zoomed.y), (-340., -220.));
        assert_eq!((zoomed.width, zoomed.height), (600., 480.));
    }

    #[test]
    fn keyboard_moves_and_resizes_shapes() {
        let mut bubble = shape(AnnotationType::Bubble, 20., 60., 120., 50.);
        bubble.tail = Some([30., 150.]);
        translate(&mut bubble, 10., -5.);
        assert_eq!(
            (bubble.x, bubble.y, bubble.tail),
            (30., 55., Some([40., 145.]))
        );

        let mut line = shape(AnnotationType::Line, 0., 0., 10., 10.);
        keyboard_resize(&mut line, 1., 0.);
        assert_eq!((line.x, line.width, line.height), (0., 11., 10.));

        let mut step = shape(AnnotationType::Step, 0., 0., 40., 40.);
        keyboard_resize(&mut step, 10., 0.);
        assert_eq!(
            (step.x, step.y, step.width, step.height),
            (-5., -5., 50., 50.)
        );

        let mut spotlight = shape(AnnotationType::Spotlight, 10., 10., -20., 20.);
        keyboard_resize(&mut spotlight, 0., -30.);
        assert_eq!(
            (spotlight.x, spotlight.width, spotlight.height),
            (-10., 20., MIN_SIZE)
        );
    }

    #[test]
    fn a_click_places_each_new_tool_at_a_usable_size() {
        let list = one_of_each();
        let mut step = shape(AnnotationType::Step, 50., 50., 0., 0.);
        apply_defaults(&mut step, &list);
        assert_eq!(step.number, Some(1));
        assert_eq!((step.x, step.y, step.width), (29., 29., STEP_DEFAULT_SIZE));

        let mut bubble = shape(AnnotationType::Bubble, 50., 50., 2., 1.);
        apply_defaults(&mut bubble, &list);
        assert_eq!(bubble.fill_color, "#F05656");
        assert_eq!(bubble.stroke_color, "transparent");
        assert!(takes_click_size(bubble.annotation_type));
        give_default_size(&mut bubble, (50., 50.));
        finish_bubble(&mut bubble);
        assert_eq!((bubble.x, bubble.width, bubble.height), (-10., 120., 80.));
        assert_eq!(bubble.tail, Some([12., 124.]));

        let mut line = shape(AnnotationType::Line, 50., 50., 0., 0.);
        give_default_size(&mut line, (50., 50.));
        assert_eq!((line.width, line.height), LINE_CLICK_EXTENT);

        let mut magnifier = shape(AnnotationType::Magnifier, 0., 0., 0., 0.);
        apply_defaults(&mut magnifier, &list);
        assert_eq!(magnifier.zoom, Some(MAGNIFIER_DEFAULT_ZOOM));
        assert_eq!(magnifier.stroke_width, MAGNIFIER_BORDER_WIDTH);
    }
}
