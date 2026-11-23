use std::sync::Arc;

use frameever_project::{AudioVisualizerStyle, AudioVisualsConfiguration};
use frameever_rendering::AudioLevels;
use gpui::{
    AnyElement, Context, FontWeight, Hsla, IntoElement, ParentElement, Styled, Window, div,
    prelude::FluentBuilder, px, svg,
};

use crate::editor_sidebar::SidebarTab;
use crate::editor_timeline::TrackKind;
use crate::editor_window::EditorWindow;
use crate::i18n::{t, tc};
use crate::ui;

impl SidebarTab {
    pub(crate) fn shown_in_audio_only(self) -> bool {
        matches!(self, Self::Background | Self::Audio | Self::Captions)
    }
}

impl TrackKind {
    pub(crate) fn shown_in_audio_only(self) -> bool {
        matches!(
            self,
            Self::Clip | Self::Caption | Self::Text | Self::Shape | Self::Audio | Self::Image
        )
    }
}

impl EditorWindow {
    pub(crate) fn is_audio_only(&self) -> bool {
        self.summary().is_some_and(|summary| summary.audio_only)
    }

    pub(crate) fn audio_visuals_enabled(&self) -> bool {
        self.project
            .audio_visuals
            .as_ref()
            .is_some_and(|visuals| visuals.enabled)
    }

    pub(crate) fn shows_audio_card(&self) -> bool {
        self.project_ready() && self.is_audio_only() && !self.audio_visuals_enabled()
    }

    pub(crate) fn apply_audio_only_timeline(&mut self) {
        if self.is_audio_only() {
            self.timeline.audio_only = true;
            self.timeline
                .rows
                .retain(|row| row.kind.shown_in_audio_only());
        }
    }

    pub(crate) fn refresh_audio_levels(&self) {
        if !self.is_audio_only() {
            return;
        }
        let levels = AudioLevels::from_waveform_peaks(
            &peak_slices(&self.timeline.mic_waveforms),
            &peak_slices(&self.timeline.system_waveforms),
        );
        if let Some(instance) = &self.instance {
            instance
                .render_constants
                .set_audio_levels(Some(Arc::new(levels)));
        }
        if self.audio_visuals_enabled() {
            self.publish_project();
        }
    }

