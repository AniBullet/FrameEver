use super::{MainWindow, Mode, surfaces::focus_ring};
use crate::{
    app_windows,
    i18n::{t, tr},
    settings_window::Row,
    store::{self, GeneralSettings, SettingsEnum, StudioQuality},
    ui,
};
use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px, svg,
};
use serde_json::Value;

fn summary(mode: Mode, settings: &GeneralSettings) -> String {
    let first = match mode {
        Mode::Studio => match settings.studio_recording_quality {
            StudioQuality::Compatibility => t!("Smaller"),
            StudioQuality::Balanced => t!("Balanced"),
            StudioQuality::Ultra => t!("Maximum"),
        }
        .to_string(),
        Mode::Instant => match settings.instant_mode_max_resolution {
            1280 => "720p".into(),
            1920 => "1080p".into(),
            2560 => "1440p".into(),
            3840 => "4K".into(),
            width => format!("{width} px"),
        },
    };
    let mut result = first;
    if mode == Mode::Instant {
        for (on, label) in [
            (settings.instant_click_highlight, t!("Click highlight")),
            (settings.instant_show_input, t!("Show input")),
        ] {
            if on {
                result.push_str(&format!(" · {label}"));
            }
        }
    }
    result.push_str(&format!(" · {}", t!("{fps} FPS", fps = settings.max_fps)));
    if let Some(seconds) = settings.recording_countdown.filter(|seconds| *seconds > 0) {
        result.push_str(&format!(" · {}", t!("{seconds} s", seconds = seconds)));
    }
    result
}

fn state_icons(mode: Mode, settings: &GeneralSettings) -> Vec<&'static str> {
    if mode != Mode::Instant {
        return Vec::new();
    }
    [
        (
            settings.instant_click_highlight,
            "icons/mouse-pointer-2.svg",
        ),
        (settings.instant_show_input, "icons/keyboard.svg"),
    ]
    .into_iter()
    .filter_map(|(on, icon)| on.then_some(icon))
    .collect()
}

struct OptionRow {
    label: &'static str,
    key: &'static str,
    value: Value,
    choices: Vec<(SharedString, Value)>,
    listed_in_menu: bool,
}

pub(super) type ListedPreset = (String, SharedString);

#[derive(Default)]
pub(super) struct OwnPresets {
    keyboard: Vec<ListedPreset>,
    gamepad: Vec<ListedPreset>,
}

pub(super) fn listed_user_presets() -> OwnPresets {
    let (gamepad, keyboard) = frameever_rendering::input_presets::presets_dir()
        .map(|dir| frameever_rendering::input_presets::list(&dir))
        .unwrap_or_default()
        .into_iter()
        .partition(crate::editor_input_display::draws_a_gamepad);
    let listed = |presets: Vec<frameever_rendering::input_presets::UserPreset>| {
        presets
            .into_iter()
            .map(|preset| (preset.id, preset.name.into()))
            .collect()
    };
    OwnPresets {
        keyboard: listed(keyboard),
        gamepad: listed(gamepad),
    }
}

impl OptionRow {
    fn chosen(&self) -> Option<usize> {
        self.choices
            .iter()
            .position(|(_, value)| *value == self.value)
    }

    fn menu_items(&self) -> Vec<ui::MenuItem> {
        self.choices
            .iter()
            .map(|(label, value)| ui::MenuItem::new(label.clone(), *value == self.value))
            .chain([ui::MenuItem::new(t!("Manage presets…"), false)])
            .collect()
    }

    fn save(&self, index: usize) -> bool {
        self.choices.get(index).is_some_and(|(_, value)| {
            store::set_store_setting(store::GENERAL_SETTINGS, self.key, value.clone())
        })
    }
}

