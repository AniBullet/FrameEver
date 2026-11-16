use frameever_project::{
    BackgroundConfiguration, Camera, CaptionSettings, CursorConfiguration, MaskKind, MaskSegment,
    ProjectConfiguration, SHAPE_MAX_ANIMATION_DURATION, SHAPE_MAX_CORNER_RADIUS,
    SHAPE_MAX_STROKE_WIDTH, ShapeAnimation, ShapeArrowHead, ShapeKind, ShapeSegment,
    StyleOverrides, TextAlign, TextAnimation, TextBackgroundStyle, TextLayout, TextSegment,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Map, Value};

use crate::editor_panels::{TEXT_FONT_SIZE_MAX, TEXT_FONT_SIZE_MIN};
use crate::i18n::t;

pub const STORE_SECTION: &str = "stylePresets";
pub const NAME_MAX_CHARS: usize = 64;
const FONT_FAMILY_MAX_CHARS: usize = 128;
const GENERIC_FAMILIES: [&str; 3] = ["sans-serif", "serif", "monospace"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PresetKind {
    Text,
    Mask,
    Captions,
    Style,
    Shape,
}

impl PresetKind {
    pub const fn key(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Mask => "mask",
            Self::Captions => "captions",
            Self::Style => "style",
            Self::Shape => "shape",
        }
    }
}

pub trait PresetStyle: Serialize + DeserializeOwned + Default + Clone + PartialEq {
    const KIND: PresetKind;

    fn sanitize(&mut self) {}
}

#[derive(Debug, Clone, PartialEq)]
pub struct StylePreset<S> {
    pub id: String,
    pub name: String,
    pub style: S,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StylePresets<S> {
    presets: Vec<StylePreset<S>>,
    default: Option<String>,
}

impl<S> Default for StylePresets<S> {
    fn default() -> Self {
        Self {
            presets: Vec::new(),
            default: None,
        }
    }
}

impl<S: PresetStyle> StylePresets<S> {
    pub fn from_section(section: &Map<String, Value>) -> Self {
        section
            .get(S::KIND.key())
            .map(Self::from_value)
            .unwrap_or_default()
    }

    pub fn from_value(value: &Value) -> Self {
        let mut presets: Vec<StylePreset<S>> = Vec::new();
        for entry in value
            .get("presets")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(id) = entry
                .get("id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|id| !id.is_empty())
            else {
                continue;
            };
            let Some(name) = entry
                .get("name")
                .and_then(Value::as_str)
                .and_then(clean_name)
            else {
                continue;
            };
            let Some(style) = entry.get("style").and_then(decode_style::<S>) else {
                continue;
            };
            if presets.iter().any(|preset| preset.id == id) {
                continue;
            }
            presets.push(StylePreset {
                id: id.to_string(),
                name,
                style,
            });
        }
        let default = value
            .get("default")
            .and_then(Value::as_str)
            .filter(|id| presets.iter().any(|preset| preset.id == *id))
            .map(str::to_string);
        Self { presets, default }
    }

    pub fn to_value(&self) -> Value {
        let presets = self
            .presets
            .iter()
            .map(|preset| {
                serde_json::json!({
                    "id": preset.id,
                    "name": preset.name,
                    "style": serde_json::to_value(&preset.style).unwrap_or(Value::Null),
                })
            })
            .collect();
        let mut map = Map::new();
        map.insert("presets".into(), Value::Array(presets));
        map.insert(
            "default".into(),
            self.default.clone().map_or(Value::Null, Value::String),
        );
        Value::Object(map)
    }

    pub fn update<R>(mutate: impl FnOnce(&mut Self) -> R) -> Option<(Self, R)> {
        let mut outcome = None;
        let written = crate::store::update_store_section(STORE_SECTION, |section| {
            let mut presets = Self::from_section(section);
            let result = mutate(&mut presets);
            section.insert(S::KIND.key().into(), presets.to_value());
            outcome = Some((presets, result));
            true
        });
        if !written {
            tracing::warn!(kind = S::KIND.key(), "the style presets could not be saved");
            return None;
        }
        outcome
    }

    pub fn presets(&self) -> &[StylePreset<S>] {
        &self.presets
    }

    pub fn is_empty(&self) -> bool {
        self.presets.is_empty()
    }

    pub fn get(&self, id: &str) -> Option<&StylePreset<S>> {
        self.presets.iter().find(|preset| preset.id == id)
    }

    pub fn position(&self, id: &str) -> Option<usize> {
        self.presets.iter().position(|preset| preset.id == id)
    }

    pub fn default_preset(&self) -> Option<&StylePreset<S>> {
        self.default.as_deref().and_then(|id| self.get(id))
    }

    pub fn default_style(&self) -> Option<&S> {
        self.default_preset().map(|preset| &preset.style)
    }

    pub fn is_default(&self, id: &str) -> bool {
        self.default.as_deref() == Some(id)
    }

    pub fn matching(&self, style: &S) -> Option<&StylePreset<S>> {
        self.default_preset()
            .filter(|preset| preset.style == *style)
            .or_else(|| self.presets.iter().find(|preset| preset.style == *style))
    }

    pub fn unique_name(&self, base: &str) -> String {
        let base = clean_name(base).unwrap_or_else(|| t!("My style").to_string());
        let taken = |name: &str| {
            self.presets
                .iter()
                .any(|preset| preset.name.to_lowercase() == name.to_lowercase())
        };
        if !taken(&base) {
            return base;
        }
        (2_usize..)
            .map(|suffix| format!("{base} {suffix}"))
            .find(|name| !taken(name))
            .unwrap_or(base)
    }

    pub fn add(&mut self, name: &str, mut style: S) -> Option<String> {
        let name = clean_name(name)?;
        style.sanitize();
        let mut id = crate::store::new_uuid_v4();
        while self.get(&id).is_some() {
            id = crate::store::new_uuid_v4();
        }
        self.presets.push(StylePreset {
            id: id.clone(),
            name,
            style,
        });
        Some(id)
    }

