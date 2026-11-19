use frameever_project::input_display::{
    KeyLayout, MAKER_COLUMNS, MAKER_ROWS, MAKER_WIDTHS, MOUSE_BLOCK, MakerSelection, MakerStart,
    MousePart, PresetMaker,
    key_codes::{ALT_LEFT, CONTROL_LEFT, META_LEFT, SHIFT_LEFT},
    keystroke_code,
};
use gpui::{
    AppContext as _, Context, Entity, FocusHandle, FontWeight, Hsla, InteractiveElement,
    IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, ParentElement, Pixels, Point, Render,
    SharedString, Styled, Subscription, Window, div, prelude::FluentBuilder, px, rgb,
};

use crate::{editor_input_display::PICTURE_BACKDROP, i18n::t, theme::Theme, ui};

pub const MAKER_WINDOW_WIDTH: f32 = 784.;
pub const MAKER_WINDOW_HEIGHT: f32 = 460.;
pub const MAKER_WINDOW_TITLE: &str = concat!(crate::brand::app_name!(), " Preset Maker");

const UNIT: f32 = 40.;
const KEY_GAP: f32 = 4.;
const KEY_REST: u32 = 0x181818;
const KEY_REST_INK: u32 = 0xc4c4c4;
const KEY_HELD: u32 = 0xffffff;
const KEY_HELD_INK: u32 = 0x111111;
const MOUSE_BODY: u32 = 0x2b2b2b;

fn start_label(start: MakerStart) -> &'static str {
    match start {
        MakerStart::WasdMouse => t!("WASD + mouse"),
        MakerStart::FullKeyboard => t!("Full keyboard"),
        MakerStart::Empty => t!("Empty"),
    }
}

fn mouse_part_label(part: MousePart) -> &'static str {
    match part {
        MousePart::Left => t!("Left button"),
        MousePart::Right => t!("Right button"),
        MousePart::Wheel => t!("Wheel"),
        MousePart::Side => t!("Side buttons"),
    }
}

fn mouse_part_shapes(part: MousePart) -> &'static [(f32, f32, f32, f32)] {
    match part {
        MousePart::Left => &[(8., 4., 41., 62.)],
        MousePart::Right => &[(55., 4., 41., 62.)],
        MousePart::Wheel => &[(45., 18., 14., 32.)],
        MousePart::Side => &[(0., 74., 7., 18.), (0., 96., 7., 18.)],
    }
}

pub struct MakerSeed {
    pub editing: Option<String>,
    pub name: String,
    pub layout: Option<KeyLayout>,
}

impl MakerSeed {
    pub fn model(&self) -> PresetMaker {
        match &self.layout {
            Some(layout) => PresetMaker::editing(self.name.clone(), layout),
            None => PresetMaker::starting(MakerStart::WasdMouse, self.name.clone()),
        }
    }
}

struct BoardDrag {
    item: MakerSelection,
    down: Point<Pixels>,
    x: f64,
    y: f64,
}

pub type PresetSaved = std::rc::Rc<dyn Fn(String, &mut gpui::App)>;

pub struct InputPresetMaker {
    theme: Theme,
    focus: FocusHandle,
    model: PresetMaker,
    editing: Option<String>,
    on_saved: PresetSaved,
    name: Entity<ui::TextInputState>,
    label: Entity<ui::TextInputState>,
    drag: Option<BoardDrag>,
    modifiers: gpui::Modifiers,
    save_failed: bool,
    _subscriptions: Vec<Subscription>,
}

impl InputPresetMaker {
    pub fn new(
        seed: MakerSeed,
        on_saved: PresetSaved,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        crate::theme::bind_window(window, cx);
        let model = seed.model();
        let name = cx.new(|cx| {
            let mut input = ui::TextInputState::single_line(window, cx);
            input.set_placeholder(t!("Preset name"));
            input.set_text(model.name.clone(), cx);
            input
        });
        let label = cx.new(|cx| ui::TextInputState::single_line(window, cx));
        let subscriptions = vec![
            cx.subscribe_in(
                &name,
                window,
                |this: &mut Self, input, event: &ui::TextInputEvent, window, cx| {
                    this.model.set_name(input.read(cx).text());
                    this.leave_field(event, window, cx);
                },
            ),
            cx.subscribe_in(
                &label,
                window,
                |this: &mut Self, input, event: &ui::TextInputEvent, window, cx| {
                    let text = input.read(cx).text().to_string();
                    this.model.set_label(&text);
                    this.leave_field(event, window, cx);
                },
            ),
            cx.observe_window_activation(window, |this, window, cx| {
                if !window.is_window_active() {
                    this.model.release_all();
                    this.modifiers = gpui::Modifiers::default();
                    this.drag = None;
                    cx.notify();
                }
            }),
        ];
        Self {
            theme: Theme::for_window(window, cx, false),
            focus: cx.focus_handle(),
            model,
            editing: seed.editing,
            on_saved,
            name,
            label,
            drag: None,
            modifiers: gpui::Modifiers::default(),
            save_failed: false,
            _subscriptions: subscriptions,
        }
    }

