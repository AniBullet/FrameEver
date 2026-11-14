//! `Select`'s trigger -- the closed half of the control [`crate::ui::Menu`]
//! opens. `KSelect.Trigger` in the Solid app is `flex h-8 rounded-lg bg-gray-3
//! px-2` plus a chevron that rotates on `data-expanded`; the settings window's
//! `SelectSettingItem` is the same shape at `px-2.5 py-1.5` with a border.
//!
//! The chevron does not rotate: this gpui rev has no transform (the same gap
//! that leaves the camera bubble's mirror button disabled).

use std::{cell::Cell, rc::Rc};

use gpui::{
    App, Bounds, ClickEvent, ElementId, FontWeight, Hsla, InteractiveElement, IntoElement,
    KeyDownEvent, KeyboardClickEvent, ParentElement, Pixels, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, canvas, div, prelude::FluentBuilder, px, svg,
};

use super::menu::OpenHandler;
use crate::theme::Theme;

#[derive(IntoElement)]
pub struct Select {
    id: ElementId,
    label: SharedString,
    padding_x: Pixels,
    padding_y: Pixels,
    height: Option<Pixels>,
    radius: Pixels,
    text_size: Pixels,
    /// The two triggers disagree: `SelectSettingItem`'s button names no
    /// `font-*` class and so inherits `body`'s Medium, while the editor's
    /// `KSelect.Value` says `font-normal` explicitly (`ConfigSidebar.tsx:740`
    /// and friends) and drops back to Regular.
    weight: FontWeight,
    bg: Option<Hsla>,
    border: Option<Hsla>,
    text: Hsla,
    chevron: Hsla,
    chevron_size: Pixels,
    /// `gap-1.5` on the settings row, `gap-2` on the editor's trigger.
    gap: Pixels,
    stretch: bool,
    disabled: bool,
    focus_ring: Option<Hsla>,
    on_click: Option<crate::ui::button::ClickHandler>,
    on_open: Option<OpenHandler>,
}

impl Select {
    /// `SelectSettingItem`'s button: `flex flex-row gap-1.5 text-xs items-center
    /// px-2.5 py-1.5 rounded-lg border bg-gray-3 text-gray-12 border-gray-4`,
    /// radius 8 under the settings material.
    pub fn settings(
        theme: &Theme,
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            padding_x: px(10.),
            padding_y: px(6.),
            height: None,
            radius: px(8.),
            text_size: px(12.),
            // No `font-*` class on the button, so it keeps `body`'s Medium.
            weight: FontWeight::MEDIUM,
            bg: Some(theme.settings_fill()),
            border: Some(theme.settings_border()),
            text: theme.settings_text(),
            chevron: theme.settings_muted(),
            chevron_size: px(14.),
            gap: px(6.),
            stretch: false,
            disabled: false,
            focus_ring: Some(Hsla::from(theme.ring)),
            on_click: None,
            on_open: None,
        }
    }

    /// The editor's `KSelect.Trigger`: a borderless `ed-ctl` pill.
    pub fn plain(theme: &Theme, id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            padding_x: px(10.),
            padding_y: px(0.),
            height: Some(px(crate::theme::CONTROL_HEIGHT)),
            radius: px(7.),
            text_size: px(13.),
            // `KSelect.Value` is `text-sm ... font-normal`, which opts out of
            // `body`'s Medium (`ConfigSidebar.tsx:740`, `:3121`, `:3173`,
            // `:3369`, `:5283`, `:5380`).
            weight: FontWeight::NORMAL,
            bg: Some(Hsla::from(theme.editor.ctl)),
            border: None,
            text: Hsla::from(theme.editor.text_1),
            chevron: Hsla::from(theme.editor.text_3),
            chevron_size: px(14.),
            gap: px(8.),
            focus_ring: None,
            ..Self::settings(theme, id, label)
        }
    }

    /// The label takes the remaining width and the chevron is pushed to the
    /// end -- `EditorButton`'s `rightIconEnd`.
    pub fn stretch_label(mut self) -> Self {
        self.stretch = true;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }

    pub fn on_open(
        mut self,
        handler: impl Fn(&Bounds<Pixels>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open = Some(Box::new(handler));
        self
    }
}

