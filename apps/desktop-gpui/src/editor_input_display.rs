use std::sync::Arc;

use frameever_project::{
    InputDisplay, InputDisplayStyle, ProjectConfiguration, XY,
    input_display::{
        BuiltinPresetId, DisplayState, INPUT_DISPLAY_DEFAULT_SIZE, INPUT_DISPLAY_MARGIN,
        INPUT_DISPLAY_OPACITY_RANGE, INPUT_DISPLAY_SIZE_RANGE, INPUT_DISPLAY_SNAP_STOPS,
        display_rect, position_for_origin, preset_for_recording, size_for_width, snap_position,
    },
};
use frameever_rendering::{
    input_display::PresetShape,
    input_presets::{ImportRefusal, UserPreset},
};
use gpui::{
    AnyElement, Context, FontWeight, Hsla, InteractiveElement, IntoElement, ParentElement,
    RenderImage, SharedString, StatefulInteractiveElement, Styled, StyledImage, Window, div, img,
    prelude::FluentBuilder, px, rgb, svg,
};

use crate::{
    editor_canvas::{GuideAxis, NormRect, SnapGuide, SnapLineKind},
    editor_window::EditorWindow,
    i18n::{n, t, tc, tr},
    library, ui,
};

const TILE_COLUMNS: f32 = 2.;
const TILE_GAP: f32 = 6.;
const TILE_HEIGHT: f32 = 86.;
const TILE_PICTURE_HEIGHT: f32 = 46.;
pub(crate) const PICTURE_BACKDROP: u32 = 0x2a2f36;
const TILE_INK: u32 = 0xd8d8d8;
const TILE_ACTION_ICON: f32 = 14.;

pub(crate) type InputThumbnailKey = (String, InputDisplayStyle);

const THUMBNAIL_WIDTH: u32 = 320;

pub(crate) struct InputImportError {
    pub message: SharedString,
    pub details: SharedString,
    pub open: bool,
}

pub(crate) fn import_refusal_message(refusal: &ImportRefusal) -> String {
    use frameever_project::input_display::PresetRefusal;
    match refusal {
        ImportRefusal::FolderUnreadable { .. } => t!(
            "Couldn't import this preset. FrameEver couldn't read that folder. Check that it still exists and try again."
        )
        .to_string(),
        ImportRefusal::NoPresetFile => t!(
            "Couldn't import this preset. The folder has no preset JSON file. Choose the folder that holds the preset's .json and .png files."
        )
        .to_string(),
        ImportRefusal::TooManyPresetFiles { .. } => t!(
            "Couldn't import this preset. The folder has more preset files than FrameEver takes at once. Put the ones you want in their own folder and import that."
        )
        .to_string(),
        ImportRefusal::NotSaved { .. } => t!(
            "Couldn't import this preset. FrameEver couldn't save it. Check free disk space and try again."
        )
        .to_string(),
        ImportRefusal::File { file, refusal } => match refusal {
            PresetRefusal::MissingAtlas => t!(
                "Couldn't import this preset. {file} has no image beside it. Put its .png file in the same folder and try again.",
                file = file
            ),
            PresetRefusal::JsonTooLarge { .. }
            | PresetRefusal::AtlasFileTooLarge { .. }
            | PresetRefusal::AtlasTooLarge { .. }
            | PresetRefusal::CanvasTooLarge { .. }
            | PresetRefusal::TooManyElements { .. } => t!(
                "Couldn't import this preset. {file} is larger than FrameEver allows. Choose a smaller preset.",
                file = file
            ),
            _ => t!(
                "Couldn't import this preset. {file} isn't a preset FrameEver can read. Save it again in its preset editor, or choose another preset.",
                file = file
            ),
        },
    }
}

pub(crate) const INPUT_PRESETS: [(BuiltinPresetId, &str); 3] = [
    (BuiltinPresetId::WasdMouse, n!("WASD + mouse")),
    (BuiltinPresetId::FullKeyboard, n!("Full keyboard")),
    (BuiltinPresetId::Gamepad, n!("Gamepad")),
];

const INPUT_SETUP_DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(250);

#[derive(Default)]
pub(crate) struct PendingInputSetup(Option<InputDisplay>);

impl PendingInputSetup {
    pub(crate) fn queue(&mut self, setup: InputDisplay) {
        self.0 = Some(setup);
    }