    pub fn set_style(&mut self, id: &str, mut style: S) -> bool {
        style.sanitize();
        match self.presets.iter_mut().find(|preset| preset.id == id) {
            Some(preset) => {
                preset.style = style;
                true
            }
            None => false,
        }
    }

    pub fn rename(&mut self, id: &str, name: &str) -> bool {
        let Some(name) = clean_name(name) else {
            return false;
        };
        match self.presets.iter_mut().find(|preset| preset.id == id) {
            Some(preset) => {
                preset.name = name;
                true
            }
            None => false,
        }
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let Some(index) = self.position(id) else {
            return false;
        };
        self.presets.remove(index);
        if self.default.as_deref() == Some(id) {
            self.default = None;
        }
        true
    }

    pub fn move_by(&mut self, id: &str, offset: isize) -> bool {
        let Some(index) = self.position(id) else {
            return false;
        };
        match index.checked_add_signed(offset) {
            Some(target) if target < self.presets.len() && target != index => {
                let preset = self.presets.remove(index);
                self.presets.insert(target, preset);
                true
            }
            _ => false,
        }
    }

    pub fn set_default(&mut self, id: Option<&str>) -> bool {
        match id {
            Some(id) if self.get(id).is_none() => false,
            Some(id) => {
                self.default = Some(id.to_string());
                true
            }
            None => self.default.take().is_some(),
        }
    }

    pub fn make_default_from(&mut self, mut style: S, name: &str) -> Option<String> {
        style.sanitize();
        let id = match self.presets.iter().find(|preset| preset.style == style) {
            Some(existing) => existing.id.clone(),
            None => {
                let name = self.unique_name(name);
                self.add(&name, style)?
            }
        };
        self.set_default(Some(&id));
        Some(id)
    }
}

pub fn clean_name(name: &str) -> Option<String> {
    let collapsed = name.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed: String = collapsed.chars().take(NAME_MAX_CHARS).collect();
    let trimmed = trimmed.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn decode_style<S: PresetStyle>(stored: &Value) -> Option<S> {
    let stored = stored.as_object()?;
    let mut style = serde_json::from_value::<S>(Value::Object(stored.clone()))
        .unwrap_or_else(|_| decode_field_by_field(stored));
    style.sanitize();
    Some(style)
}

fn decode_field_by_field<S: PresetStyle>(stored: &Map<String, Value>) -> S {
    let mut merged = match serde_json::to_value(S::default()) {
        Ok(Value::Object(map)) => map,
        _ => Map::new(),
    };
    for (key, value) in stored {
        let Some(previous) = merged.insert(key.clone(), value.clone()) else {
            merged.remove(key);
            continue;
        };
        if serde_json::from_value::<S>(Value::Object(merged.clone())).is_err() {
            merged.insert(key.clone(), previous);
        }
    }
    serde_json::from_value(Value::Object(merged)).unwrap_or_default()
}

fn finite_f32(value: f32, fallback: f32, min: f32, max: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback
    }
}

fn finite_f64(value: f64, fallback: f64, min: f64, max: f64) -> f64 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback
    }
}

fn valid_color(value: &mut String, fallback: &str) {
    if crate::editor_sidebar::normalize_hex(value).is_none() {
        *value = fallback.to_string();
    }
}

fn valid_optional_color(value: &mut Option<String>) {
    if value
        .as_deref()
        .is_some_and(|color| crate::editor_sidebar::normalize_hex(color).is_none())
    {
        *value = None;
    }
}