    pub fn focus_root(&self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
    }

    fn leave_field(
        &mut self,
        event: &ui::TextInputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(
            event,
            ui::TextInputEvent::Confirmed | ui::TextInputEvent::Cancelled
        ) {
            self.focus_root(window, cx);
        }
        cx.notify();
    }

    fn select(&mut self, selection: Option<MakerSelection>, cx: &mut Context<Self>) {
        self.model.select(selection);
        let label = self
            .model
            .selected_key()
            .map(|key| key.label.clone())
            .unwrap_or_default();
        self.label.update(cx, |input, cx| input.set_text(label, cx));
        cx.notify();
    }

    fn press(&mut self, code: u16, cx: &mut Context<Self>) {
        if self.model.key_down(code) {
            self.select(self.model.selected, cx);
        }
        cx.notify();
    }

    fn on_key_down(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if ui::text_input_has_focus(window, cx) {
            return;
        }
        let Some(code) = keystroke_code(&event.keystroke.key) else {
            return;
        };
        self.press(code, cx);
        if self.model.capture {
            cx.stop_propagation();
        }
    }

    fn on_key_up(&mut self, event: &gpui::KeyUpEvent, cx: &mut Context<Self>) {
        if let Some(code) = keystroke_code(&event.keystroke.key) {
            self.model.key_up(code);
            cx.notify();
        }
    }

    fn on_modifiers(
        &mut self,
        modifiers: gpui::Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let before = std::mem::replace(&mut self.modifiers, modifiers);
        let typing = ui::text_input_has_focus(window, cx);
        for (was, is, code) in [
            (before.shift, modifiers.shift, SHIFT_LEFT),
            (before.control, modifiers.control, CONTROL_LEFT),
            (before.alt, modifiers.alt, ALT_LEFT),
            (before.platform, modifiers.platform, META_LEFT),
        ] {
            if is && !was && !typing {
                self.press(code, cx);
            } else if was && !is {
                self.model.key_up(code);
            }
        }
        cx.notify();
    }

    fn begin_drag(&mut self, item: MakerSelection, event: &MouseDownEvent, cx: &mut Context<Self>) {
        let place = match item {
            MakerSelection::Key(index) => self.model.keys.get(index).map(|key| (key.x, key.y)),
            MakerSelection::Mouse => self.model.mouse.map(|mouse| (mouse.x, mouse.y)),
        };
        let Some((x, y)) = place else {
            return;
        };
        self.drag = Some(BoardDrag {
            item,
            down: event.position,
            x,
            y,
        });
        self.select(Some(item), cx);
    }

    fn drag_to(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some(drag) = &self.drag else {
            return;
        };
        if !event.dragging() {
            self.drag = None;
            return;
        }
        let x = drag.x + f64::from(f32::from(event.position.x - drag.down.x) / UNIT);
        let y = drag.y + f64::from(f32::from(event.position.y - drag.down.y) / UNIT);
        let moved = match drag.item {
            MakerSelection::Key(index) => self.model.move_key(index, x, y),
            MakerSelection::Mouse => self.model.move_mouse(x, y),
        };
        if moved {
            cx.notify();
        }
    }