fn activation_button(keystroke: &gpui::Keystroke) -> Option<gpui::KeyboardButton> {
    if keystroke.modifiers.modified() {
        return None;
    }
    match keystroke.key.as_str() {
        "enter" => Some(gpui::KeyboardButton::Enter),
        "space" => Some(gpui::KeyboardButton::Space),
        _ => None,
    }
}

impl RenderOnce for Select {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let Select {
            id,
            label,
            padding_x,
            padding_y,
            height,
            radius,
            text_size,
            weight,
            bg,
            border,
            text,
            chevron,
            chevron_size,
            gap,
            stretch,
            disabled,
            focus_ring,
            on_click,
            on_open,
        } = self;
        let focus_ring = focus_ring.filter(|_| on_click.is_some() && !disabled);
        let on_click = on_click.filter(|_| !disabled).map(Rc::new);
        let bounds = Rc::new(Cell::new(Bounds::default()));

        div()
            .id(id)
            .flex()
            .flex_row()
            .items_center()
            .gap(gap)
            .px(padding_x)
            .when(f32::from(padding_y) > 0., |this| this.py(padding_y))
            .when_some(height, |this, height| this.h(height))
            .rounded(radius)
            .when_some(border, |this, border| this.border_1().border_color(border))
            .when_some(bg, |this, bg| this.bg(bg))
            .when(!disabled, |this| this.cursor_pointer())
            .text_size(text_size)
            // The label is the only text under here -- the chevron is an svg.
            .font_weight(weight)
            .text_color(text)
            .when(disabled, |this| this.opacity(0.5))
            .map(|this| {
                if stretch {
                    this.child(div().flex_1().child(label))
                } else {
                    this.child(label)
                }
            })
            .child(
                svg()
                    .path("icons/chevron-down.svg")
                    .size(chevron_size)
                    .flex_shrink_0()
                    .text_color(chevron),
            )
            .when_some(focus_ring.zip(on_click.clone()), |this, (ring, handler)| {
                let measured = bounds.clone();
                this.tab_index(0)
                    .focus_visible(move |style| style.border_color(ring))
                    .child(
                        canvas(move |area, _, _| measured.set(area), |_, _, _, _| {})
                            .absolute()
                            .size_full(),
                    )
                    .on_key_down(move |event: &KeyDownEvent, window, cx| {
                        let Some(button) = activation_button(&event.keystroke) else {
                            return;
                        };
                        window.prevent_default();
                        cx.stop_propagation();
                        let click = ClickEvent::Keyboard(KeyboardClickEvent {
                            button,
                            bounds: bounds.get(),
                        });
                        handler(&click, window, cx);
                    })
            })
            .when_some(on_click, |this, handler| {
                this.on_click(move |event, window, cx| {
                    if !event.is_keyboard() {
                        handler(event, window, cx);
                    }
                })
            })
            .when_some(on_open.filter(|_| !disabled), crate::ui::Menu::trigger)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(text: &'static str) -> gpui::Keystroke {
        gpui::Keystroke::parse(text).unwrap()
    }

    #[test]
    fn enter_and_space_open_a_focused_select() {
        assert_eq!(
            activation_button(&key("enter")),
            Some(gpui::KeyboardButton::Enter)
        );
        assert_eq!(
            activation_button(&key("space")),
            Some(gpui::KeyboardButton::Space)
        );
        assert_eq!(activation_button(&key("ctrl-enter")), None);
        assert_eq!(activation_button(&key("down")), None);
        assert_eq!(activation_button(&key("a")), None);
    }
}