fn same_json<T: Serialize>(a: &T, b: &T) -> bool {
    match (serde_json::to_value(a), serde_json::to_value(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

pub fn font_is_available(family: &str, installed: &[String]) -> bool {
    let family = family.trim();
    installed.is_empty()
        || GENERIC_FAMILIES
            .iter()
            .any(|generic| generic.eq_ignore_ascii_case(family))
        || installed
            .iter()
            .any(|name| name.eq_ignore_ascii_case(family))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TextStyle {
    pub font_family: String,
    pub font_size: f32,
    pub font_weight: f32,
    pub italic: bool,
    pub uppercase: bool,
    pub align: TextAlign,
    pub letter_spacing: f32,
    pub line_height: f32,
    pub opacity: f32,
    pub shadow: f32,
    pub glow: f32,
    pub stroke_width: f32,
    pub stroke_color: String,
    pub background_style: TextBackgroundStyle,
    pub background_color: Option<String>,
    pub color: String,
    pub gradient_color: Option<String>,
    pub animation_in: TextAnimation,
    pub animation_out: TextAnimation,
    pub animation_in_duration: f64,
    pub animation_out_duration: f64,
    pub fade_duration: f64,
    pub layout: TextLayout,
    pub layout_transition: f64,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self::of(&crate::editor_edits::default_text_segment(0., 0., 0))
    }
}

impl TextStyle {
    pub fn of(segment: &TextSegment) -> Self {
        Self {
            font_family: segment.font_family.clone(),
            font_size: segment.font_size,
            font_weight: segment.font_weight,
            italic: segment.italic,
            uppercase: segment.uppercase,
            align: segment.align,
            letter_spacing: segment.letter_spacing,
            line_height: segment.line_height,
            opacity: segment.opacity,
            shadow: segment.shadow,
            glow: segment.glow,
            stroke_width: segment.stroke_width,
            stroke_color: segment.stroke_color.clone(),
            background_style: segment.background_style,
            background_color: segment.background_color.clone(),
            color: segment.color.clone(),
            gradient_color: segment.gradient_color.clone(),
            animation_in: segment.animation_in,
            animation_out: segment.animation_out,
            animation_in_duration: segment.animation_in_duration,
            animation_out_duration: segment.animation_out_duration,
            fade_duration: segment.fade_duration,
            layout: segment.layout,
            layout_transition: segment.layout_transition,
        }
    }

    pub fn apply_to(&self, segment: &mut TextSegment) -> bool {
        let before = (Self::of(segment), segment.center, segment.size);
        if segment.font_size != self.font_size {
            scale_text_box(segment, self.font_size);
        }
        segment.font_family = self.font_family.clone();
        segment.font_size = self.font_size;
        segment.font_weight = self.font_weight;
        segment.italic = self.italic;
        segment.uppercase = self.uppercase;
        segment.align = self.align;
        segment.letter_spacing = self.letter_spacing;
        segment.line_height = self.line_height;
        segment.opacity = self.opacity;
        segment.shadow = self.shadow;
        segment.glow = self.glow;
        segment.stroke_width = self.stroke_width;
        segment.stroke_color = self.stroke_color.clone();
        segment.background_style = self.background_style;
        segment.background_color = self.background_color.clone();
        segment.color = self.color.clone();
        segment.gradient_color = self.gradient_color.clone();
        segment.animation_in = self.animation_in;
        segment.animation_out = self.animation_out;
        segment.animation_in_duration = self.animation_in_duration;
        segment.animation_out_duration = self.animation_out_duration;
        segment.fade_duration = self.fade_duration;
        segment.layout = self.layout;
        segment.layout_transition = self.layout_transition;
        before != (Self::of(segment), segment.center, segment.size)
    }

    pub fn for_installed_fonts(&self, installed: &[String]) -> Self {
        let mut style = self.clone();
        if !font_is_available(&style.font_family, installed) {
            style.font_family = GENERIC_FAMILIES[0].to_string();
        }
        style
    }
}

fn scale_text_box(segment: &mut TextSegment, font_size: f32) {
    let previous = if segment.font_size == 0. {
        48.
    } else {
        segment.font_size
    };
    let box_scale = f64::from(font_size / previous);
    let top_edge = segment.center.y - segment.size.y / 2.;
    segment.size.x = (segment.size.x * box_scale).min(1.);
    segment.size.y *= box_scale;
    segment.center.y = top_edge + segment.size.y / 2.;
}

impl PresetStyle for TextStyle {
    const KIND: PresetKind = PresetKind::Text;

    fn sanitize(&mut self) {
        let fallback = Self::default();
        let family: String = self
            .font_family
            .trim()
            .chars()
            .take(FONT_FAMILY_MAX_CHARS)
            .collect();
        self.font_family = if family.is_empty() {
            fallback.font_family.clone()
        } else {
            family
        };
        self.font_size = finite_f32(
            self.font_size,
            fallback.font_size,
            TEXT_FONT_SIZE_MIN,
            TEXT_FONT_SIZE_MAX,
        );
        self.font_weight = finite_f32(self.font_weight, fallback.font_weight, 100., 900.);
        self.letter_spacing = finite_f32(self.letter_spacing, 0., -3., 20.);
        self.line_height = finite_f32(self.line_height, fallback.line_height, 0.8, 2.);
        self.opacity = finite_f32(self.opacity, 1., 0., 1.);
        self.shadow = finite_f32(self.shadow, 0., 0., 1.);
        self.glow = finite_f32(self.glow, 0., 0., 1.);
        self.stroke_width = finite_f32(self.stroke_width, 0., 0., 12.);
        self.animation_in_duration = finite_f64(
            self.animation_in_duration,
            fallback.animation_in_duration,
            0.,
            3.,
        );
        self.animation_out_duration = finite_f64(
            self.animation_out_duration,
            fallback.animation_out_duration,
            0.,
            3.,
        );
        self.fade_duration = finite_f64(self.fade_duration, fallback.fade_duration, 0., 3.);
        self.layout_transition =
            finite_f64(self.layout_transition, fallback.layout_transition, 0., 3.);
        valid_color(&mut self.color, &fallback.color);
        valid_color(&mut self.stroke_color, &fallback.stroke_color);
        valid_optional_color(&mut self.background_color);
        valid_optional_color(&mut self.gradient_color);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MaskStyle {
    pub mask_type: MaskKind,
    pub feather: f64,
    pub opacity: f64,
    pub pixelation: f64,
    pub darkness: f64,
    pub fade_duration: f64,
}

impl PartialEq for MaskStyle {
    fn eq(&self, other: &Self) -> bool {
        same_json(self, other)
    }
}

impl Default for MaskStyle {
    fn default() -> Self {
        Self::of(&crate::editor_edits::default_mask_segment(0., 0., 0))
    }
}

impl MaskStyle {
    pub fn of(segment: &MaskSegment) -> Self {
        Self {
            mask_type: segment.mask_type,
            feather: segment.feather,
            opacity: segment.opacity,
            pixelation: segment.pixelation,
            darkness: segment.darkness,
            fade_duration: segment.fade_duration,
        }
    }

    pub fn apply_to(&self, segment: &mut MaskSegment) -> bool {
        let animated = !segment.keyframes.intensity.is_empty();
        if Self::of(segment) == *self && !animated {
            return false;
        }
        segment.keyframes.intensity.clear();
        segment.mask_type = self.mask_type;
        segment.feather = self.feather;
        segment.opacity = self.opacity;
        segment.pixelation = self.pixelation;
        segment.darkness = self.darkness;
        segment.fade_duration = self.fade_duration;
        true
    }
}

impl PresetStyle for MaskStyle {
    const KIND: PresetKind = PresetKind::Mask;

    fn sanitize(&mut self) {
        let fallback = Self::default();
        self.feather = finite_f64(self.feather, fallback.feather, 0., 1.);
        self.opacity = finite_f64(self.opacity, fallback.opacity, 0., 1.);
        self.darkness = finite_f64(self.darkness, fallback.darkness, 0., 1.);
        self.fade_duration = finite_f64(self.fade_duration, fallback.fade_duration, 0., 1.);
        if !self.pixelation.is_finite() {
            self.pixelation = fallback.pixelation;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ShapeStyle {
    pub stroke_color: String,
    pub stroke_width: f32,
    pub fill: bool,
    pub fill_color: String,
    pub fill_opacity: f32,
    pub corner_radius: f32,
    pub arrow_head: ShapeArrowHead,
    pub shadow: f32,
    pub opacity: f32,
    pub animation_in: ShapeAnimation,
    pub animation_out: ShapeAnimation,
    pub animation_in_duration: f64,
    pub animation_out_duration: f64,
}

impl Default for ShapeStyle {
    fn default() -> Self {
        Self::of(&ShapeSegment::default())
    }
}

impl ShapeStyle {
    pub fn of(segment: &ShapeSegment) -> Self {
        Self {
            stroke_color: segment.stroke_color.clone(),
            stroke_width: segment.stroke_width,
            fill: segment.fill,
            fill_color: segment.fill_color.clone(),
            fill_opacity: segment.fill_opacity,
            corner_radius: segment.corner_radius,
            arrow_head: segment.arrow_head,
            shadow: segment.shadow,
            opacity: segment.opacity,
            animation_in: segment.animation_in,
            animation_out: segment.animation_out,
            animation_in_duration: segment.animation_in_duration,
            animation_out_duration: segment.animation_out_duration,
        }
    }

    pub fn apply_to(&self, segment: &mut ShapeSegment) -> bool {
        let previous = Self::of(segment);
        segment.stroke_color = self.stroke_color.clone();
        segment.stroke_width = self.stroke_width;
        segment.fill = self.fill || matches!(segment.kind, ShapeKind::Highlight | ShapeKind::Badge);
        segment.fill_color = self.fill_color.clone();
        segment.fill_opacity = self.fill_opacity;
        segment.corner_radius = self.corner_radius;
        segment.arrow_head = self.arrow_head;
        segment.shadow = self.shadow;
        segment.opacity = self.opacity;
        segment.animation_in = self.animation_in;
        segment.animation_out = self.animation_out;
        segment.animation_in_duration = self.animation_in_duration;
        segment.animation_out_duration = self.animation_out_duration;
        Self::of(segment) != previous
    }
}

impl PresetStyle for ShapeStyle {
    const KIND: PresetKind = PresetKind::Shape;

    fn sanitize(&mut self) {
        let fallback = Self::default();
        valid_color(&mut self.stroke_color, &fallback.stroke_color);
        valid_color(&mut self.fill_color, &fallback.fill_color);
        self.stroke_width = finite_f32(
            self.stroke_width,
            fallback.stroke_width,
            0.,
            SHAPE_MAX_STROKE_WIDTH,
        );
        self.fill_opacity = finite_f32(self.fill_opacity, fallback.fill_opacity, 0., 1.);
        self.corner_radius = finite_f32(
            self.corner_radius,
            fallback.corner_radius,
            0.,
            SHAPE_MAX_CORNER_RADIUS,
        );
        self.shadow = finite_f32(self.shadow, fallback.shadow, 0., 1.);
        self.opacity = finite_f32(self.opacity, fallback.opacity, 0., 1.);
        self.animation_in_duration = finite_f64(
            self.animation_in_duration,
            fallback.animation_in_duration,
            0.,
            SHAPE_MAX_ANIMATION_DURATION,
        );
        self.animation_out_duration = finite_f64(
            self.animation_out_duration,
            fallback.animation_out_duration,
            0.,
            SHAPE_MAX_ANIMATION_DURATION,
        );
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CaptionStyle(pub CaptionSettings);

impl PartialEq for CaptionStyle {
    fn eq(&self, other: &Self) -> bool {
        same_json(&self.0, &other.0)
    }
}

impl Default for CaptionStyle {
    fn default() -> Self {
        Self::of(&CaptionSettings::default())
    }
}

impl CaptionStyle {
    pub fn of(settings: &CaptionSettings) -> Self {
        let mut settings = settings.clone();
        settings.enabled = false;
        settings.export_with_subtitles = false;
        settings.show_filler_words = false;
        settings.filler_language = None;
        Self(settings)
    }

    pub fn applied_to(&self, current: &CaptionSettings) -> CaptionSettings {
        CaptionSettings {
            enabled: current.enabled,
            export_with_subtitles: current.export_with_subtitles,
            show_filler_words: current.show_filler_words,
            filler_language: current.filler_language.clone(),
            ..self.0.clone()
        }
    }

    pub fn apply_to(&self, settings: &mut CaptionSettings) -> bool {
        let next = self.applied_to(settings);
        if same_json(&next, settings) {
            return false;
        }
        *settings = next;
        true
    }
}

impl PresetStyle for CaptionStyle {
    const KIND: PresetKind = PresetKind::Captions;

    fn sanitize(&mut self) {
        let fallback = CaptionSettings::default();
        let settings = &mut self.0;
        settings.enabled = false;
        settings.export_with_subtitles = false;
        settings.show_filler_words = false;
        settings.filler_language = None;
        settings.size = settings.size.clamp(12, 100);
        settings.background_opacity = settings.background_opacity.min(100);
        settings.font_weight = settings.font_weight.clamp(100, 900);
        settings.fade_duration =
            finite_f32(settings.fade_duration, fallback.fade_duration, 0., 0.5);
        settings.linger_duration =
            finite_f32(settings.linger_duration, fallback.linger_duration, 0., 5.);
        settings.word_transition_duration = finite_f32(
            settings.word_transition_duration,
            fallback.word_transition_duration,
            0.,
            5.,
        );
        settings.manual_position = settings.manual_position.and_then(|position| {
            (position.x.is_finite() && position.y.is_finite()).then(|| {
                frameever_project::XY::new(position.x.clamp(0., 1.), position.y.clamp(0., 1.))
            })
        });
        valid_color(&mut settings.color, &fallback.color);
        valid_color(&mut settings.background_color, &fallback.background_color);
        valid_color(&mut settings.outline_color, &fallback.outline_color);
        valid_color(&mut settings.highlight_color, &fallback.highlight_color);
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct StyleTrackStyle {
    pub background: Option<BackgroundConfiguration>,
    pub camera: Option<Camera>,
    pub cursor: Option<CursorConfiguration>,
    pub camera_only_padding: Option<f64>,
}

impl PartialEq for StyleTrackStyle {
    fn eq(&self, other: &Self) -> bool {
        same_json(self, other)
    }
}

impl StyleTrackStyle {
    pub fn of(overrides: &StyleOverrides) -> Self {
        Self {
            background: overrides.background.clone(),
            camera: overrides.camera.clone(),
            cursor: overrides.cursor.clone(),
            camera_only_padding: overrides.camera_only_padding,
        }
    }

    pub fn apply_to(&self, overrides: &mut StyleOverrides) -> bool {
        if Self::of(overrides) == *self {
            return false;
        }
        overrides.background = self.background.clone();
        overrides.camera = self.camera.clone();
        overrides.cursor = self.cursor.clone();
        overrides.camera_only_padding = self.camera_only_padding;
        true
    }
}

impl PresetStyle for StyleTrackStyle {
    const KIND: PresetKind = PresetKind::Style;

    fn sanitize(&mut self) {
        self.camera_only_padding = self
            .camera_only_padding
            .filter(|padding| padding.is_finite())
            .map(|padding| padding.clamp(0., 40.));
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PresetLibrary {
    pub text: StylePresets<TextStyle>,
    pub mask: StylePresets<MaskStyle>,
    pub captions: StylePresets<CaptionStyle>,
    pub style: StylePresets<StyleTrackStyle>,
    pub shape: StylePresets<ShapeStyle>,
}

impl PresetLibrary {
    pub fn load() -> Self {
        Self::from_section(&crate::store::store_section(STORE_SECTION))
    }

    pub fn from_section(section: &Map<String, Value>) -> Self {
        Self {
            text: StylePresets::from_section(section),
            mask: StylePresets::from_section(section),
            captions: StylePresets::from_section(section),
            style: StylePresets::from_section(section),
            shape: StylePresets::from_section(section),
        }
    }
}

pub trait PresetTarget: PresetStyle {
    fn presets(library: &PresetLibrary) -> &StylePresets<Self>;

    fn presets_mut(library: &mut PresetLibrary) -> &mut StylePresets<Self>;

    fn count(project: &ProjectConfiguration) -> usize;

    fn read(project: &ProjectConfiguration, index: usize) -> Option<Self>;

    fn write(&self, project: &mut ProjectConfiguration, index: usize) -> bool;

    fn for_installed(&self, _installed: &[String]) -> Self {
        self.clone()
    }

    fn missing_font(&self, _installed: &[String]) -> Option<&str> {
        None
    }
}

impl PresetTarget for TextStyle {
    fn presets(library: &PresetLibrary) -> &StylePresets<Self> {
        &library.text
    }

    fn presets_mut(library: &mut PresetLibrary) -> &mut StylePresets<Self> {
        &mut library.text
    }

    fn count(project: &ProjectConfiguration) -> usize {
        project
            .timeline
            .as_ref()
            .map_or(0, |timeline| timeline.text_segments.len())
    }

    fn read(project: &ProjectConfiguration, index: usize) -> Option<Self> {
        let segment = project.timeline.as_ref()?.text_segments.get(index)?;
        Some(Self::of(segment))
    }

    fn write(&self, project: &mut ProjectConfiguration, index: usize) -> bool {
        project
            .timeline
            .as_mut()
            .and_then(|timeline| timeline.text_segments.get_mut(index))
            .is_some_and(|segment| self.apply_to(segment))
    }

    fn for_installed(&self, installed: &[String]) -> Self {
        self.for_installed_fonts(installed)
    }

    fn missing_font(&self, installed: &[String]) -> Option<&str> {
        (!font_is_available(&self.font_family, installed)).then_some(self.font_family.as_str())
    }
}

impl PresetTarget for MaskStyle {
    fn presets(library: &PresetLibrary) -> &StylePresets<Self> {
        &library.mask
    }

    fn presets_mut(library: &mut PresetLibrary) -> &mut StylePresets<Self> {
        &mut library.mask
    }

    fn count(project: &ProjectConfiguration) -> usize {
        project
            .timeline
            .as_ref()
            .map_or(0, |timeline| timeline.mask_segments.len())
    }

    fn read(project: &ProjectConfiguration, index: usize) -> Option<Self> {
        let segment = project.timeline.as_ref()?.mask_segments.get(index)?;
        Some(Self::of(segment))
    }

    fn write(&self, project: &mut ProjectConfiguration, index: usize) -> bool {
        project
            .timeline
            .as_mut()
            .and_then(|timeline| timeline.mask_segments.get_mut(index))
            .is_some_and(|segment| self.apply_to(segment))
    }
}

impl PresetTarget for CaptionStyle {
    fn presets(library: &PresetLibrary) -> &StylePresets<Self> {
        &library.captions
    }

    fn presets_mut(library: &mut PresetLibrary) -> &mut StylePresets<Self> {
        &mut library.captions
    }

    fn count(project: &ProjectConfiguration) -> usize {
        usize::from(project.captions.is_some())
    }

    fn read(project: &ProjectConfiguration, _index: usize) -> Option<Self> {
        Some(Self::of(
            &project
                .captions
                .as_ref()
                .map(|captions| captions.settings.clone())
                .unwrap_or_default(),
        ))
    }

    fn write(&self, project: &mut ProjectConfiguration, _index: usize) -> bool {
        project
            .captions
            .as_mut()
            .is_some_and(|captions| self.apply_to(&mut captions.settings))
    }
}

impl PresetTarget for StyleTrackStyle {
    fn presets(library: &PresetLibrary) -> &StylePresets<Self> {
        &library.style
    }

    fn presets_mut(library: &mut PresetLibrary) -> &mut StylePresets<Self> {
        &mut library.style
    }

    fn count(project: &ProjectConfiguration) -> usize {
        project
            .timeline
            .as_ref()
            .map_or(0, |timeline| timeline.style_segments.len())
    }

    fn read(project: &ProjectConfiguration, index: usize) -> Option<Self> {
        let segment = project.timeline.as_ref()?.style_segments.get(index)?;
        Some(Self::of(&segment.overrides))
    }

    fn write(&self, project: &mut ProjectConfiguration, index: usize) -> bool {
        project
            .timeline
            .as_mut()
            .and_then(|timeline| timeline.style_segments.get_mut(index))
            .is_some_and(|segment| self.apply_to(&mut segment.overrides))
    }
}

impl PresetTarget for ShapeStyle {
    fn presets(library: &PresetLibrary) -> &StylePresets<Self> {
        &library.shape
    }

    fn presets_mut(library: &mut PresetLibrary) -> &mut StylePresets<Self> {
        &mut library.shape
    }

    fn count(project: &ProjectConfiguration) -> usize {
        project
            .timeline
            .as_ref()
            .map_or(0, |timeline| timeline.shape_segments.len())
    }

    fn read(project: &ProjectConfiguration, index: usize) -> Option<Self> {
        let segment = project.timeline.as_ref()?.shape_segments.get(index)?;
        Some(Self::of(segment))
    }

    fn write(&self, project: &mut ProjectConfiguration, index: usize) -> bool {
        project
            .timeline
            .as_mut()
            .and_then(|timeline| timeline.shape_segments.get_mut(index))
            .is_some_and(|segment| self.apply_to(segment))
    }
}

pub fn apply_to_items<S: PresetTarget>(
    project: &mut ProjectConfiguration,
    indices: &[usize],
    style: &S,
    installed: &[String],
) -> bool {
    let style = style.for_installed(installed);
    let mut changed = false;
    for index in indices {
        changed |= style.write(project, *index);
    }
    changed
}

pub fn apply_to_all<S: PresetTarget>(
    project: &mut ProjectConfiguration,
    style: &S,
    installed: &[String],
) -> bool {
    let indices: Vec<usize> = (0..S::count(project)).collect();
    apply_to_items(project, &indices, style, installed)
}

fn default_style<S: PresetTarget>(library: &PresetLibrary, installed: &[String]) -> Option<S> {
    S::presets(library)
        .default_style()
        .map(|style| style.for_installed(installed))
}

pub fn new_text_segment(
    library: &PresetLibrary,
    installed: &[String],
    start: f64,
    end: f64,
    track: u32,
) -> TextSegment {
    let mut segment = crate::editor_edits::default_text_segment(start, end, track);
    if let Some(style) = default_style::<TextStyle>(library, installed) {
        style.apply_to(&mut segment);
    }
    segment
}

pub fn new_mask_segment(library: &PresetLibrary, start: f64, end: f64, track: u32) -> MaskSegment {
    let mut segment = crate::editor_edits::default_mask_segment(start, end, track);
    if let Some(style) = default_style::<MaskStyle>(library, &[]) {
        style.apply_to(&mut segment);
    }
    segment
}

pub fn new_shape_segment(
    library: &PresetLibrary,
    kind: ShapeKind,
    start: f64,
    end: f64,
    center: frameever_project::XY<f64>,
    aspect: f64,
) -> ShapeSegment {
    let mut segment = ShapeSegment::new(kind, start, end, center, aspect);
    if let Some(style) = default_style::<ShapeStyle>(library, &[]) {
        style.apply_to(&mut segment);
    }
    segment
}

pub fn new_style_segment(
    library: &PresetLibrary,
    start: f64,
    end: f64,
    track: u32,
) -> frameever_project::StyleSegment {
    let mut segment = frameever_project::StyleSegment {
        start,
        end,
        track,
        ..Default::default()
    };
    if let Some(style) = default_style::<StyleTrackStyle>(library, &[]) {
        style.apply_to(&mut segment.overrides);
    }
    segment
}

pub fn seed_default_captions(library: &PresetLibrary, project: &mut ProjectConfiguration) {
    if project.captions.is_some() {
        return;
    }
    let Some(style) = default_style::<CaptionStyle>(library, &[]) else {
        return;
    };
    project.captions = Some(frameever_project::CaptionsData {
        settings: style.applied_to(&CaptionSettings::default()),
        ..Default::default()
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn text_project(segments: Vec<TextSegment>) -> ProjectConfiguration {
        let mut timeline: frameever_project::TimelineConfiguration =
            serde_json::from_value(json!({ "segments": [], "zoomSegments": [] })).unwrap();
        timeline.text_segments = segments;
        ProjectConfiguration {
            timeline: Some(timeline),
            ..ProjectConfiguration::default()
        }
    }

    fn red_text() -> TextStyle {
        let mut segment = crate::editor_edits::default_text_segment(0., 1., 0);
        segment.color = "#ff0000".into();
        segment.font_size = 96.;
        TextStyle::of(&segment)
    }

    #[test]
    fn a_saved_style_preset_survives_the_store_and_is_matched_again() {
        let mut presets = StylePresets::<TextStyle>::default();
        let id = presets.add("  Big   red  ", red_text()).unwrap();
        presets.set_default(Some(&id));

        let reloaded = StylePresets::<TextStyle>::from_value(&presets.to_value());

        assert_eq!(reloaded.presets()[0].name, "Big red");
        assert!(reloaded.is_default(&id));
        assert_eq!(
            reloaded
                .matching(&red_text())
                .map(|preset| preset.id.as_str()),
            Some(id.as_str())
        );
        assert!(reloaded.matching(&TextStyle::default()).is_none());
    }

    #[test]
    fn style_presets_rename_reorder_and_forget_their_default_when_deleted() {
        let mut presets = StylePresets::<MaskStyle>::default();
        let first = presets.add("One", MaskStyle::default()).unwrap();
        let second = presets
            .add(
                "Two",
                MaskStyle {
                    darkness: 0.9,
                    ..MaskStyle::default()
                },
            )
            .unwrap();
        presets.set_default(Some(&second));

        assert!(presets.rename(&first, "First"));
        assert!(!presets.rename(&first, "   "));
        assert!(presets.move_by(&second, -1));
        assert!(!presets.move_by(&second, -1));
        let names: Vec<_> = presets.presets().iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Two", "First"]);

        assert!(presets.remove(&second));
        assert!(presets.default_style().is_none());
        assert_eq!(presets.unique_name("First"), "First 2");
    }

    #[test]
    fn using_a_style_for_new_items_reuses_a_matching_preset() {
        let mut presets = StylePresets::<TextStyle>::default();
        let existing = presets.add("Red", red_text()).unwrap();

        let id = presets.make_default_from(red_text(), "My style").unwrap();
        assert_eq!(id, existing);
        assert_eq!(presets.presets().len(), 1);

        let fresh = presets
            .make_default_from(TextStyle::default(), "Red")
            .unwrap();
        assert_eq!(presets.presets().len(), 2);
        assert_eq!(presets.get(&fresh).unwrap().name, "Red 2");
        assert!(presets.is_default(&fresh));
    }

    #[test]
    fn a_stored_style_with_a_broken_field_keeps_the_rest() {
        let stored = json!({
            "presets": [
                { "id": "a", "name": "Odd", "style": { "color": "#00ff00", "fontSize": "huge", "fontWeight": 9999 } },
                { "id": "a", "name": "Duplicate", "style": {} },
                { "name": "No id", "style": {} }
            ],
            "default": "missing"
        });

        let presets = StylePresets::<TextStyle>::from_value(&stored);

        assert_eq!(presets.presets().len(), 1);
        let style = &presets.presets()[0].style;
        assert_eq!(style.color, "#00ff00");
        assert_eq!(style.font_size, TextStyle::default().font_size);
        assert_eq!(style.font_weight, 900.);
        assert!(presets.default_preset().is_none());
    }

    #[test]
    fn applying_a_text_style_keeps_content_and_timing_and_scales_the_box() {
        let mut segment = crate::editor_edits::default_text_segment(2., 5., 1);
        segment.content = "Hello".into();
        let top = segment.center.y - segment.size.y / 2.;
        let mut project = text_project(vec![segment.clone(), segment]);

        assert!(apply_to_items(&mut project, &[1], &red_text(), &[]));

        let segments = &project.timeline.as_ref().unwrap().text_segments;
        assert_eq!(segments[0].color, "#ffffff");
        let applied = &segments[1];
        assert_eq!((applied.start, applied.end, applied.track), (2., 5., 1));
        assert_eq!(applied.content, "Hello");
        assert_eq!(applied.color, "#ff0000");
        assert!((applied.size.y - 0.11).abs() < 1e-9);
        assert!((applied.center.y - applied.size.y / 2. - top).abs() < 1e-9);
        assert!(!apply_to_items(&mut project, &[1], &red_text(), &[]));
    }

    #[test]
    fn a_text_style_whose_font_is_missing_falls_back_to_sans_serif() {
        let style = TextStyle {
            font_family: "Not A Font".into(),
            ..TextStyle::default()
        };
        let installed = vec!["Inter".to_string()];
        assert_eq!(style.missing_font(&installed), Some("Not A Font"));
        let mut project = text_project(vec![crate::editor_edits::default_text_segment(0., 1., 0)]);
        apply_to_all(&mut project, &style, &installed);
        assert_eq!(
            project.timeline.unwrap().text_segments[0].font_family,
            "sans-serif"
        );
    }

    #[test]
    fn applying_a_mask_style_clears_intensity_keyframes() {
        let mut segment = crate::editor_edits::default_mask_segment(0., 1., 0);
        segment
            .keyframes
            .intensity
            .push(frameever_project::MaskScalarKeyframe {
                time: 0.5,
                value: 0.2,
            });
        assert!(MaskStyle::of(&segment).apply_to(&mut segment));
        assert!(segment.keyframes.intensity.is_empty());
    }

    #[test]
    fn a_caption_style_leaves_whether_captions_show_alone() {
        let mut source = CaptionSettings {
            color: "#ffcc00".into(),
            enabled: true,
            show_filler_words: true,
            ..CaptionSettings::default()
        };
        source.size = 64;
        let style = CaptionStyle::of(&source);
        let mut target = CaptionSettings::default();

        assert!(style.apply_to(&mut target));

        assert_eq!(target.color, "#ffcc00");
        assert_eq!(target.size, 64);
        assert!(!target.enabled);
        assert!(!target.show_filler_words);
    }

    #[test]
    fn new_items_start_from_the_default_style_preset() {
        let mut library = PresetLibrary::default();
        let id = library.text.add("Red", red_text()).unwrap();
        library.text.set_default(Some(&id));
        let padded = StyleTrackStyle {
            camera_only_padding: Some(12.),
            ..StyleTrackStyle::default()
        };
        let id = library.style.add("Padded", padded).unwrap();
        library.style.set_default(Some(&id));
        let id = library
            .captions
            .add(
                "Yellow",
                CaptionStyle::of(&CaptionSettings {
                    color: "#ffff00".into(),
                    ..CaptionSettings::default()
                }),
            )
            .unwrap();
        library.captions.set_default(Some(&id));

        let text = new_text_segment(&library, &[], 1., 3., 2);
        assert_eq!(text.color, "#ff0000");
        assert_eq!((text.start, text.end, text.track), (1., 3., 2));
        assert_eq!(
            new_style_segment(&library, 0., 1., 0)
                .overrides
                .camera_only_padding,
            Some(12.)
        );
        assert_eq!(
            new_mask_segment(&library, 0., 1., 0).darkness,
            crate::editor_edits::default_mask_segment(0., 1., 0).darkness
        );
        let mut project = ProjectConfiguration::default();
        seed_default_captions(&library, &mut project);
        assert_eq!(project.captions.unwrap().settings.color, "#ffff00");
    }

    fn blue_outline() -> ShapeStyle {
        ShapeStyle {
            stroke_color: "#2563EB".to_string(),
            stroke_width: 10.,
            fill: false,
            fill_color: "#93C5FD".to_string(),
            fill_opacity: 0.5,
            corner_radius: 4.,
            arrow_head: ShapeArrowHead::Open,
            shadow: 0.,
            opacity: 0.8,
            animation_in: ShapeAnimation::Scale,
            animation_out: ShapeAnimation::None,
            animation_in_duration: 0.6,
            animation_out_duration: 0.,
        }
    }

    fn project_with_shapes(kinds: &[ShapeKind]) -> ProjectConfiguration {
        let mut project = text_project(Vec::new());
        project.timeline.as_mut().unwrap().shape_segments = kinds
            .iter()
            .enumerate()
            .map(|(index, kind)| {
                let mut shape = ShapeSegment::new(
                    *kind,
                    index as f64,
                    index as f64 + 2.,
                    frameever_project::XY::new(0.3, 0.6),
                    16. / 9.,
                );
                shape.track = index as u32;
                shape.number = index as u32 + 1;
                shape
            })
            .collect();
        project
    }

    #[test]
    fn shape_styles_carry_the_look_but_not_kind_geometry_timing_or_number() {
        let mut project = project_with_shapes(&[ShapeKind::Arrow, ShapeKind::Rectangle]);
        let before = project.timeline.as_ref().unwrap().shape_segments.clone();
        assert!(apply_to_all(&mut project, &blue_outline(), &[]));
        let after = &project.timeline.as_ref().unwrap().shape_segments;
        for (old, new) in before.iter().zip(after) {
            assert_eq!(ShapeStyle::of(new), blue_outline());
            assert_eq!(new.kind, old.kind);
            assert_eq!((new.from, new.to), (old.from, old.to));
            assert_eq!(
                (new.start, new.end, new.track),
                (old.start, old.end, old.track)
            );
            assert_eq!(new.number, old.number);
        }
        assert!(!apply_to_all(&mut project, &blue_outline(), &[]));
    }

    #[test]
    fn highlights_and_badges_keep_their_fill_under_an_outline_style() {
        let mut project =
            project_with_shapes(&[ShapeKind::Highlight, ShapeKind::Badge, ShapeKind::Ellipse]);
        assert!(apply_to_items(
            &mut project,
            &[0, 1, 2, 9],
            &blue_outline(),
            &[]
        ));
        let shapes = &project.timeline.as_ref().unwrap().shape_segments;
        assert!(shapes[0].fill);
        assert!(shapes[1].fill);
        assert!(!shapes[2].fill);
    }

    #[test]
    fn hostile_shape_styles_are_clamped_to_the_inspector_ranges() {
        let mut style = ShapeStyle {
            stroke_color: "not a colour".to_string(),
            stroke_width: f32::NAN,
            corner_radius: 5000.,
            opacity: -1.,
            animation_in_duration: 1e9,
            ..blue_outline()
        };
        style.sanitize();
        let fallback = ShapeStyle::default();
        assert_eq!(style.stroke_color, fallback.stroke_color);
        assert_eq!(style.stroke_width, fallback.stroke_width);
        assert_eq!(style.corner_radius, SHAPE_MAX_CORNER_RADIUS);
        assert_eq!(style.opacity, 0.);
        assert_eq!(style.animation_in_duration, SHAPE_MAX_ANIMATION_DURATION);
    }

    #[test]
    fn new_shapes_start_from_the_default_shape_preset() {
        let center = frameever_project::XY::new(0.5, 0.5);
        let mut library = PresetLibrary::default();
        let plain = new_shape_segment(&library, ShapeKind::Rectangle, 1., 3., center, 16. / 9.);
        assert_eq!(
            plain,
            ShapeSegment::new(ShapeKind::Rectangle, 1., 3., center, 16. / 9.)
        );
        library.shape.make_default_from(blue_outline(), "Mine");
        let shape = new_shape_segment(&library, ShapeKind::Ellipse, 4., 6., center, 16. / 9.);
        assert_eq!(ShapeStyle::of(&shape), blue_outline());
        assert_eq!((shape.start, shape.end), (4., 6.));
        let badge = new_shape_segment(&library, ShapeKind::Badge, 0., 2., center, 16. / 9.);
        assert!(badge.fill);
    }

    #[test]
    fn style_presets_of_each_kind_save_to_their_own_store_entry() {
        let dir = tempfile::tempdir().unwrap();
        crate::store::set_test_tauri_store_path(Some(dir.path().join("store.json")));

        let (_, id) =
            StylePresets::<TextStyle>::update(|presets| presets.add("Red", red_text()).unwrap())
                .unwrap();
        StylePresets::<MaskStyle>::update(|presets| presets.add("Mask", MaskStyle::default()))
            .unwrap();

        let library = PresetLibrary::load();
        crate::store::set_test_tauri_store_path(None);
        assert_eq!(library.text.presets()[0].id, id);
        assert_eq!(library.mask.presets()[0].name, "Mask");
        assert!(library.captions.is_empty());
    }
}