    fn start_from(&mut self, start: MakerStart, cx: &mut Context<Self>) {
        self.model = PresetMaker::starting(start, self.model.name.clone());
        self.drag = None;
        self.select(None, cx);
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.model.can_save() {
            return;
        }
        let Some(dir) = frameever_rendering::input_presets::presets_dir() else {
            self.save_failed = true;
            cx.notify();
            return;
        };
        match frameever_rendering::input_presets::save_layout(
            &dir,
            self.editing.as_deref(),
            &self.model.name,
            &self.model.layout(),
        ) {
            Ok(id) => {
                let on_saved = self.on_saved.clone();
                cx.defer(move |cx| on_saved(id, cx));
                window.remove_window();
            }
            Err(error) => {
                tracing::warn!(%error, "could not save the input preset");
                self.save_failed = true;
                cx.notify();
            }
        }
    }

    fn flag(
        &self,
        id: &'static str,
        name: &'static str,
        on: bool,
        cx: &mut Context<Self>,
        flip: impl Fn(&mut PresetMaker) + 'static,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .child(name)
            .child(ui::Toggle::plain(&self.theme, id, on).on_click(cx.listener(
                move |this, _, window, cx| {
                    flip(&mut this.model);
                    this.focus_root(window, cx);
                    cx.notify();
                },
            )))
    }

    fn render_board(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let mut board = div()
            .id("preset-maker-board")
            .relative()
            .flex_none()
            .w(px(UNIT * MAKER_COLUMNS as f32))
            .h(px(UNIT * MAKER_ROWS as f32))
            .rounded(px(8.))
            .bg(rgb(PICTURE_BACKDROP))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.focus_root(window, cx);
                    this.select(None, cx);
                }),
            );
        for (index, key) in self.model.keys.iter().enumerate() {
            let held = self.model.is_held(key.code);
            let selected = self.model.selected == Some(MakerSelection::Key(index));
            board = board.child(
                div()
                    .id(SharedString::from(format!("preset-maker-key-{index}")))
                    .absolute()
                    .left(px(key.x as f32 * UNIT))
                    .top(px(key.y as f32 * UNIT))
                    .w(px(key.width as f32 * UNIT - KEY_GAP))
                    .h(px(UNIT - KEY_GAP))
                    .flex()
                    .items_center()
                    .justify_center()
                    .overflow_hidden()
                    .rounded(px(6.))
                    .border_2()
                    .border_color(if selected {
                        Hsla::from(theme.primary)
                    } else {
                        gpui::transparent_black()
                    })
                    .bg(rgb(if held { KEY_HELD } else { KEY_REST }))
                    .text_color(rgb(if held { KEY_HELD_INK } else { KEY_REST_INK }))
                    .text_size(px(12.))
                    .font_weight(FontWeight::MEDIUM)
                    .cursor_pointer()
                    .child(SharedString::from(key.label.clone()))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            this.focus_root(window, cx);
                            this.begin_drag(MakerSelection::Key(index), event, cx);
                        }),
                    ),
            );
        }
        if let Some(mouse) = self.model.mouse {
            let selected = self.model.selected == Some(MakerSelection::Mouse);
            let mut block = div()
                .id("preset-maker-mouse-block")
                .absolute()
                .left(px(mouse.x as f32 * UNIT))
                .top(px(mouse.y as f32 * UNIT))
                .w(px(MOUSE_BLOCK.0 as f32 * UNIT))
                .h(px(MOUSE_BLOCK.1 as f32 * UNIT))
                .cursor_pointer()
                .child(
                    div()
                        .absolute()
                        .left(px(4.))
                        .top_0()
                        .w(px(96.))
                        .h(px(150.))
                        .rounded(px(44.))
                        .border_2()
                        .border_color(if selected {
                            Hsla::from(theme.primary)
                        } else {
                            gpui::transparent_black()
                        })
                        .bg(rgb(MOUSE_BODY)),
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, event: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        this.focus_root(window, cx);
                        this.begin_drag(MakerSelection::Mouse, event, cx);
                    }),
                );
            for part in MousePart::ALL
                .into_iter()
                .filter(|part| mouse.parts.shows(*part))
            {
                for (left, top, width, height) in mouse_part_shapes(part) {
                    block = block.child(
                        div()
                            .absolute()
                            .left(px(*left))
                            .top(px(*top))
                            .w(px(*width))
                            .h(px(*height))
                            .rounded(px(4.))
                            .bg(rgb(KEY_REST)),
                    );
                }
            }
            board = board.child(block);
        }
        board
    }

    fn render_selected_mouse(&self, row: gpui::Div, cx: &mut Context<Self>) -> gpui::Div {
        let Some(mouse) = self.model.mouse else {
            return row;
        };
        let theme = self.theme;
        MousePart::ALL
            .into_iter()
            .fold(row, |row, part| {
                let shown = mouse.parts.shows(part);
                row.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.))
                        .child(mouse_part_label(part))
                        .child(
                            ui::Toggle::plain(
                                &theme,
                                SharedString::from(format!("preset-maker-mouse-{part:?}")),
                                shown,
                            )
                            .on_click(cx.listener(
                                move |this, _, window, cx| {
                                    this.model.set_mouse_part(part, !shown);
                                    this.focus_root(window, cx);
                                    cx.notify();
                                },
                            )),
                        ),
                )
            })
            .child(div().flex_1())
            .child(
                ui::Button::plain(
                    &theme,
                    "preset-maker-remove-mouse",
                    ui::ButtonVariant::Gray,
                    ui::ButtonSize::Sm,
                )
                .label(t!("Remove"))
                .on_click(cx.listener(|this, _, window, cx| {
                    this.model.delete_selected();
                    this.focus_root(window, cx);
                    this.select(None, cx);
                })),
            )
    }

    fn render_selected_key(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(16.))
            .min_h(px(34.));
        if self.model.selected == Some(MakerSelection::Mouse) {
            return self.render_selected_mouse(row, cx);
        }
        let Some(key) = self.model.selected_key() else {
            return row;
        };
        row.child(
            ui::TextInput::plain(&theme, "preset-maker-label", &self.label)
                .width(px(96.))
                .height(px(30.))
                .padding_x(px(8.))
                .text_size(px(13.))
                .bg(Hsla::from(theme.background))
                .border(Hsla::from(theme.border)),
        )
        .child(
            ui::SegmentedControl::editor(
                &theme,
                "preset-maker-width",
                MAKER_WIDTHS
                    .into_iter()
                    .map(|width| ui::SegmentOption::new(format!("{width}×"), key.width == width))
                    .collect(),
            )
            .on_select(cx.listener(|this, index: &usize, window, cx| {
                if let Some(width) = MAKER_WIDTHS.get(*index) {
                    this.model.set_width(*width);
                }
                this.focus_root(window, cx);
                cx.notify();
            })),
        )
        .child(
            ui::Button::plain(
                &theme,
                "preset-maker-delete",
                ui::ButtonVariant::Gray,
                ui::ButtonSize::Sm,
            )
            .label(t!("Delete"))
            .on_click(cx.listener(|this, _, window, cx| {
                this.model.delete_selected();
                this.focus_root(window, cx);
                this.select(None, cx);
            })),
        )
    }
}