    pub(crate) fn flush<R>(&mut self, save: impl FnOnce(&InputDisplay) -> R) -> Option<R> {
        self.0.take().map(|setup| save(&setup))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputDisplaySlider {
    Size,
    Opacity,
}

impl InputDisplaySlider {
    pub fn limits(self) -> (f32, f32, f32) {
        let (min, max) = match self {
            Self::Size => INPUT_DISPLAY_SIZE_RANGE,
            Self::Opacity => INPUT_DISPLAY_OPACITY_RANGE,
        };
        (min as f32, max as f32, 1.)
    }

    pub fn read(self, project: &ProjectConfiguration) -> f32 {
        let block = project.input_display.clone().unwrap_or_default();
        match self {
            Self::Size => block.size as f32,
            Self::Opacity => block.opacity as f32,
        }
    }

    pub fn write(self, project: &mut ProjectConfiguration, value: f32) -> bool {
        match self {
            Self::Size => set_input_display_size(project, f64::from(value)),
            Self::Opacity => set_input_display_opacity(project, f64::from(value)),
        }
    }
}

fn style_label(style: InputDisplayStyle) -> &'static str {
    match style {
        InputDisplayStyle::Keycaps => t!("Keycaps"),
        InputDisplayStyle::Outline => tc!("input display style", "Outline"),
        InputDisplayStyle::Ghost => t!("Ghost"),
    }
}

pub(crate) fn switch_input_display(project: &mut ProjectConfiguration, on: bool) -> bool {
    if project.shown_input_display().is_some() == on {
        return false;
    }
    project
        .input_display
        .get_or_insert_with(InputDisplay::default)
        .enabled = on;
    true
}

pub(crate) fn switch_input_display_for_recording(
    project: &mut ProjectConfiguration,
    on: bool,
    project_path: &std::path::Path,
) -> bool {
    if !switch_input_display(project, on) {
        return false;
    }
    if let (true, Some(block), Ok(meta)) = (
        on,
        project
            .input_display
            .as_mut()
            .filter(|block| !block.preset_chosen),
        frameever_project::RecordingMeta::load_for_project(project_path),
    ) {
        block.preset = preset_for_recording(&meta, &block.preset).to_string();
    }
    true
}

pub(crate) fn choose_input_preset(project: &mut ProjectConfiguration, preset: &str) -> bool {
    let block = project
        .input_display
        .get_or_insert_with(InputDisplay::default);
    if block.preset == preset && block.preset_chosen {
        return false;
    }
    block.preset = preset.to_string();
    block.preset_chosen = true;
    true
}

pub(crate) fn after_preset_removed(project: &mut ProjectConfiguration, removed: &str) -> bool {
    match project.input_display.as_mut() {
        Some(block) if block.preset == removed => {
            block.preset = BuiltinPresetId::WasdMouse.id().to_string();
            block.preset_chosen = false;
            true
        }
        _ => false,
    }
}

pub(crate) fn choose_input_style(
    project: &mut ProjectConfiguration,
    style: InputDisplayStyle,
) -> bool {
    let block = project
        .input_display
        .get_or_insert_with(InputDisplay::default);
    if block.style == style {
        return false;
    }
    block.style = style;
    true
}

fn input_display_block(project: &mut ProjectConfiguration) -> &mut InputDisplay {
    project
        .input_display
        .get_or_insert_with(InputDisplay::default)
}

pub(crate) fn place_input_display(project: &mut ProjectConfiguration, position: XY<f64>) -> bool {
    let block = input_display_block(project);
    let position = XY::new(position.x.clamp(0., 1.), position.y.clamp(0., 1.));
    if block.position == position {
        return false;
    }
    block.position = position;
    true
}

pub(crate) fn resize_input_display(
    project: &mut ProjectConfiguration,
    size: f64,
    position: XY<f64>,
) -> bool {
    let sized = set_input_display_size(project, size);
    place_input_display(project, position) || sized
}

pub(crate) fn set_input_display_size(project: &mut ProjectConfiguration, size: f64) -> bool {
    let block = input_display_block(project);
    let size = size
        .round()
        .clamp(INPUT_DISPLAY_SIZE_RANGE.0, INPUT_DISPLAY_SIZE_RANGE.1);
    if block.size == size {
        return false;
    }
    block.size = size;
    true
}

pub(crate) fn set_input_display_opacity(project: &mut ProjectConfiguration, opacity: f64) -> bool {
    let block = input_display_block(project);
    let opacity = opacity
        .round()
        .clamp(INPUT_DISPLAY_OPACITY_RANGE.0, INPUT_DISPLAY_OPACITY_RANGE.1);
    if block.opacity == opacity {
        return false;
    }
    block.opacity = opacity;
    true
}

pub(crate) fn input_display_rect(
    block: &InputDisplay,
    shape: PresetShape,
    output: (u32, u32),
) -> NormRect {
    let rect = display_rect(block, shape.size, shape.frame_share, output);
    let (width, height) = (f64::from(output.0.max(1)), f64::from(output.1.max(1)));
    NormRect {
        x: rect.x / width,
        y: rect.y / height,
        w: rect.width / width,
        h: rect.height / height,
    }
}

fn position_of(rect: NormRect, output: (u32, u32)) -> XY<f64> {
    let (width, height) = (f64::from(output.0), f64::from(output.1));
    position_for_origin(
        (rect.x * width, rect.y * height),
        (rect.w * width, rect.h * height),
        output,
    )
}

pub(crate) fn input_drag_position(
    start: NormRect,
    output: (u32, u32),
    canvas: (f64, f64),
    delta: (f64, f64),
    shift: bool,
) -> (XY<f64>, Vec<SnapGuide>) {
    let raw = position_of(
        NormRect {
            x: start.x + delta.0 / canvas.0.max(1.),
            y: start.y + delta.1 / canvas.1.max(1.),
            ..start
        },
        output,
    );
    if shift {
        return (raw, Vec::new());
    }
    let (position, stops) = snap_position(raw);
    let margin_x = INPUT_DISPLAY_MARGIN;
    let margin_y = INPUT_DISPLAY_MARGIN * f64::from(output.0) / f64::from(output.1.max(1));
    let guide = |axis: GuideAxis, stop: f64, margin: f64| SnapGuide {
        axis,
        pos: margin + stop * (1. - 2. * margin),
        start: 0.,
        end: 1.,
        kind: if stop == 0.5 {
            SnapLineKind::FrameCenter
        } else {
            SnapLineKind::Margin
        },
    };
    let guides = [
        stops[0].map(|stop| guide(GuideAxis::V, stop, margin_x)),
        stops[1].map(|stop| guide(GuideAxis::H, stop, margin_y)),
    ]
    .into_iter()
    .flatten()
    .collect();
    (position, guides)
}

pub(crate) fn input_nudge_position(
    rect: NormRect,
    output: (u32, u32),
    canvas: (f64, f64),
    direction: (f64, f64),
    shift: bool,
) -> XY<f64> {
    let step = if shift { 10. } else { 1. };
    position_of(
        NormRect {
            x: rect.x + direction.0 * step / canvas.0.max(1.),
            y: rect.y + direction.1 * step / canvas.1.max(1.),
            ..rect
        },
        output,
    )
}

pub(crate) fn input_resize(
    start: NormRect,
    frame_share: f64,
    output: (u32, u32),
    canvas: (f64, f64),
    delta: (f64, f64),
    direction: (i8, i8),
) -> (f64, XY<f64>) {
    let (output_width, output_height) = (f64::from(output.0), f64::from(output.1));
    let (width, height) = (start.w * output_width, start.h * output_height);
    let grow = |length: f64, travel: f64, canvas: f64, output: f64, direction: i8| {
        if length > 0. {
            (length + travel / canvas.max(1.) * output * f64::from(direction)) / length
        } else {
            1.
        }
    };
    let scale = grow(width, delta.0, canvas.0, output_width, direction.0).max(grow(
        height,
        delta.1,
        canvas.1,
        output_height,
        direction.1,
    ));
    let size = size_for_width(width * scale.max(0.), frame_share, output.0);
    let sized_width = output_width * frame_share * size / INPUT_DISPLAY_DEFAULT_SIZE;
    let sized_height = if width > 0. {
        sized_width * height / width
    } else {
        height
    };
    let left = if direction.0 < 0 {
        (start.x + start.w) * output_width - sized_width
    } else {
        start.x * output_width
    };
    let top = if direction.1 < 0 {
        (start.y + start.h) * output_height - sized_height
    } else {
        start.y * output_height
    };
    (
        size,
        position_for_origin((left, top), (sized_width, sized_height), output),
    )
}

fn thumbnail(preset: &str, style: InputDisplayStyle) -> Option<Arc<RenderImage>> {
    let loaded = frameever_rendering::input_display::resolve_preset(preset, style, 1)?;
    let mut picture =
        frameever_rendering::input_display::compose(&loaded, &DisplayState::default());
    if picture.width() > THUMBNAIL_WIDTH {
        let height = (u64::from(picture.height()) * u64::from(THUMBNAIL_WIDTH)
            / u64::from(picture.width()))
        .max(1) as u32;
        picture = image::imageops::thumbnail(&picture, THUMBNAIL_WIDTH, height);
    }
    frameever_rendering::input_display::straighten_alpha(&mut picture);
    Some(library::rgba_to_render_image(picture))
}

fn listed_presets() -> (Vec<UserPreset>, Vec<(InputThumbnailKey, Arc<RenderImage>)>) {
    let presets = frameever_rendering::input_presets::presets_dir()
        .map(|dir| frameever_rendering::input_presets::list(&dir))
        .unwrap_or_default();
    let mut pictures = Vec::new();
    for (preset, _) in INPUT_PRESETS {
        for style in InputDisplayStyle::ALL {
            if let Some(picture) = thumbnail(preset.id(), style) {
                pictures.push(((preset.id().to_string(), style), picture));
            }
        }
    }
    for preset in &presets {
        if preset.layout.is_some() {
            for style in InputDisplayStyle::ALL {
                if let Some(picture) = thumbnail(&preset.id, style) {
                    pictures.push(((preset.id.clone(), style), picture));
                }
            }
        } else if let Some(picture) = thumbnail(&preset.id, InputDisplayStyle::default()) {
            for style in InputDisplayStyle::ALL {
                pictures.push(((preset.id.clone(), style), picture.clone()));
            }
        }
    }
    (presets, pictures)
}

impl EditorWindow {
    pub(crate) fn edit_input_display(
        &mut self,
        reason: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut ProjectConfiguration) -> bool,
    ) {
        self.edit_project(reason, window, cx, change);
        self.remember_input_display(window, cx);
    }