fn rows(mode: Mode, settings: &GeneralSettings, own_presets: &OwnPresets) -> Vec<OptionRow> {
    let numbers = |items: &[(u32, &'static str)]| {
        items
            .iter()
            .map(|(value, label)| (SharedString::from(*label), Value::from(*value)))
            .collect()
    };
    let mut rows = vec![
        if mode == Mode::Studio {
            OptionRow {
                label: Row::StudioQuality.label(),
                key: "studioRecordingQuality",
                value: Value::from(settings.studio_recording_quality.as_json()),
                choices: vec![
                    (t!("Smaller files").into(), Value::from("compatibility")),
                    (t!("Balanced").into(), Value::from("balanced")),
                    (t!("Maximum detail").into(), Value::from("ultra")),
                ],
                listed_in_menu: false,
            }
        } else {
            OptionRow {
                label: Row::InstantResolution.label(),
                key: "instantModeMaxResolution",
                value: Value::from(settings.instant_mode_max_resolution),
                choices: numbers(&[
                    (1280, "720p"),
                    (1920, "1080p"),
                    (2560, "1440p"),
                    (3840, "4K"),
                ]),
                listed_in_menu: false,
            }
        },
        OptionRow {
            label: Row::MaxFps.label(),
            key: "maxFps",
            value: Value::from(settings.max_fps),
            choices: numbers(&[(24, "24"), (25, "25"), (30, "30"), (60, "60"), (120, "120")]),
            listed_in_menu: false,
        },
        OptionRow {
            label: Row::Countdown.label(),
            key: "recordingCountdown",
            value: Value::from(settings.recording_countdown.unwrap_or(0)),
            choices: numbers(&[(0, t!("Off")), (3, "3"), (5, "5"), (10, "10")]),
            listed_in_menu: false,
        },
    ];
    let toggle = |label, key, value| OptionRow {
        label,
        key,
        value: Value::Bool(value),
        choices: vec![
            (t!("Off").into(), Value::Bool(false)),
            (t!("On").into(), Value::Bool(true)),
        ],
        listed_in_menu: false,
    };
    if mode == Mode::Studio {
        rows.push(toggle(
            Row::SeparateCursor.label(),
            "custom_cursor_capture2",
            settings.custom_cursor_capture,
        ));
        rows.push(toggle(
            Row::RecordInput.label(),
            "captureKeyboardEvents",
            settings.capture_keyboard_events,
        ));
    }
    if mode == Mode::Instant {
        rows.push(toggle(
            t!("Click highlight"),
            "instantClickHighlight",
            settings.instant_click_highlight,
        ));
        rows.push(toggle(
            t!("Show input"),
            "instantShowInput",
            settings.instant_show_input,
        ));
        if settings.instant_show_input {
            let device = settings.instant_input_device;
            rows.push(OptionRow {
                label: t!("Device"),
                key: "instantInputDevice",
                value: Value::from(device.as_json()),
                choices: store::InstantInputDevice::ALL
                    .iter()
                    .map(|device| (device.label().into(), Value::from(device.as_json())))
                    .collect(),
                listed_in_menu: false,
            });
            let (key, chosen, built_in, own): (_, _, Vec<ListedPreset>, _) =
                if device == store::InstantInputDevice::Gamepad {
                    (
                        "instantInputGamepadPreset",
                        &settings.instant_input_gamepad_preset,
                        vec![crate::editor_input_display::built_in_gamepad_preset()],
                        &own_presets.gamepad,
                    )
                } else {
                    (
                        "instantInputPreset",
                        &settings.instant_input_preset,
                        crate::editor_input_display::keyboard_presets()
                            .map(|(preset, label)| (preset.id().to_string(), tr(label).into()))
                            .collect(),
                        &own_presets.keyboard,
                    )
                };
            rows.push(OptionRow {
                label: t!("Preset"),
                key,
                value: Value::from(chosen.as_str()),
                choices: built_in
                    .iter()
                    .chain(own)
                    .map(|(id, name)| (name.clone(), Value::from(id.as_str())))
                    .collect(),
                listed_in_menu: true,
            });
            let steps = |items: &[(f64, SharedString)]| {
                items
                    .iter()
                    .map(|(value, label)| (label.clone(), Value::from(*value)))
                    .collect()
            };
            rows.push(OptionRow {
                label: t!("Size"),
                key: "instantInputSize",
                value: Value::from(settings.instant_input_size),
                choices: steps(&[
                    (40.0, t!("Small").into()),
                    (60.0, t!("Medium").into()),
                    (90.0, t!("Large").into()),
                ]),
                listed_in_menu: false,
            });
            rows.push(OptionRow {
                label: t!("Opacity"),
                key: "instantInputOpacity",
                value: Value::from(settings.instant_input_opacity),
                choices: steps(&[
                    (40.0, "40%".into()),
                    (60.0, "60%".into()),
                    (80.0, "80%".into()),
                    (100.0, "100%".into()),
                ]),
                listed_in_menu: false,
            });
            rows.push(OptionRow {
                label: t!("Position"),
                key: "instantInputPosition",
                value: store::instant_input_position_json(settings.instant_input_position),
                choices: store::InstantInputCorner::ALL
                    .iter()
                    .map(|corner| {
                        (
                            corner.label().into(),
                            store::instant_input_position_json(corner.position()),
                        )
                    })
                    .collect(),
                listed_in_menu: false,
            });
        }
    }
    rows.push(OptionRow {
        label: t!("Auto-stop, minutes"),
        key: "recordingAutoStopMinutes",
        value: Value::from(settings.recording_auto_stop_minutes),
        choices: numbers(&[(0, t!("Off")), (5, "5"), (15, "15"), (30, "30")]),
        listed_in_menu: false,
    });
    rows
}

impl MainWindow {
    pub(super) fn close_recording_options(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.recording_options_open {
            return false;
        }
        self.recording_options_open = false;
        self.recording_options_menu = None;
        window.focus(&self.recording_options_trigger, cx);
        cx.notify();
        true
    }

    pub(super) fn render_recording_options_summary(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        if self.session.read(cx).phase != crate::session::Phase::Idle {
            return None;
        }
        let settings = GeneralSettings::load();
        let label: SharedString = format!(
            "{}: {}",
            t!("Recording options"),
            summary(self.mode, &settings)
        )
        .into();
        let tooltip = label.clone();
        let theme = self.theme;
        let button = focus_ring(
            div()
                .id("recording-options-summary")
                .track_focus(&self.recording_options_trigger)
                .tab_index(0)
                .flex()
                .flex_shrink_0()
                .h(px(28.))
                .items_center()
                .gap(px(8.))
                .px(px(6.))
                .rounded(px(8.))
                .aria_label(label)
                .tooltip(move |_, cx| ui::Tooltip::new(&theme, tooltip.clone()).view(cx))
                .tooltip_show_delay(ui::TOOLTIP_SHOW_DELAY)
                .when(self.recording_options_open, |style| {
                    style.bg(theme.accent).text_color(theme.foreground)
                })
                .hover(move |style| style.bg(theme.accent).text_color(theme.foreground))
                .child(
                    svg()
                        .path("icons/sliders-horizontal.svg")
                        .size(px(16.))
                        .flex_shrink_0()
                        .text_color(theme.muted_foreground),
                )
                .children(state_icons(self.mode, &settings).into_iter().map(|icon| {
                    svg()
                        .path(icon)
                        .size(px(14.))
                        .flex_shrink_0()
                        .text_color(theme.muted_foreground)
                })),
            theme,
        )
        .on_click(cx.listener(|this, _, window, cx| {
            if !this.close_recording_options(window, cx) {
                this.clear_mode_hover();
                this.recording_options_open = true;
                this.recording_options_menu = None;
                this.recording_options_presets = listed_user_presets();
                this.recording_options_save_failed = false;
                window.focus(&this.recording_options_focus, cx);
                cx.notify();
            }
        }));
        Some(
            div()
                .flex()
                .flex_1()
                .min_w_0()
                .items_center()
                .child(button)
                .into_any_element(),
        )
    }

    pub(super) fn render_recording_options(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        if !self.recording_options_open
            || self.panel.is_some()
            || self.session.read(cx).phase != crate::session::Phase::Idle
        {
            return None;
        }
        let theme = self.theme;
        let settings = GeneralSettings::load();
        let content = rows(self.mode, &settings, &self.recording_options_presets)
            .into_iter()
            .map(|row| {
                let key = row.key;
                let label = tr(row.label);
                let line = div()
                    .flex()
                    .min_h(px(34.))
                    .flex_shrink_0()
                    .items_center()
                    .gap(px(8.))
                    .px(px(8.))
                    .child(div().flex_1().min_w_0().text_size(px(12.)).child(label));
                if row.listed_in_menu {
                    let shown = row
                        .chosen()
                        .or((!row.choices.is_empty()).then_some(0))
                        .map(|index| row.choices[index].0.clone())
                        .unwrap_or_default();
                    let items = row.menu_items();
                    return line.child(
                        div().min_w_0().max_w(px(200.)).child(
                            ui::Select::settings(
                                &theme,
                                SharedString::from(format!("recording-options-{key}")),
                                shown,
                            )
                            .on_click(cx.listener(
                                move |this, event: &gpui::ClickEvent, _, cx| {
                                    this.recording_options_menu =
                                        Some(ui::MenuState::new(event.position(), &items));
                                    cx.notify();
                                },
                            )),
                        ),
                    );
                }
                let options = row
                    .choices
                    .iter()
                    .map(|(label, value)| {
                        ui::SegmentOption::new(label.clone(), *value == row.value)
                    })
                    .collect();
                line.child(
                    ui::SegmentedControl::settings(
                        &theme,
                        SharedString::from(format!("recording-options-{key}")),
                        options,
                    )
                    .text_size(px(11.))
                    .item_height(px(24.))
                    .item_padding(px(7.), px(0.))
                    .on_select(cx.listener(
                        move |this, index: &usize, _, cx| {
                            this.recording_options_save_failed = !row.save(*index);
                            if this.recording_options_save_failed {
                                tracing::warn!(key, "recording option could not be saved");
                            }
                            cx.notify();
                        },
                    )),
                )
            });
        Some(
            div()
                .id("recording-options-popover")
                .track_focus(&self.recording_options_focus)
                .tab_group()
                .absolute()
                .bottom(px(44.))
                .left(px(8.))
                .right(px(8.))
                .max_h(px(298.))
                .overflow_y_scroll()
                .occlude()
                .bg(theme.popover)
                .border_1()
                .border_color(theme.border)
                .rounded(px(16.))
                .p(px(4.))
                .flex()
                .flex_col()
                .text_color(theme.foreground)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .h(px(24.))
                        .px(px(8.))
                        .text_size(px(12.))
                        .child(t!("Recording options"))
                        .child(
                            focus_ring(
                                div()
                                    .id("recording-options-close")
                                    .tab_index(0)
                                    .size(px(24.))
                                    .rounded(px(8.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .aria_label(t!("Close"))
                                    .child(
                                        svg()
                                            .path("icons/x.svg")
                                            .size(px(14.))
                                            .text_color(theme.muted_foreground),
                                    ),
                                theme,
                            )
                            .on_click(cx.listener(
                                |this, _, window, cx| {
                                    this.close_recording_options(window, cx);
                                },
                            )),
                        ),
                )
                .children(content)
                .when(self.recording_options_save_failed, |element| {
                    element.child(
                        div()
                            .text_size(px(11.))
                            .text_color(theme.warning_text)
                            .child(t!("Couldn't save your recording settings. Try again.")),
                    )
                })
                .child(div().h(px(1.)).flex_shrink_0().my(px(4.)).bg(theme.border))
                .child(
                    focus_ring(
                        div()
                            .id("recording-options-settings")
                            .tab_index(0)
                            .h(px(34.))
                            .px(px(8.))
                            .text_color(theme.link)
                            .flex()
                            .items_center()
                            .text_size(px(12.))
                            .child(t!("All recording settings")),
                        theme,
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.close_recording_options(window, cx);
                        let mode = this.mode;
                        cx.defer(move |cx| app_windows::open_quality_settings(mode, cx));
                    })),
                )
                .into_any_element(),
        )
    }

    fn recording_options_menu_row(&self) -> Option<OptionRow> {
        rows(
            self.mode,
            &GeneralSettings::load(),
            &self.recording_options_presets,
        )
        .into_iter()
        .find(|row| row.listed_in_menu)
    }

    fn choose_from_recording_options_menu(&mut self, index: usize, cx: &mut Context<Self>) {
        self.recording_options_menu = None;
        if let Some(row) = self.recording_options_menu_row() {
            if index == row.choices.len() {
                self.recording_options_open = false;
                cx.defer(|cx| {
                    app_windows::open_settings_row(
                        crate::settings_window::catalog::Row::InputPresets,
                        cx,
                    )
                });
                cx.notify();
                return;
            }
            self.recording_options_save_failed = !row.save(index);
            if self.recording_options_save_failed {
                tracing::warn!(key = row.key, "recording option could not be saved");
            }
        }
        cx.notify();
    }

    pub(super) fn recording_options_menu_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        let Some(menu) = self
            .recording_options_menu
            .as_mut()
            .filter(|_| self.recording_options_open)
        else {
            return false;
        };
        match menu.on_key(key) {
            ui::MenuKey::Commit(index) => self.choose_from_recording_options_menu(index, cx),
            ui::MenuKey::Dismiss => self.recording_options_menu = None,
            ui::MenuKey::Moved => {}
            ui::MenuKey::Ignored => return false,
        }
        cx.notify();
        true
    }

    pub(super) fn render_recording_options_menu(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let state = self
            .recording_options_menu
            .as_ref()
            .filter(|_| self.recording_options_open)?;
        let row = self.recording_options_menu_row()?;
        Some(
            ui::Menu::plain(
                &self.theme,
                "recording-options-menu",
                row.menu_items(),
                state,
            )
            .on_select(cx.listener(|this, index: &usize, _, cx| {
                this.choose_from_recording_options_menu(*index, cx);
            }))
            .on_dismiss(cx.listener(|this, _, _, cx| {
                this.recording_options_menu = None;
                cx.notify();
            }))
            .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::InputEvent as _;

    fn press(cx: &mut gpui::VisualTestContext, key: &str) {
        cx.update(|window, cx| {
            window.refresh();
            window.draw(cx).clear(cx);
            let keystroke = gpui::Keystroke::parse_with_os(key, cx.platform_info().os).unwrap();
            window.dispatch_event(
                gpui::KeyDownEvent {
                    keystroke,
                    is_held: false,
                    prefer_character_input: false,
                }
                .to_platform_input(),
                cx,
            );
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            let keystroke = gpui::Keystroke::parse_with_os(key, cx.platform_info().os).unwrap();
            window.dispatch_event(gpui::KeyUpEvent { keystroke }.to_platform_input(), cx);
        });
        cx.run_until_parked();
    }

    struct TestStore(tempfile::TempDir);

    impl TestStore {
        fn new() -> Self {
            let directory = tempfile::tempdir().unwrap();
            let storage = directory.path().join("storage");
            std::fs::create_dir_all(&storage).unwrap();
            store::set_test_tauri_store_path(Some(directory.path().join("store")));
            assert!(store::set_store_setting(
                store::GENERAL_SETTINGS,
                "storagePath",
                serde_json::json!(storage)
            ));
            Self(directory)
        }
    }

    impl Drop for TestStore {
        fn drop(&mut self) {
            store::set_test_tauri_store_path(None);
        }
    }

    #[test]
    fn recording_options_summary_follows_settings_and_screenshot_has_none() {
        let _store = TestStore::new();
        let mut settings = GeneralSettings::load();
        settings.max_fps = 30;
        settings.recording_countdown = Some(5);
        settings.instant_mode_max_resolution = 2560;
        assert_eq!(summary(Mode::Instant, &settings), "1440p · 30 FPS · 5 s");
        settings.max_fps = 60;
        settings.recording_countdown = None;
        assert_eq!(summary(Mode::Instant, &settings), "1440p · 60 FPS");
        settings.max_fps = 48;
        settings.instant_mode_max_resolution = 2000;
        settings.recording_countdown = Some(7);
        assert_eq!(summary(Mode::Instant, &settings), "2000 px · 48 FPS · 7 s");
        settings.recording_countdown = Some(0);
        for (quality, expected) in [
            (StudioQuality::Compatibility, "Smaller · 48 FPS"),
            (StudioQuality::Balanced, "Balanced · 48 FPS"),
            (StudioQuality::Ultra, "Maximum · 48 FPS"),
        ] {
            settings.studio_recording_quality = quality;
            assert_eq!(summary(Mode::Studio, &settings), expected);
        }
    }
    #[test]
    fn recording_options_write_the_recording_page_keys_without_losing_other_settings() {
        let fixture = TestStore::new();
        assert!(store::set_store_setting(
            store::GENERAL_SETTINGS,
            "defaultProjectNameTemplate",
            Value::from("Keep {date}")
        ));
        let choose = |mode, key, value: Value| {
            let row = rows(mode, &GeneralSettings::load(), &OwnPresets::default())
                .into_iter()
                .find(|row| row.key == key)
                .unwrap();
            let index = row
                .choices
                .iter()
                .position(|(_, candidate)| *candidate == value)
                .unwrap();
            assert!(row.save(index));
        };
        choose(Mode::Studio, "studioRecordingQuality", Value::from("ultra"));
        choose(Mode::Studio, "maxFps", Value::from(25));
        choose(Mode::Studio, "recordingCountdown", Value::from(10));
        choose(Mode::Studio, "custom_cursor_capture2", Value::Bool(false));
        choose(Mode::Studio, "captureKeyboardEvents", Value::Bool(false));
        let settings = GeneralSettings::load();
        assert_eq!(settings.studio_recording_quality, StudioQuality::Ultra);
        assert_eq!(settings.max_fps, 25);
        assert_eq!(settings.recording_countdown, Some(10));
        assert!(!settings.custom_cursor_capture);
        assert!(!settings.capture_keyboard_events);
        assert_eq!(summary(Mode::Studio, &settings), "Maximum · 25 FPS · 10 s");
        choose(Mode::Instant, "instantModeMaxResolution", Value::from(3840));
        choose(Mode::Instant, "recordingAutoStopMinutes", Value::from(15));
        choose(Mode::Instant, "instantClickHighlight", Value::Bool(true));
        assert_eq!(GeneralSettings::load().instant_mode_max_resolution, 3840);
        assert!(GeneralSettings::load().instant_click_highlight);
        let saved: Value =
            serde_json::from_slice(&std::fs::read(fixture.0.path().join("store")).unwrap())
                .unwrap();
        assert_eq!(saved["general_settings"]["recordingAutoStopMinutes"], 15);
        assert_eq!(saved["general_settings"]["instantClickHighlight"], true);
        assert_eq!(
            saved["general_settings"]["defaultProjectNameTemplate"],
            "Keep {date}"
        );
        assert!(
            !rows(
                Mode::Studio,
                &GeneralSettings::load(),
                &OwnPresets::default()
            )
            .iter()
            .any(|row| row.key == "instantClickHighlight")
        );
    }

    #[test]
    fn the_summary_names_what_an_instant_recording_will_draw_into_the_video() {
        let mut settings = GeneralSettings {
            recording_countdown: None,
            ..GeneralSettings::default()
        };
        assert_eq!(summary(Mode::Instant, &settings), "1080p · 60 FPS");
        settings.instant_click_highlight = true;
        assert_eq!(
            summary(Mode::Instant, &settings),
            "1080p · Click highlight · 60 FPS"
        );
        settings.instant_show_input = true;
        settings.recording_countdown = Some(3);
        assert_eq!(
            summary(Mode::Instant, &settings),
            "1080p · Click highlight · Show input · 60 FPS · 3 s"
        );
        assert_eq!(summary(Mode::Studio, &settings), "Balanced · 60 FPS · 3 s");
    }

    #[test]
    fn the_footer_marks_what_an_instant_recording_will_draw_into_the_video() {
        let mut settings = GeneralSettings::default();
        settings.instant_click_highlight = false;
        settings.instant_show_input = false;
        assert!(state_icons(Mode::Instant, &settings).is_empty());
        settings.instant_show_input = true;
        assert_eq!(
            state_icons(Mode::Instant, &settings),
            ["icons/keyboard.svg"]
        );
        settings.instant_click_highlight = true;
        assert_eq!(
            state_icons(Mode::Instant, &settings),
            ["icons/mouse-pointer-2.svg", "icons/keyboard.svg"]
        );
        assert!(state_icons(Mode::Studio, &settings).is_empty());
    }

    #[test]
    fn the_preset_list_ends_with_the_way_to_manage_presets() {
        let settings = GeneralSettings {
            instant_show_input: true,
            ..GeneralSettings::default()
        };
        let presets = rows(Mode::Instant, &settings, &OwnPresets::default())
            .into_iter()
            .find(|row| row.listed_in_menu)
            .unwrap();
        let labels: Vec<String> = presets
            .menu_items()
            .iter()
            .map(|item| item.label.to_string())
            .collect();
        assert_eq!(labels, ["WASD + mouse", "Full keyboard", "Manage presets…"]);
    }

    #[test]
    fn options_come_in_the_order_they_are_used_with_auto_stop_last() {
        let keys = |mode, settings: &GeneralSettings| -> Vec<&'static str> {
            rows(mode, settings, &OwnPresets::default())
                .iter()
                .map(|row| row.key)
                .collect()
        };
        let mut settings = GeneralSettings::default();
        assert_eq!(
            keys(Mode::Studio, &settings),
            [
                "studioRecordingQuality",
                "maxFps",
                "recordingCountdown",
                "custom_cursor_capture2",
                "captureKeyboardEvents",
                "recordingAutoStopMinutes"
            ]
        );
        settings.instant_show_input = true;
        assert_eq!(
            keys(Mode::Instant, &settings),
            [
                "instantModeMaxResolution",
                "maxFps",
                "recordingCountdown",
                "instantClickHighlight",
                "instantShowInput",
                "instantInputDevice",
                "instantInputPreset",
                "instantInputSize",
                "instantInputOpacity",
                "instantInputPosition",
                "recordingAutoStopMinutes"
            ]
        );
    }

    #[test]
    fn show_input_is_an_instant_option_whose_choices_appear_once_it_is_on() {
        let _store = TestStore::new();
        let keys = |mode| -> Vec<&'static str> {
            rows(mode, &GeneralSettings::load(), &OwnPresets::default())
                .into_iter()
                .map(|row| row.key)
                .filter(|key| {
                    key.starts_with("instantShowInput") || key.starts_with("instantInput")
                })
                .collect()
        };
        let choose = |key, value: Value| {
            let row = rows(
                Mode::Instant,
                &GeneralSettings::load(),
                &OwnPresets::default(),
            )
            .into_iter()
            .find(|row| row.key == key)
            .unwrap();
            let index = row
                .choices
                .iter()
                .position(|(_, candidate)| *candidate == value)
                .unwrap();
            assert!(row.save(index));
        };
        assert_eq!(keys(Mode::Instant), ["instantShowInput"]);
        assert!(keys(Mode::Studio).is_empty());
        assert_eq!(GeneralSettings::load().instant_input_display(), None);
        choose("instantShowInput", Value::Bool(true));
        assert_eq!(
            keys(Mode::Instant),
            [
                "instantShowInput",
                "instantInputDevice",
                "instantInputPreset",
                "instantInputSize",
                "instantInputOpacity",
                "instantInputPosition"
            ]
        );
        assert!(keys(Mode::Studio).is_empty());
        let presets = rows(
            Mode::Instant,
            &GeneralSettings::load(),
            &OwnPresets::default(),
        )
        .into_iter()
        .find(|row| row.key == "instantInputPreset")
        .unwrap();
        assert!(presets.listed_in_menu);
        let row = |key: &str| {
            rows(
                Mode::Instant,
                &GeneralSettings::load(),
                &OwnPresets::default(),
            )
            .into_iter()
            .find(|row| row.key == key)
            .unwrap()
        };
        let corners = row("instantInputPosition");
        assert_eq!(corners.choices.len(), 4);
        assert_eq!(corners.chosen(), Some(2));
        assert_eq!(row("instantInputSize").chosen(), Some(1));
        assert_eq!(row("instantInputOpacity").chosen(), Some(2));
        assert_eq!(row("instantInputDevice").chosen(), Some(0));
        choose("instantInputPreset", Value::from("full-keyboard"));
        choose(
            "instantInputPosition",
            store::instant_input_position_json((1.0, 0.0)),
        );
        choose("instantInputSize", Value::from(90.0));
        choose("instantInputOpacity", Value::from(40.0));
        let shown = GeneralSettings::load().instant_input_display().unwrap();
        assert_eq!(shown.preset, "full-keyboard");
        assert_eq!((shown.position.x, shown.position.y), (1.0, 0.0));
        assert_eq!((shown.size, shown.opacity), (90.0, 40.0));
        assert_eq!(row("instantInputPosition").chosen(), Some(1));
        assert_eq!(row("instantInputSize").chosen(), Some(2));
        assert!(store::set_store_setting(
            store::GENERAL_SETTINGS,
            "instantInputPosition",
            store::instant_input_position_json((0.3, 0.6))
        ));
        assert_eq!(row("instantInputPosition").chosen(), None);
        choose("instantInputDevice", Value::from("gamepad"));
        assert_eq!(
            shown_device(),
            frameever_recording::live_input_display::InputDevice::Gamepad
        );
        assert_eq!(
            keys(Mode::Instant),
            [
                "instantShowInput",
                "instantInputDevice",
                "instantInputGamepadPreset",
                "instantInputSize",
                "instantInputOpacity",
                "instantInputPosition"
            ]
        );
        assert_eq!(row("instantInputGamepadPreset").choices.len(), 1);
        assert_eq!(row("instantInputGamepadPreset").chosen(), Some(0));
        choose("instantShowInput", Value::Bool(false));
        assert_eq!(keys(Mode::Instant), ["instantShowInput"]);
        assert_eq!(GeneralSettings::load().instant_input_display(), None);
    }

    fn shown_device() -> frameever_recording::live_input_display::InputDevice {
        GeneralSettings::load()
            .instant_input_display()
            .unwrap()
            .device
    }

    #[test]
    fn the_instant_preset_row_offers_presets_the_person_made_or_imported() {
        let _store = TestStore::new();
        assert!(store::set_store_setting(
            store::GENERAL_SETTINGS,
            "instantShowInput",
            Value::Bool(true)
        ));
        let user = OwnPresets {
            keyboard: vec![
                ("user:p-1".to_string(), SharedString::from("Speedrun")),
                ("user:p-2".to_string(), SharedString::from("Pixel pad")),
            ],
            gamepad: vec![("user:p-3".to_string(), SharedString::from("No triggers"))],
        };
        let row = || {
            rows(Mode::Instant, &GeneralSettings::load(), &user)
                .into_iter()
                .find(|row| row.key == "instantInputPreset")
                .unwrap()
        };
        let listed = |row: &OptionRow| {
            row.choices
                .iter()
                .map(|(label, value)| (label.to_string(), value.as_str().unwrap().to_string()))
                .collect::<Vec<_>>()
        };

        assert_eq!(
            listed(&row())[2..],
            [
                ("Speedrun".to_string(), "user:p-1".to_string()),
                ("Pixel pad".to_string(), "user:p-2".to_string())
            ]
        );
        assert_eq!(
            listed(&row())[..2]
                .iter()
                .map(|(_, id)| id.as_str())
                .collect::<Vec<_>>(),
            ["wasd-mouse", "full-keyboard"]
        );
        assert_eq!(row().chosen(), Some(0));
        assert!(row().save(3));
        assert_eq!(row().chosen(), Some(3));
        assert_eq!(
            GeneralSettings::load()
                .instant_input_display()
                .unwrap()
                .preset,
            "user:p-2"
        );
        assert_eq!(
            row()
                .menu_items()
                .iter()
                .map(|item| item.checked)
                .collect::<Vec<_>>(),
            [false, false, false, true, false]
        );
        assert!(store::set_store_setting(
            store::GENERAL_SETTINGS,
            "instantInputDevice",
            Value::from("gamepad")
        ));
        let pads = rows(Mode::Instant, &GeneralSettings::load(), &user)
            .into_iter()
            .find(|row| row.listed_in_menu)
            .unwrap();
        assert_eq!(
            listed(&pads),
            [
                ("Gamepad".to_string(), "gamepad".to_string()),
                ("No triggers".to_string(), "user:p-3".to_string())
            ]
        );
        assert!(pads.save(1));
        let shown = GeneralSettings::load().instant_input_display().unwrap();
        assert_eq!(
            (shown.gamepad_preset.as_str(), shown.preset.as_str()),
            ("user:p-3", "user:p-2")
        );
    }

    #[test]
    fn recording_options_keep_the_file_when_saving_fails() {
        let fixture = TestStore::new();
        let row = rows(
            Mode::Studio,
            &GeneralSettings::load(),
            &OwnPresets::default(),
        )
        .remove(0);
        std::fs::write(fixture.0.path().join("store"), b"invalid json").unwrap();
        assert!(!row.save(0));
        assert!(!row.save(usize::MAX));
        assert_eq!(
            std::fs::read(fixture.0.path().join("store")).unwrap(),
            b"invalid json"
        );
    }

    #[gpui::test]
    fn recording_options_enter_opens_escape_returns_focus_and_screenshot_closes(
        cx: &mut gpui::TestAppContext,
    ) {
        let _store = TestStore::new();
        let session = cx.update(|cx| {
            crate::main_window::bind_keys(cx);
            crate::feeds::Feeds::init(cx);
            crate::media_access::MediaAccess::init(cx);
            crate::target_overlay::TargetSelect::init(cx);
            crate::session::RecordingSession::init(cx)
        });
        let observed_session = session.clone();
        let (main, cx) =
            cx.add_window_view(move |window, cx| MainWindow::new(observed_session, window, cx));
        main.update_in(cx, |main, window, cx| {
            main.mode = Mode::Studio;
            window.focus(&main.focus_handle, cx);
        });
        cx.run_until_parked();
        let mut reached = false;
        for _ in 0..24 {
            press(cx, "tab");
            reached = main.update_in(cx, |main, window, _| {
                main.recording_options_trigger.is_focused(window)
            });
            if reached {
                break;
            }
        }
        assert!(reached, "Tab must reach recording options");
        press(cx, "enter");
        main.read_with(cx, |main, _| assert!(main.recording_options_open));
        press(cx, "tab");
        main.update_in(cx, |main, window, _| {
            assert!(!main.recording_options_focus.is_focused(window))
        });
        press(cx, "tab");
        press(cx, "enter");
        assert_eq!(
            GeneralSettings::load().studio_recording_quality,
            StudioQuality::Compatibility
        );
        press(cx, "escape");
        main.update_in(cx, |main, window, _| {
            assert!(!main.recording_options_open);
            assert!(main.recording_options_trigger.is_focused(window));
        });
        press(cx, "enter");
        main.update(cx, |main, cx| {
            assert!(main.recording_options_open);
            main.set_mode(Mode::Instant, cx);
            assert!(!main.recording_options_open);
            assert!(main.render_recording_options(cx).is_none());
            main.set_mode(Mode::Studio, cx);
            assert!(!main.recording_options_open);
        });
        main.update_in(cx, |main, window, cx| {
            window.focus(&main.recording_options_trigger, cx)
        });
        cx.run_until_parked();
        press(cx, "enter");
        main.read_with(cx, |main, _| assert!(main.recording_options_open));
        session.update(cx, |session, cx| {
            session.phase = crate::session::Phase::Starting;
            cx.notify();
        });
        cx.run_until_parked();
        main.update(cx, |main, cx| {
            assert!(!main.recording_options_open);
            assert!(main.render_recording_options_summary(cx).is_none());
        });
    }
}