impl Render for InputPresetMaker {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.theme.refresh(window, cx, false);
        let theme = self.theme;
        let can_save = self.model.can_save();

        let bar = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(16.))
            .min_h(px(34.))
            .when_some(self.model.start, |bar, chosen| {
                bar.child(
                    div()
                        .text_color(Hsla::from(theme.muted_foreground))
                        .child(t!("Start from")),
                )
                .child(
                    ui::SegmentedControl::editor(
                        &theme,
                        "preset-maker-start",
                        MakerStart::ALL
                            .into_iter()
                            .map(|start| {
                                ui::SegmentOption::new(start_label(start), start == chosen)
                            })
                            .collect(),
                    )
                    .on_select(cx.listener(
                        |this, index: &usize, window, cx| {
                            if let Some(start) = MakerStart::ALL.get(*index) {
                                this.start_from(*start, cx);
                            }
                            this.focus_root(window, cx);
                        },
                    )),
                )
            })
            .child(self.flag(
                "preset-maker-capture",
                t!("Press keys to add"),
                self.model.capture,
                cx,
                |model| model.capture = !model.capture,
            ))
            .child(self.flag(
                "preset-maker-mouse",
                t!("Mouse"),
                self.model.mouse.is_some(),
                cx,
                PresetMaker::toggle_mouse,
            ))
            .child(div().flex_1())
            .child(
                ui::Button::plain(
                    &theme,
                    "preset-maker-clear",
                    ui::ButtonVariant::Gray,
                    ui::ButtonSize::Sm,
                )
                .label(t!("Clear"))
                .on_click(cx.listener(|this, _, window, cx| {
                    this.model.clear();
                    this.focus_root(window, cx);
                    this.select(None, cx);
                })),
            );

        let footer = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .child(
                ui::TextInput::plain(&theme, "preset-maker-name", &self.name)
                    .width(px(200.))
                    .height(px(30.))
                    .padding_x(px(8.))
                    .text_size(px(13.))
                    .bg(Hsla::from(theme.background))
                    .border(Hsla::from(theme.border)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(12.))
                    .text_color(Hsla::from(theme.muted_foreground))
                    .when(self.save_failed, |this| {
                        this.child(t!("Couldn't save this preset. Try again."))
                    }),
            )
            .child(
                ui::Button::plain(
                    &theme,
                    "preset-maker-cancel",
                    ui::ButtonVariant::Gray,
                    ui::ButtonSize::Md,
                )
                .label(t!("Cancel"))
                .on_click(|_, window, _| window.remove_window()),
            )
            .child(
                ui::Button::plain(
                    &theme,
                    "preset-maker-save",
                    ui::ButtonVariant::Primary,
                    ui::ButtonSize::Md,
                )
                .label(t!("Save"))
                .disabled(!can_save)
                .on_click(cx.listener(|this, _, window, cx| this.save(window, cx))),
            );

        let shell = div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key_down))
            .on_key_up(cx.listener(|this, event: &gpui::KeyUpEvent, _, cx| {
                this.on_key_up(event, cx);
            }))
            .on_modifiers_changed(cx.listener(
                |this, event: &gpui::ModifiersChangedEvent, window, cx| {
                    this.on_modifiers(event.modifiers, window, cx);
                },
            ))
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                this.drag_to(event, cx);
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.drag.take().is_some() {
                        cx.notify();
                    }
                }),
            )
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .gap(px(8.))
            .px(px(32.))
            .pt(px(44.))
            .pb(px(20.))
            .bg(Hsla::from(theme.background))
            .font_family(crate::theme::ui_font())
            .font_weight(FontWeight::MEDIUM)
            .text_size(px(13.))
            .text_color(theme.foreground)
            .child(bar)
            .child(self.render_board(cx))
            .child(self.render_selected_key(cx))
            .child(div().flex_1())
            .child(footer);

        #[cfg(target_os = "windows")]
        let shell = shell.child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .h(px(36.))
                .flex()
                .justify_end()
                .window_control_area(gpui::WindowControlArea::Drag)
                .child(crate::ui::windows_caption_controls(
                    theme,
                    window.is_window_active(),
                    window.is_maximized(),
                    true,
                    false,
                )),
        );

        shell
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use frameever_project::input_display::BuiltinPresetId;

    #[test]
    fn a_new_preset_starts_from_wasd_and_an_edit_starts_from_the_saved_layout() {
        let fresh = MakerSeed {
            editing: None,
            name: "My preset".to_string(),
            layout: None,
        }
        .model();
        assert_eq!(
            fresh.layout().keys,
            BuiltinPresetId::WasdMouse.layout().keys
        );
        assert_eq!(fresh.start, Some(MakerStart::WasdMouse));
        assert_eq!(fresh.name, "My preset");

        let mut saved = BuiltinPresetId::WasdMouse.layout();
        saved.mouse = None;
        saved.keys.truncate(2);
        let edited = MakerSeed {
            editing: Some("user:p-1".to_string()),
            name: "Mine".to_string(),
            layout: Some(saved.clone()),
        }
        .model();
        assert_eq!(edited.layout(), saved);
        assert_eq!(edited.start, None);
    }

    #[test]
    fn the_mouse_on_the_board_is_drawn_inside_its_block() {
        let (wide, high) = (MOUSE_BLOCK.0 as f32 * UNIT, MOUSE_BLOCK.1 as f32 * UNIT);
        for part in MousePart::ALL {
            for (left, top, width, height) in mouse_part_shapes(part) {
                assert!(left + width <= wide && top + height <= high, "{part:?}");
            }
            assert!(!mouse_part_label(part).is_empty());
        }
        for start in MakerStart::ALL {
            assert!(!start_label(start).is_empty());
        }
    }

    #[test]
    fn the_board_fits_inside_the_window() {
        assert!(UNIT * MAKER_COLUMNS as f32 + 64. <= MAKER_WINDOW_WIDTH);
        assert!(UNIT * MAKER_ROWS as f32 + 240. <= MAKER_WINDOW_HEIGHT);
    }
}
