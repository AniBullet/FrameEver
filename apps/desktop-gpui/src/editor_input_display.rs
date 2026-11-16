use std::sync::Arc;

use frameever_project::{
    InputDisplay, InputDisplayStyle, ProjectConfiguration,
    input_display::{BuiltinPresetId, DisplayState},
};
use gpui::{
    AnyElement, Context, FontWeight, Hsla, InteractiveElement, IntoElement, ParentElement,
    RenderImage, SharedString, StatefulInteractiveElement, Styled, StyledImage, Window, div, img,
    prelude::FluentBuilder, px, rgb,
};

use crate::{
    editor_window::EditorWindow,
    i18n::{n, t, tc, tr},
    library, ui,
};

const TILE_COLUMNS: f32 = 2.;
const TILE_GAP: f32 = 6.;
const TILE_HEIGHT: f32 = 86.;
const TILE_PICTURE_HEIGHT: f32 = 46.;
const TILE_BACKDROP: u32 = 0x2a2f36;
const TILE_INK: u32 = 0xd8d8d8;

pub(crate) type InputThumbnailKey = (BuiltinPresetId, InputDisplayStyle);

pub(crate) const INPUT_PRESETS: [(BuiltinPresetId, &str); 2] = [
    (BuiltinPresetId::WasdMouse, n!("WASD + mouse")),
    (BuiltinPresetId::FullKeyboard, n!("Full keyboard")),
];

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

pub(crate) fn choose_input_preset(project: &mut ProjectConfiguration, preset: &str) -> bool {
    let block = project
        .input_display
        .get_or_insert_with(InputDisplay::default);
    if block.preset == preset {
        return false;
    }
    block.preset = preset.to_string();
    true
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

fn thumbnail(key: InputThumbnailKey) -> Option<Arc<RenderImage>> {
    let loaded = frameever_rendering::input_display::builtin_preset(key.0, key.1, 1).ok()?;
    let mut picture =
        frameever_rendering::input_display::compose(&loaded, &DisplayState::default());
    frameever_rendering::input_display::straighten_alpha(&mut picture);
    Some(library::rgba_to_render_image(picture))
}

impl EditorWindow {
    pub(crate) fn ensure_input_thumbnails(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let wanted: Vec<InputThumbnailKey> = INPUT_PRESETS
            .iter()
            .flat_map(|(preset, _)| InputDisplayStyle::ALL.map(|style| (*preset, style)))
            .filter(|key| !self.sidebar.input_thumbnails.contains_key(key))
            .collect();
        if wanted.is_empty() || self.sidebar.input_thumbnail_task.is_some() {
            return;
        }
        self.sidebar.input_thumbnail_task = Some(cx.spawn_in(window, async move |this, cx| {
            let pictures = cx
                .background_executor()
                .spawn(async move {
                    wanted
                        .into_iter()
                        .filter_map(|key| thumbnail(key).map(|picture| (key, picture)))
                        .collect::<Vec<_>>()
                })
                .await;
            this.update(cx, |this, cx| {
                this.sidebar.input_thumbnails.extend(pictures);
                this.sidebar.input_thumbnail_task = None;
                cx.notify();
            })
            .ok();
        }));
    }

    pub(crate) fn render_input_display_tab(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.theme;
        let shown = self.project.shown_input_display().cloned();
        let on = shown.is_some();
        let section = ui::Field::section(&theme, t!("Input display")).value(
            ui::Toggle::plain(&theme, "input-display-enabled", on)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.edit_project("input-display-enabled", window, cx, move |project| {
                        switch_input_display(project, !on)
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
        let tiles = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap(px(TILE_GAP))
            .children(INPUT_PRESETS.iter().map(|(preset, label)| {
                let preset = *preset;
                let selected = block.preset == preset.id();
                let picture = self
                    .sidebar
                    .input_thumbnails
                    .get(&(preset, block.style))
                    .cloned();
                div()
                    .id(SharedString::from(format!("input-preset-{}", preset.id())))
                    .w(px(cell))
                    .h(px(TILE_HEIGHT))
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_between()
                    .gap(px(6.))
                    .pt(px(8.))
                    .px(px(6.))
                    .pb(px(6.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(if selected {
                        Hsla::from(theme.foreground)
                    } else {
                        Hsla::from(theme.border)
                    })
                    .bg(rgb(TILE_BACKDROP))
                    .text_color(rgb(TILE_INK))
                    .text_size(px(12.))
                    .font_weight(FontWeight::NORMAL)
                    .tab_index(0)
                    .cursor_pointer()
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
                    .child(tr(label))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.edit_project("input-display-preset", window, cx, move |project| {
                            choose_input_preset(project, preset.id())
                        });
                    }))
            }));

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
            this.edit_project("input-display-style", window, cx, move |project| {
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
                    .child(styles),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn every_listed_preset_has_a_thumbnail_in_every_style() {
        for (preset, _) in INPUT_PRESETS {
            for style in InputDisplayStyle::ALL {
                assert!(thumbnail((preset, style)).is_some(), "{preset:?} {style:?}");
            }
        }
        assert_eq!(
            INPUT_PRESETS.map(|(preset, _)| preset),
            BuiltinPresetId::ALL
        );
    }
}