    fn edit_audio_visuals(
        &mut self,
        reason: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut AudioVisualsConfiguration),
    ) {
        self.edit_project(reason, window, cx, move |project| {
            let before = project.audio_visuals.clone();
            let background = &project.background.source;
            let visuals = project.audio_visuals.get_or_insert_with(|| {
                Box::new(AudioVisualsConfiguration::for_background(background))
            });
            change(visuals);
            project.audio_visuals != before
        });
    }

    pub(crate) fn add_audio_visuals(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.edit_audio_visuals("audio-visuals-on", window, cx, |visuals| {
            visuals.enabled = true;
        });
        if let Some(index) = SidebarTab::ALL
            .iter()
            .position(|tab| *tab == SidebarTab::Background)
        {
            self.select_sidebar_tab(index, window, cx);
        }
    }

    pub(crate) fn render_audio_card(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.theme;
        let editor = theme.editor;
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(6.))
            .p(px(32.))
            .text_center()
            .child(
                svg()
                    .path("icons/volume-2.svg")
                    .size(px(24.))
                    .text_color(Hsla::from(editor.text_2)),
            )
            .child(
                div()
                    .mt(px(6.))
                    .text_size(px(14.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(Hsla::from(editor.text_1))
                    .child(t!("Audio only")),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(Hsla::from(editor.text_2))
                    .child(t!("This recording has no video.")),
            )
            .child(
                div()
                    .mt(px(10.))
                    .flex()
                    .flex_row()
                    .gap(px(8.))
                    .child(
                        ui::Button::plain(
                            &theme,
                            "audio-only-export",
                            ui::ButtonVariant::Blue,
                            ui::ButtonSize::Sm,
                        )
                        .label(t!("Export audio"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.open_export(window, cx);
                        })),
                    )
                    .child(
                        ui::Button::plain(
                            &theme,
                            "audio-only-add-visuals",
                            ui::ButtonVariant::Gray,
                            ui::ButtonSize::Sm,
                        )
                        .label(t!("Add visuals"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.add_audio_visuals(window, cx);
                        })),
                    ),
            )
            .into_any_element()
    }

    pub(crate) fn render_audio_visuals_group(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.is_audio_only() {
            return None;
        }
        let theme = self.theme;
        let visuals = self
            .project
            .audio_visuals
            .as_deref()
            .cloned()
            .unwrap_or_default();
        let enabled = visuals.enabled;
        let styles = AudioVisualizerStyle::ALL;
        let size_index = visuals.size_index();
        Some(
            ui::Field::section(&theme, t!("Visuals"))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(12.))
                        .child(ui::Field::editor_row(&theme, t!("Show visuals")).child(
                            ui::Toggle::plain(&theme, "audio-visuals-enabled", enabled).on_click(
                                cx.listener(move |this, _, window, cx| {
                                    this.edit_audio_visuals(
                                        "audio-visuals-enabled",
                                        window,
                                        cx,
                                        move |visuals| visuals.enabled = !enabled,
                                    );
                                }),
                            ),
                        ))
                        .when(enabled, |rows| {
                            rows.child(
                                ui::SegmentedControl::editor(
                                    &theme,
                                    "audio-visualizer-style",
                                    styles
                                        .iter()
                                        .map(|style| {
                                            ui::SegmentOption::new(
                                                style_label(*style),
                                                *style == visuals.style,
                                            )
                                        })
                                        .collect(),
                                )
                                .stretch()
                                .on_select(cx.listener(
                                    move |this, index: &usize, window, cx| {
                                        let Some(style) = styles.get(*index).copied() else {
                                            return;
                                        };
                                        this.edit_audio_visuals(
                                            "audio-visuals-style",
                                            window,
                                            cx,
                                            move |visuals| visuals.style = style,
                                        );
                                    },
                                )),
                            )
                            .child(
                                ui::Field::editor_row(&theme, t!("Size")).child(
                                    ui::SegmentedControl::editor(
                                        &theme,
                                        "audio-visualizer-size",
                                        size_labels()
                                            .into_iter()
                                            .enumerate()
                                            .map(|(index, label)| {
                                                ui::SegmentOption::new(label, index == size_index)
                                            })
                                            .collect(),
                                    )
                                    .on_select(cx.listener(
                                        |this, index: &usize, window, cx| {
                                            let Some(size) = AudioVisualsConfiguration::SIZES
                                                .get(*index)
                                                .copied()
                                            else {
                                                return;
                                            };
                                            this.edit_audio_visuals(
                                                "audio-visuals-size",
                                                window,
                                                cx,
                                                move |visuals| visuals.size = size,
                                            );
                                        },
                                    )),
                                ),
                            )
                        }),
                )
                .into_any_element(),
        )
    }
}

fn peak_slices(waveforms: &[Arc<Vec<f32>>]) -> Vec<&[f32]> {
    waveforms.iter().map(|peaks| peaks.as_slice()).collect()
}

fn style_label(style: AudioVisualizerStyle) -> &'static str {
    match style {
        AudioVisualizerStyle::Bars => t!("Bars"),
        AudioVisualizerStyle::Wave => t!("Wave"),
        AudioVisualizerStyle::Circle => tc!("audio visuals", "Circle"),
    }
}

fn size_labels() -> [&'static str; 3] {
    [t!("Small"), tc!("audio visuals", "Medium"), t!("Large")]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_audio_only_project_keeps_the_background_audio_and_captions_tabs() {
        let shown: Vec<_> = SidebarTab::ALL
            .into_iter()
            .filter(|tab| tab.shown_in_audio_only())
            .collect();
        assert_eq!(
            shown,
            [
                SidebarTab::Background,
                SidebarTab::Audio,
                SidebarTab::Captions
            ]
        );
    }

    #[test]
    fn an_audio_only_timeline_has_no_zoom_scene_or_keyboard_lanes() {
        for kind in [
            TrackKind::Zoom,
            TrackKind::Scene,
            TrackKind::ThreeD,
            TrackKind::Keyboard,
            TrackKind::Style,
            TrackKind::Mask,
        ] {
            assert!(!kind.shown_in_audio_only(), "{kind:?}");
        }
        for kind in [
            TrackKind::Clip,
            TrackKind::Caption,
            TrackKind::Text,
            TrackKind::Shape,
            TrackKind::Audio,
            TrackKind::Image,
        ] {
            assert!(kind.shown_in_audio_only(), "{kind:?}");
        }
    }
}
