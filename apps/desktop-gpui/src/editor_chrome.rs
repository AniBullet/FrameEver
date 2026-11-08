use gpui::{
    AnyElement, App, ClickEvent, Div, ElementId, FontWeight, Hsla, InteractiveElement, IntoElement,
    ParentElement, SharedString, Stateful, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder, px, svg,
};

use crate::{theme::Theme, ui};

pub(crate) const TITLE_BAR_HEIGHT: f32 = 44.;
pub(crate) const RAIL_WIDTH: f32 = 46.;
pub(crate) const INSPECTOR_PANEL_WIDTH: f32 = 340.;
pub(crate) const INSPECTOR_WIDTH: f32 = RAIL_WIDTH + INSPECTOR_PANEL_WIDTH;
pub(crate) const PANEL_HEAD_HEIGHT: f32 = 40.;

pub(crate) fn project_extension_suffix(theme: &Theme) -> Div {
    div()
        .flex_shrink_0()
        .text_size(px(13.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(Hsla::from(theme.editor.text_3))
        .child(format!(".{}", frameever_utils::brand::PROJECT_EXTENSION))
}

pub(crate) fn title_bar(
    theme: Theme,
    window: &Window,
    compact: bool,
    leading: Vec<AnyElement>,
    trailing: AnyElement,
) -> Div {
    let drag_area = div()
        .flex_1()
        .h_full()
        .when(cfg!(target_os = "windows"), |area| {
            area.occlude()
                .window_control_area(gpui::WindowControlArea::Drag)
        })
        .when(cfg!(target_os = "macos"), |area| {
            area.on_mouse_down(gpui::MouseButton::Left, |event, window, _| {
                if event.click_count == 2 {
                    window.titlebar_double_click();
                } else {
                    window.start_window_move();
                }
            })
        });

    let header = div()
        .relative()
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .h(px(TITLE_BAR_HEIGHT))
        .flex_none()
        .gap(px(6.))
        .px(px(10.))
        .when(compact, |header| header.px(px(6.)))
        .when(cfg!(target_os = "windows"), |header| {
            header.window_control_area(gpui::WindowControlArea::Drag)
        })
        .when(!cfg!(target_os = "windows"), |header| {
            header.child(div().h_full().w(px(82.)).flex_none())
        })
        .children(leading)
        .child(drag_area)
        .child(trailing);

    #[cfg(target_os = "windows")]
    let header = header.child(ui::windows_caption_controls(
        theme,
        window.is_window_active(),
        window.is_maximized(),
        true,
        true,
    ));
    #[cfg(not(target_os = "windows"))]
    let _ = (theme, window);

    header
}

pub(crate) fn title_bar_group() -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(2.))
        .flex_none()
        .when(cfg!(target_os = "windows"), |group| group.occlude())
}

pub(crate) fn title_bar_divider(theme: &Theme) -> Div {
    div()
        .w(px(1.))
        .h(px(16.))
        .mx(px(6.))
        .flex_none()
        .bg(Hsla::from(theme.editor.line_strong))
}

pub(crate) fn primary_button(
    theme: &Theme,
    id: impl Into<ElementId>,
    icon: Option<&'static str>,
    label: impl Into<SharedString>,
    enabled: bool,
) -> Stateful<Div> {
    let editor = theme.editor;
    div()
        .id(id.into())
        .relative()
        .flex()
        .flex_row()
        .items_center()
        .justify_center()
        .gap(px(6.))
        .pl(px(if icon.is_some() { 12. } else { 14. }))
        .pr(px(14.))
        .h(px(crate::theme::CONTROL_HEIGHT))
        .flex_none()
        .overflow_hidden()
        .rounded(px(8.))
        .bg(editor.accent)
        .when(enabled, |button| {
            button
                .tab_index(0)
                .cursor_pointer()
                .hover(move |style| style.bg(editor.accent_hover))
                .active(move |style| style.bg(editor.accent_active))
        })
        .when(!enabled, |button| button.opacity(0.5))
        .text_size(px(13.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(gpui::white())
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .h(px(1.))
                .bg(gpui::hsla(0., 0., 1., 0.22)),
        )
        .children(icon.map(|icon| {
            svg()
                .path(icon)
                .size(px(16.))
                .flex_shrink_0()
                .text_color(gpui::white())
        }))
        .child(label.into())
}

pub(crate) fn rail_item(
    theme: &Theme,
    (id, index): (&'static str, usize),
    icon: &'static str,
    label: SharedString,
    selected: bool,
    disabled: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let theme = *theme;
    div()
        .id(ElementId::NamedInteger(
            SharedString::from(format!("{id}-tooltip")),
            index as u64,
        ))
        .flex_none()
        .tooltip_show_delay(ui::TOOLTIP_SHOW_DELAY)
        .tooltip(move |_, cx| ui::Tooltip::new(&theme, label.clone()).view(cx))
        .child(
            ui::IconButton::new((id, index), icon)
                .size(px(32.))
                .icon_size(px(16.))
                .color(theme.muted_foreground.into())
                .disabled(disabled)
                .when(selected, |button| {
                    button
                        .filled(theme.selection.into(), None)
                        .color(theme.foreground.into())
                })
                .on_click(on_click),
        )
}

pub(crate) fn inspector(
    theme: &Theme,
    rail_id: &'static str,
    rail_items: Vec<AnyElement>,
    panel: Vec<AnyElement>,
) -> Div {
    div()
        .w_full()
        .h_full()
        .flex()
        .flex_row()
        .flex_none()
        .min_h_0()
        .bg(theme.background)
        .child(
            div()
                .id(rail_id)
                .bg(theme.sidebar)
                .w(px(RAIL_WIDTH))
                .h_full()
                .flex_none()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(2.))
                .py(px(6.))
                .overflow_y_scroll()
                .border_r_1()
                .border_color(theme.border)
                .children(rail_items),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .overflow_hidden()
                .children(panel),
        )
}

pub(crate) fn panel_head(
    theme: &Theme,
    title: impl IntoElement,
    actions: Option<AnyElement>,
) -> Div {
    div()
        .h(px(PANEL_HEAD_HEIGHT))
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap(px(8.))
        .px(px(16.))
        .text_size(px(13.))
        .font_weight(FontWeight::MEDIUM)
        .border_b_1()
        .border_color(theme.border)
        .child(div().min_w_0().truncate().child(title))
        .children(actions)
}