    pub(crate) fn remember_input_display(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(setup) = self.project.input_display.clone() else {
            return;
        };
        self.sidebar.input_setup_pending.borrow_mut().queue(setup);
        let pending = self.sidebar.input_setup_pending.clone();
        self.sidebar.input_setup_task = Some(cx.spawn_in(window, async move |_, cx| {
            cx.background_executor().timer(INPUT_SETUP_DEBOUNCE).await;
            let Some(setup) = pending.borrow_mut().flush(InputDisplay::clone) else {
                return;
            };
            let saved = cx
                .background_executor()
                .spawn(async move { crate::store::set_input_display_setup(&setup) })
                .await;
            if !saved {
                tracing::warn!("Could not remember the input display setup");
            }
        }));
    }

    pub(crate) fn flush_input_display_setup(&mut self) {
        self.sidebar.input_setup_task = None;
        let saved = self
            .sidebar
            .input_setup_pending
            .borrow_mut()
            .flush(crate::store::set_input_display_setup);
        if saved == Some(false) {
            tracing::warn!("Could not remember the input display setup");
        }
    }

    pub(crate) fn show_input_display_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.sidebar.tab == crate::editor_sidebar::SidebarTab::InputDisplay {
            return;
        }
        if let Some(index) = crate::editor_sidebar::SidebarTab::ALL
            .iter()
            .position(|tab| *tab == crate::editor_sidebar::SidebarTab::InputDisplay)
        {
            self.select_sidebar_tab(index, window, cx);
            self.canvas_selection = Some(crate::editor_canvas::CanvasSelection::InputDisplay);
        }
    }

    fn input_display_shape(&self, preset: &str) -> Option<PresetShape> {
        let mut cache = self.sidebar.input_shape.borrow_mut();
        if let Some((cached, shape)) = cache.as_ref()
            && cached == preset
        {
            return *shape;
        }
        let shape = frameever_rendering::input_display::preset_shape(preset);
        *cache = Some((preset.to_string(), shape));
        shape
    }

    fn input_display_output(&self) -> Option<(u32, u32)> {
        let layout = self.frame_layout.as_ref()?;
        Some((layout.output_size[0], layout.output_size[1]))
    }

    pub(crate) fn input_display_canvas_rect(&self) -> Option<NormRect> {
        let block = self.project.shown_input_display()?;
        let shape = self.input_display_shape(&block.preset)?;
        Some(input_display_rect(
            block,
            shape,
            self.input_display_output()?,
        ))
    }

    fn write_input_display(
        &mut self,
        change: impl FnOnce(&mut ProjectConfiguration) -> bool,
        cx: &mut Context<Self>,
    ) {
        if change(&mut self.project) {
            self.project_changed_live(cx);
        }
    }

    pub(crate) fn drag_input_display(
        &mut self,
        start: NormRect,
        resize: Option<(i8, i8)>,
        canvas: (f64, f64),
        delta: (f64, f64),
        shift: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(output) = self.input_display_output() else {
            return;
        };
        if let Some(direction) = resize {
            let Some(shape) = self
                .project
                .shown_input_display()
                .map(|block| block.preset.clone())
                .and_then(|preset| self.input_display_shape(&preset))
            else {
                return;
            };
            let (size, position) =
                input_resize(start, shape.frame_share, output, canvas, delta, direction);
            self.snap_guides.clear();
            self.write_input_display(|project| resize_input_display(project, size, position), cx);
            return;
        }
        let (position, guides) = input_drag_position(start, output, canvas, delta, shift);
        self.snap_guides = guides;
        self.write_input_display(|project| place_input_display(project, position), cx);
    }

    pub(crate) fn nudge_input_display(
        &mut self,
        rect: NormRect,
        canvas: (f64, f64),
        direction: (f64, f64),
        shift: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(output) = self.input_display_output() else {
            return false;
        };
        let position = input_nudge_position(rect, output, canvas, direction, shift);
        self.write_input_display(|project| place_input_display(project, position), cx);
        self.schedule_save(window, cx);
        self.remember_input_display(window, cx);
        true
    }

    fn render_input_position(&self, block: &InputDisplay, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.theme;
        let exact = self.sidebar.input_position_exact;
        let grid = div()
            .flex()
            .flex_col()
            .flex_none()
            .gap(px(4.))
            .p(px(6.))
            .rounded(px(6.))
            .border_1()
            .border_color(Hsla::from(theme.border))
            .children(
                INPUT_DISPLAY_SNAP_STOPS
                    .into_iter()
                    .enumerate()
                    .map(|(row, y)| {
                        div().flex().flex_row().gap(px(4.)).children(
                            INPUT_DISPLAY_SNAP_STOPS
                                .into_iter()
                                .enumerate()
                                .map(|(column, x)| {
                                    let chosen = block.position == XY::new(x, y);
                                    div()
                                        .id(SharedString::from(format!(
                                            "input-display-anchor-{column}-{row}"
                                        )))
                                        .w(px(18.))
                                        .h(px(12.))
                                        .rounded(px(2.))
                                        .bg(if chosen {
                                            Hsla::from(theme.foreground)
                                        } else {
                                            Hsla::from(theme.border_strong)
                                        })
                                        .tab_index(0)
                                        .cursor_pointer()
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.edit_input_display(
                                                "input-display-anchor",
                                                window,
                                                cx,
                                                move |project| {
                                                    place_input_display(project, XY::new(x, y))
                                                },
                                            );
                                        }))
                                }),
                        )
                    }),
            );
        let head = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .h(px(32.))
            .text_size(px(13.))
            .text_color(Hsla::from(theme.foreground))
            .child(t!("Position"))
            .child(
                div()
                    .id("input-display-exact")
                    .size(px(22.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(6.))
                    .tab_index(0)
                    .cursor_pointer()
                    .when(exact, |this| this.bg(Hsla::from(theme.border)))
                    .hover(|this| this.bg(Hsla::from(theme.border)))
                    .child(
                        svg()
                            .path(if exact {
                                "icons/chevron-down.svg"
                            } else {
                                "icons/chevron-right.svg"
                            })
                            .size(px(14.))
                            .text_color(Hsla::from(theme.muted_foreground)),
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.sidebar.input_position_exact = !this.sidebar.input_position_exact;
                        cx.notify();
                        window.refresh();
                    })),
            );
        let number = |label: &'static str, key: crate::editor_panels::FieldKey| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.))
                .text_size(px(13.))
                .text_color(Hsla::from(theme.muted_foreground))
                .child(label)
                .child(self.render_number_field(key, "%", 64.))
        };
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(head)
            .child(div().flex().flex_row().child(grid))
            .when(exact, |this| {
                this.child(
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(12.))
                        .child(number("X", crate::editor_panels::FieldKey::InputDisplayX))
                        .child(number("Y", crate::editor_panels::FieldKey::InputDisplayY)),
                )
            })
            .into_any_element()
    }

    pub(crate) fn ensure_input_thumbnails(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let generation = frameever_rendering::input_presets::generation();
        if self.sidebar.input_presets_seen == Some(generation)
            || self.sidebar.input_thumbnail_task.is_some()
        {
            return;
        }
        self.sidebar.input_thumbnail_task = Some(cx.spawn_in(window, async move |this, cx| {
            let (presets, pictures) = cx
                .background_executor()
                .spawn(async move { listed_presets() })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.sidebar.input_user_presets = presets;
                this.sidebar.input_thumbnails = pictures.into_iter().collect();
                this.sidebar.input_presets_seen = Some(generation);
                this.sidebar.input_thumbnail_task = None;
                this.sidebar.input_shape.replace(None);
                cx.notify();
                this.ensure_input_thumbnails(window, cx);
            })
            .ok();
        }));
    }

    pub(crate) fn input_preset_saved(
        &mut self,
        preset: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.sidebar.input_shape.replace(None);
        self.edit_input_display("input-display-preset", window, cx, move |project| {
            choose_input_preset(project, &preset)
        });
        self.ensure_input_thumbnails(window, cx);
        self.publish_project();
        cx.notify();
    }

    fn open_input_preset_maker(
        &mut self,
        preset: Option<UserPreset>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(editor) = window.window_handle().downcast::<Self>() else {
            return;
        };
        let seed = match preset {
            Some(preset) => crate::input_preset_maker::MakerSeed {
                editing: Some(preset.id),
                name: preset.name,
                layout: preset.layout,
            },
            None => crate::input_preset_maker::MakerSeed {
                editing: None,
                name: t!("My preset").to_string(),
                layout: None,
            },
        };
        cx.defer(move |cx| crate::app_windows::open_input_preset_maker(editor, seed, cx));
    }

    fn import_input_preset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: None,
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(folder) = paths.into_iter().next() else {
                return;
            };
            let result = cx
                .background_executor()
                .spawn(async move {
                    let dir =
                        frameever_rendering::input_presets::presets_dir().ok_or_else(|| {
                            ImportRefusal::NotSaved {
                                detail: "the app data folder is not known".to_string(),
                            }
                        })?;
                    frameever_rendering::input_presets::import_folder(&dir, &folder)
                })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.input_preset_imported(result, window, cx);
            })
            .ok();
        })
        .detach();
    }

    fn input_preset_imported(
        &mut self,
        result: Result<Vec<String>, ImportRefusal>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(imported) => {
                self.sidebar.input_import_error = None;
                if let Some(preset) = imported.into_iter().next() {
                    self.edit_input_display("input-display-preset", window, cx, move |project| {
                        choose_input_preset(project, &preset)
                    });
                }
            }
            Err(refusal) => {
                tracing::warn!(%refusal, "input preset import refused");
                self.sidebar.input_import_error = Some(InputImportError {
                    message: import_refusal_message(&refusal).into(),
                    details: refusal.to_string().into(),
                    open: false,
                });
            }
        }
        self.ensure_input_thumbnails(window, cx);
        cx.notify();
    }

    fn remove_input_preset(&mut self, preset: String, window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_in(window, async move |this, cx| {
            if !crate::platform::confirm_dialog(
                t!("Remove this preset?"),
                t!(
                    "Projects that use it show no input display until you choose another preset. This can't be undone."
                ),
                t!("Remove preset"),
                t!("Cancel"),
                true,
            ) {
                return;
            }
            let removed = cx
                .background_executor()
                .spawn({
                    let preset = preset.clone();
                    async move {
                        frameever_rendering::input_presets::presets_dir().is_some_and(|dir| {
                            frameever_rendering::input_presets::remove(&dir, &preset).is_ok()
                        })
                    }
                })
                .await;
            this.update_in(cx, |this, window, cx| {
                if removed {
                    this.edit_input_display(
                        "input-display-preset-removed",
                        window,
                        cx,
                        move |project| after_preset_removed(project, &preset),
                    );
                }
                this.ensure_input_thumbnails(window, cx);
                this.publish_project();
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn render_input_import_error(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let theme = self.theme;
        let error = self.sidebar.input_import_error.as_ref()?;
        Some(
            div()
                .flex()
                .flex_col()
                .items_start()
                .gap(px(6.))
                .text_size(px(12.))
                .text_color(Hsla::from(theme.foreground))
                .child(error.message.clone())
                .child(
                    ui::Button::plain(
                        &theme,
                        "input-display-import-details",
                        ui::ButtonVariant::Gray,
                        ui::ButtonSize::Sm,
                    )
                    .label(t!("Details"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(error) = this.sidebar.input_import_error.as_mut() {
                            error.open = !error.open;
                        }
                        cx.notify();
                    })),
                )
                .when(error.open, |this| {
                    this.child(
                        div()
                            .w_full()
                            .text_color(Hsla::from(theme.muted_foreground))
                            .child(error.details.clone()),
                    )
                })
                .into_any_element(),
        )
    }

    fn render_input_tile_action(
        &self,
        id: String,
        icon: &'static str,
        cx: &mut Context<Self>,
        act: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> impl IntoElement {
        let theme = self.theme;
        div()
            .id(SharedString::from(id))
            .size(px(20.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(4.))
            .tab_index(0)
            .cursor_pointer()
            .hover(move |this| this.bg(Hsla::from(theme.overlay)))
            .child(
                svg()
                    .path(icon)
                    .size(px(TILE_ACTION_ICON))
                    .text_color(rgb(TILE_INK)),
            )
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|_, _, _, cx| cx.stop_propagation()),
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                cx.stop_propagation();
                act(this, window, cx);
            }))
    }

    pub(crate) fn render_input_display_tab(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.theme;
        let shown = self.project.shown_input_display().cloned();
        let on = shown.is_some();
        let section = ui::Field::section(&theme, t!("Input display")).value(
            ui::Toggle::plain(&theme, "input-display-enabled", on)
                .on_click(cx.listener(move |this, _, window, cx| {
                    let project_path = this.project_path.clone();
                    this.edit_input_display("input-display-enabled", window, cx, move |project| {
                        switch_input_display_for_recording(project, !on, &project_path)
                    });
                    this.ensure_input_thumbnails(window, cx);
                }))
                .into_any_element(),
        );
        let Some(block) = shown else {
            return section.into_any_element();
        };

        let cell = ((crate::editor_sidebar::CONTENT_WIDTH - TILE_GAP * (TILE_COLUMNS - 1.))
            / TILE_COLUMNS)
            .floor()
            - 1.;
        let tile = |id: String| {
            div()
                .id(SharedString::from(id))
                .relative()
                .w(px(cell))
                .h(px(TILE_HEIGHT))
                .flex()
                .flex_col()
                .items_center()
                .gap(px(6.))
                .pt(px(8.))
                .px(px(6.))
                .pb(px(6.))
                .rounded(px(8.))
                .border_1()
                .border_color(Hsla::from(theme.border))
                .text_size(px(12.))
                .font_weight(FontWeight::NORMAL)
                .tab_index(0)
                .cursor_pointer()
        };
        let listed = INPUT_PRESETS
            .iter()
            .map(|(preset, label)| (preset.id().to_string(), SharedString::from(tr(label)), None))
            .chain(self.sidebar.input_user_presets.iter().map(|preset| {
                (
                    preset.id.clone(),
                    SharedString::from(preset.name.clone()),
                    Some(preset),
                )
            }));
        let mut tiles = div().flex().flex_row().flex_wrap().gap(px(TILE_GAP));
        for (preset, label, user) in listed {
            let selected = block.preset == preset;
            let picture = self
                .sidebar
                .input_thumbnails
                .get(&(preset.clone(), block.style))
                .cloned();
            let mut actions = div()
                .absolute()
                .top(px(2.))
                .right(px(2.))
                .flex()
                .flex_row()
                .gap(px(2.));
            if let Some(user) = user {
                if user.layout.is_some() {
                    let edited = user.clone();
                    actions = actions.child(self.render_input_tile_action(
                        format!("input-preset-edit-{preset}"),
                        "icons/pencil.svg",
                        cx,
                        move |this, window, cx| {
                            this.open_input_preset_maker(Some(edited.clone()), window, cx);
                        },
                    ));
                }
                let removed = preset.clone();
                actions = actions.child(self.render_input_tile_action(
                    format!("input-preset-remove-{preset}"),
                    "icons/trash.svg",
                    cx,
                    move |this, window, cx| {
                        this.remove_input_preset(removed.clone(), window, cx);
                    },
                ));
            }
            let chosen = preset.clone();
            tiles = tiles.child(
                tile(format!("input-preset-{preset}"))
                    .justify_between()
                    .when(selected, |this| {
                        this.border_color(Hsla::from(theme.foreground))
                    })
                    .bg(rgb(PICTURE_BACKDROP))
                    .text_color(rgb(TILE_INK))
                    .child(
                        div()
                            .w_full()
                            .h(px(TILE_PICTURE_HEIGHT))
                            .flex()
                            .items_center()
                            .justify_center()
                            .children(picture.map(|picture| {
                                img(picture)
                                    .size_full()
                                    .object_fit(gpui::ObjectFit::Contain)
                            })),
                    )
                    .child(div().max_w_full().truncate().child(label))
                    .child(actions)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        let chosen = chosen.clone();
                        this.edit_input_display(
                            "input-display-preset",
                            window,
                            cx,
                            move |project| choose_input_preset(project, &chosen),
                        );
                    })),
            );
        }
        let tiles = tiles
            .child(
                tile("input-preset-new".to_string())
                    .justify_center()
                    .text_color(Hsla::from(theme.muted_foreground))
                    .child(t!("New…"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_input_preset_maker(None, window, cx);
                    })),
            )
            .child(
                tile("input-preset-import".to_string())
                    .justify_center()
                    .text_color(Hsla::from(theme.muted_foreground))
                    .child(t!("Import…"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.import_input_preset(window, cx);
                    })),
            );

        let styles = ui::SegmentedControl::editor(
            &theme,
            "input-display-style",
            InputDisplayStyle::ALL
                .into_iter()
                .map(|style| ui::SegmentOption::new(style_label(style), block.style == style))
                .collect(),
        )
        .stretch()
        .on_select(cx.listener(move |this, index: &usize, window, cx| {
            let Some(style) = InputDisplayStyle::ALL.get(*index).copied() else {
                return;
            };
            this.edit_input_display("input-display-style", window, cx, move |project| {
                choose_input_style(project, style)
            });
        }));

        section
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(14.))
                    .when(self.sidebar.input_thumbnails.is_empty(), |this| {
                        this.min_h(px(TILE_HEIGHT))
                    })
                    .child(tiles)
                    .children(self.render_input_import_error(cx))
                    .child(styles)
                    .child(self.render_input_position(&block, cx))
                    .child(self.slider_field(
                        t!("Size"),
                        crate::editor_sidebar::SliderKey::InputDisplay(InputDisplaySlider::Size),
                        "%",
                        cx,
                    ))
                    .child(self.slider_field(
                        t!("Opacity"),
                        crate::editor_sidebar::SliderKey::InputDisplay(InputDisplaySlider::Opacity),
                        "%",
                        cx,
                    )),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use frameever_project::input_display::{
        GamepadSnapshot, INPUT_EVENTS_FILE_NAME, InputEvent, InputEventKind, InputEvents,
        gamepad_buttons,
    };

    #[test]
    fn switching_on_starts_from_the_default_setup() {
        let mut project = ProjectConfiguration::default();

        assert!(switch_input_display(&mut project, true));

        assert_eq!(
            project.shown_input_display(),
            Some(&InputDisplay {
                enabled: true,
                ..Default::default()
            })
        );
        assert!(!switch_input_display(&mut project, true));
    }

    #[test]
    fn switching_off_keeps_the_setup_for_the_next_time() {
        let mut project = ProjectConfiguration::default();
        switch_input_display(&mut project, true);
        choose_input_preset(&mut project, BuiltinPresetId::FullKeyboard.id());
        choose_input_style(&mut project, InputDisplayStyle::Ghost);

        assert!(switch_input_display(&mut project, false));
        assert!(project.shown_input_display().is_none());
        assert!(switch_input_display(&mut project, true));

        let block = project.shown_input_display().unwrap();
        assert_eq!(block.preset, "full-keyboard");
        assert_eq!(block.style, InputDisplayStyle::Ghost);
    }

    #[test]
    fn a_project_that_never_had_it_stays_untouched_when_switched_off() {
        let mut project = ProjectConfiguration::default();

        assert!(!switch_input_display(&mut project, false));
        assert!(project.input_display.is_none());
    }

    #[test]
    fn choosing_what_is_already_chosen_changes_nothing() {
        let mut project = ProjectConfiguration::default();
        switch_input_display(&mut project, true);

        assert!(choose_input_preset(
            &mut project,
            BuiltinPresetId::WasdMouse.id()
        ));
        assert!(!choose_input_preset(
            &mut project,
            BuiltinPresetId::WasdMouse.id()
        ));
        assert!(!choose_input_style(
            &mut project,
            InputDisplayStyle::Keycaps
        ));
        assert!(choose_input_preset(
            &mut project,
            BuiltinPresetId::FullKeyboard.id()
        ));
        assert!(choose_input_style(&mut project, InputDisplayStyle::Outline));
    }

    const OUTPUT: (u32, u32) = (2000, 1000);
    const CANVAS: (f64, f64) = (1000., 500.);
    const SHAPE: PresetShape = PresetShape {
        size: (400, 200),
        frame_share: 0.3,
    };

    fn placed(x: f64, y: f64) -> InputDisplay {
        InputDisplay {
            position: XY::new(x, y),
            ..Default::default()
        }
    }

    fn rect_at(x: f64, y: f64) -> NormRect {
        input_display_rect(&placed(x, y), SHAPE, OUTPUT)
    }

    #[test]
    fn the_canvas_box_is_where_the_renderer_draws() {
        assert_eq!(
            rect_at(0., 1.),
            NormRect {
                x: 0.03,
                y: 0.64,
                w: 0.3,
                h: 0.3
            }
        );
    }

    #[test]
    fn a_drag_follows_the_pointer_between_the_stops() {
        let (position, guides) =
            input_drag_position(rect_at(0., 1.), OUTPUT, CANVAS, (128., -116.), false);

        assert!((position.x - 0.2).abs() < 1e-9);
        assert!((position.y - 0.6).abs() < 1e-9);
        assert!(guides.is_empty());
        let moved = input_display_rect(&placed(position.x, position.y), SHAPE, OUTPUT);
        assert!((moved.x - (0.03 + 0.128)).abs() < 1e-9);
        assert!((moved.y - (0.64 - 0.232)).abs() < 1e-9);
    }

    #[test]
    fn a_drag_snaps_to_the_centre_lines_and_reports_them() {
        let (position, guides) =
            input_drag_position(rect_at(0., 1.), OUTPUT, CANVAS, (310., -140.), false);

        assert_eq!(position, XY::new(0.5, 0.5));
        assert_eq!(
            guides,
            vec![
                SnapGuide {
                    axis: GuideAxis::V,
                    pos: 0.5,
                    start: 0.,
                    end: 1.,
                    kind: SnapLineKind::FrameCenter
                },
                SnapGuide {
                    axis: GuideAxis::H,
                    pos: 0.5,
                    start: 0.,
                    end: 1.,
                    kind: SnapLineKind::FrameCenter
                },
            ]
        );
    }

    #[test]
    fn a_drag_snaps_to_the_margin_and_never_leaves_it() {
        let (position, guides) =
            input_drag_position(rect_at(0.5, 0.5), OUTPUT, CANVAS, (312., -900.), false);

        assert_eq!(position, XY::new(1., 0.));
        assert_eq!(
            guides
                .iter()
                .map(|guide| (guide.axis, guide.kind))
                .collect::<Vec<_>>(),
            vec![
                (GuideAxis::V, SnapLineKind::Margin),
                (GuideAxis::H, SnapLineKind::Margin)
            ]
        );
        assert!((guides[0].pos - 0.97).abs() < 1e-9);
        assert!((guides[1].pos - 0.06).abs() < 1e-9);
    }

    #[test]
    fn holding_shift_drags_without_snapping() {
        let (position, guides) =
            input_drag_position(rect_at(0., 1.), OUTPUT, CANVAS, (310., -140.), true);

        assert!(guides.is_empty());
        assert!((position.x - 310. / 640.).abs() < 1e-9);
        assert!((position.y - (1. - 140. / 290.)).abs() < 1e-9);
    }

    #[test]
    fn a_nudge_moves_one_canvas_pixel_and_ten_with_shift() {
        let start = rect_at(0.5, 0.5);
        let one = input_nudge_position(start, OUTPUT, CANVAS, (1., 0.), false);
        let ten = input_nudge_position(start, OUTPUT, CANVAS, (0., -1.), true);

        let after_one = input_display_rect(&placed(one.x, one.y), SHAPE, OUTPUT);
        let after_ten = input_display_rect(&placed(ten.x, ten.y), SHAPE, OUTPUT);
        assert!((after_one.x - start.x - 0.001).abs() < 1e-9);
        assert!((after_one.y - start.y).abs() < 1e-9);
        assert!((after_ten.y - start.y + 0.02).abs() < 1e-9);
        assert!((after_ten.x - start.x).abs() < 1e-9);
        assert_eq!(
            input_nudge_position(rect_at(0., 1.), OUTPUT, CANVAS, (-1., 1.), true),
            XY::new(0., 1.)
        );
    }

    #[test]
    fn dragging_a_corner_resizes_and_keeps_the_opposite_corner() {
        let start = rect_at(0.5, 0.5);
        let (size, position) = input_resize(
            start,
            SHAPE.frame_share,
            OUTPUT,
            CANVAS,
            (-150., 0.),
            (-1, -1),
        );
        let block = InputDisplay {
            size,
            position,
            ..Default::default()
        };
        let grown = input_display_rect(&block, SHAPE, OUTPUT);

        assert_eq!(size, 90.);
        assert!((grown.w - 0.45).abs() < 1e-9);
        assert!((grown.h - 0.45).abs() < 1e-9);
        assert!((grown.x + grown.w - start.x - start.w).abs() < 1e-9);
        assert!((grown.y + grown.h - start.y - start.h).abs() < 1e-9);
    }

    #[test]
    fn a_resize_stays_inside_the_size_range() {
        let start = rect_at(0., 0.);
        let (largest, _) = input_resize(
            start,
            SHAPE.frame_share,
            OUTPUT,
            CANVAS,
            (4000., 0.),
            (1, 1),
        );
        let (smallest, position) = input_resize(
            start,
            SHAPE.frame_share,
            OUTPUT,
            CANVAS,
            (-4000., -4000.),
            (1, 1),
        );

        assert_eq!((largest, smallest), (120., 30.));
        assert_eq!(position, XY::new(0., 0.));
    }

    #[test]
    fn the_panel_writes_only_values_inside_their_ranges() {
        let mut project = ProjectConfiguration::default();
        switch_input_display(&mut project, true);

        assert!(place_input_display(&mut project, XY::new(0.5, 7.)));
        assert!(!place_input_display(&mut project, XY::new(0.5, 1.)));
        assert!(set_input_display_size(&mut project, 400.));
        assert!(!set_input_display_size(&mut project, 120.2));
        assert!(set_input_display_opacity(&mut project, 3.));
        assert!(resize_input_display(&mut project, 90., XY::new(0., 0.)));

        let block = project.shown_input_display().unwrap();
        assert_eq!(block.position, XY::new(0., 0.));
        assert_eq!((block.size, block.opacity), (90., 20.));
    }

    fn recorded_project(events: Vec<InputEvent>) -> tempfile::TempDir {
        let directory = tempfile::tempdir().unwrap();
        let take = directory.path().join("content/segments/segment-0");
        std::fs::create_dir_all(&take).unwrap();
        InputEvents { events }
            .write_to_file(&take.join(INPUT_EVENTS_FILE_NAME))
            .unwrap();
        std::fs::write(
            directory.path().join("recording-meta.json"),
            serde_json::to_vec(&serde_json::json!({
                "pretty_name": "played",
                "segments": [{"display": {"path": "content/segments/segment-0/display.mp4"}}]
            }))
            .unwrap(),
        )
        .unwrap();
        directory
    }

    #[test]
    fn switching_on_for_a_recording_that_used_a_gamepad_picks_the_gamepad_preset() {
        let played = recorded_project(vec![InputEvent {
            time_ms: 40.0,
            kind: InputEventKind::Gamepad(
                GamepadSnapshot::default().with_button(gamepad_buttons::SOUTH),
            ),
        }]);
        let typed = recorded_project(vec![InputEvent {
            time_ms: 40.0,
            kind: InputEventKind::MouseButton {
                button: 1,
                down: true,
            },
        }]);
        let preset = |project: &ProjectConfiguration| {
            project
                .shown_input_display()
                .map(|block| block.preset.clone())
        };

        let mut project = ProjectConfiguration::default();
        assert!(switch_input_display_for_recording(
            &mut project,
            true,
            played.path()
        ));
        assert_eq!(preset(&project).as_deref(), Some("gamepad"));

        assert!(choose_input_preset(&mut project, "full-keyboard"));
        assert!(!switch_input_display_for_recording(
            &mut project,
            true,
            played.path()
        ));
        assert_eq!(preset(&project).as_deref(), Some("full-keyboard"));

        assert!(switch_input_display_for_recording(
            &mut project,
            false,
            played.path()
        ));
        assert_eq!(
            project
                .input_display
                .as_ref()
                .map(|block| block.preset.as_str()),
            Some("full-keyboard")
        );
        assert!(switch_input_display_for_recording(
            &mut project,
            true,
            played.path()
        ));
        assert_eq!(preset(&project).as_deref(), Some("full-keyboard"));

        let mut remembered = ProjectConfiguration {
            input_display: Some(InputDisplay {
                preset: "gamepad".to_string(),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(switch_input_display_for_recording(
            &mut remembered,
            true,
            typed.path()
        ));
        assert_eq!(preset(&remembered).as_deref(), Some("wasd-mouse"));

        let mut unsaved = ProjectConfiguration::default();
        assert!(switch_input_display_for_recording(
            &mut unsaved,
            true,
            std::path::Path::new("no-such-project")
        ));
        assert_eq!(preset(&unsaved).as_deref(), Some("wasd-mouse"));
    }

    #[test]
    fn a_setup_changed_just_before_closing_is_still_remembered() {
        let setup = |size: f64| InputDisplay {
            size,
            ..Default::default()
        };
        let mut pending = PendingInputSetup::default();
        let mut saved = Vec::new();

        assert!(pending.flush(|setup| saved.push(setup.size)).is_none());
        pending.queue(setup(40.));
        pending.queue(setup(80.));
        assert!(pending.flush(|setup| saved.push(setup.size)).is_some());
        assert!(pending.flush(|setup| saved.push(setup.size)).is_none());

        assert_eq!(saved, vec![80.]);
    }

    #[test]
    fn removing_the_preset_a_project_uses_falls_back_to_a_built_in_one() {
        let mut project = ProjectConfiguration::default();
        assert!(!after_preset_removed(&mut project, "user:p-1"));
        assert!(project.input_display.is_none());

        switch_input_display(&mut project, true);
        choose_input_preset(&mut project, "user:p-1");
        assert!(!after_preset_removed(&mut project, "user:p-2"));
        assert!(after_preset_removed(&mut project, "user:p-1"));

        let block = project.shown_input_display().unwrap();
        assert_eq!(block.preset, BuiltinPresetId::WasdMouse.id());
    }

    #[test]
    fn a_refused_import_says_what_happened_and_what_to_do_next() {
        use frameever_project::input_display::PresetRefusal;

        let file = |refusal| ImportRefusal::File {
            file: "wasd.json".to_string(),
            refusal,
        };
        let messages = [
            import_refusal_message(&ImportRefusal::NoPresetFile),
            import_refusal_message(&ImportRefusal::FolderUnreadable {
                detail: "os error 5".to_string(),
            }),
            import_refusal_message(&ImportRefusal::TooManyPresetFiles { count: 99 }),
            import_refusal_message(&ImportRefusal::NotSaved {
                detail: "os error 112".to_string(),
            }),
            import_refusal_message(&file(PresetRefusal::MissingAtlas)),
            import_refusal_message(&file(PresetRefusal::AtlasTooLarge {
                width: 9000,
                height: 1,
            })),
            import_refusal_message(&file(PresetRefusal::JsonUnreadable {
                detail: "EOF while parsing".to_string(),
            })),
        ];

        for message in &messages {
            assert!(
                message.starts_with("Couldn't import this preset. "),
                "{message}"
            );
            assert!(message.matches(". ").count() >= 2, "{message}");
            assert!(
                !message.contains("os error") && !message.contains("EOF"),
                "{message}"
            );
        }
        assert!(messages[4].contains("wasd.json has no image beside it"));
        assert!(messages[5].contains("wasd.json is larger than FrameEver allows"));
        assert!(messages[6].contains("wasd.json isn't a preset FrameEver can read"));
        let unique: std::collections::BTreeSet<&String> = messages.iter().collect();
        assert_eq!(unique.len(), messages.len());
    }

    #[test]
    fn every_listed_preset_has_a_thumbnail_in_every_style() {
        for (preset, _) in INPUT_PRESETS {
            for style in InputDisplayStyle::ALL {
                assert!(
                    thumbnail(preset.id(), style).is_some(),
                    "{preset:?} {style:?}"
                );
            }
        }
        assert_eq!(
            INPUT_PRESETS.map(|(preset, _)| preset),
            BuiltinPresetId::ALL
        );
    }
}
