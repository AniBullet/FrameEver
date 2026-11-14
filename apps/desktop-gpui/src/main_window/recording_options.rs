use super::{MainWindow, Mode, shell::Stage, surfaces::focus_ring};
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

fn summary(mode: Mode, settings: &GeneralSettings) -> Option<String> {
    let first = match mode {
        Mode::Screenshot => return None,
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
    let mut result = format!("{first} · {}", t!("{fps} FPS", fps = settings.max_fps));
    if let Some(seconds) = settings.recording_countdown.filter(|seconds| *seconds > 0) {
        result.push_str(&format!(" · {}", t!("{seconds} s", seconds = seconds)));
    }
    Some(result)
}

struct OptionRow {
    label: &'static str,
    key: &'static str,
    value: Value,
    choices: Vec<(&'static str, Value)>,
}

impl OptionRow {
    fn save(&self, index: usize) -> bool {
        self.choices.get(index).is_some_and(|(_, value)| {
            store::set_store_setting(store::GENERAL_SETTINGS, self.key, value.clone())
        })
    }
}

fn rows(mode: Mode, settings: &GeneralSettings) -> Vec<OptionRow> {
    if mode == Mode::Screenshot {
        return Vec::new();
    }
    let numbers = |items: &[(u32, &'static str)]| {
        items
            .iter()
            .map(|(value, label)| (*label, Value::from(*value)))
            .collect()
    };
    let mut rows = vec![
        if mode == Mode::Studio {
            OptionRow {
                label: Row::StudioQuality.label(),
                key: "studioRecordingQuality",
                value: Value::from(settings.studio_recording_quality.as_json()),
                choices: vec![
                    (t!("Smaller files"), Value::from("compatibility")),
                    (t!("Balanced"), Value::from("balanced")),
                    (t!("Maximum detail"), Value::from("ultra")),
                ],
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
            }
        },
        OptionRow {
            label: Row::MaxFps.label(),
            key: "maxFps",
            value: Value::from(settings.max_fps),
            choices: numbers(&[(24, "24"), (25, "25"), (30, "30"), (60, "60"), (120, "120")]),
        },
        OptionRow {
            label: Row::Countdown.label(),
            key: "recordingCountdown",
            value: Value::from(settings.recording_countdown.unwrap_or(0)),
            choices: numbers(&[(0, t!("Off")), (3, "3"), (5, "5"), (10, "10")]),
        },
    ];
    let toggle = |label, key, value| OptionRow {
        label,
        key,
        value: Value::Bool(value),
        choices: vec![
            (t!("Off"), Value::Bool(false)),
            (t!("On"), Value::Bool(true)),
        ],
    };
    if mode == Mode::Studio {
        rows.push(toggle(
            Row::SeparateCursor.label(),
            "custom_cursor_capture2",
            settings.custom_cursor_capture,
        ));
        rows.push(toggle(
            Row::KeyPresses.label(),
            "captureKeyboardEvents",
            settings.capture_keyboard_events,
        ));
    }
    let general = store::store_section(store::GENERAL_SETTINGS);
    rows.push(OptionRow {
        label: t!("Auto-stop, minutes"),
        key: "recordingAutoStopMinutes",
        value: general
            .get("recordingAutoStopMinutes")
            .and_then(Value::as_u64)
            .map(Value::from)
            .unwrap_or(Value::from(0)),
        choices: numbers(&[(0, t!("Off")), (5, "5"), (15, "15"), (30, "30")]),
    });
    if mode == Mode::Instant {
        rows.push(toggle(
            t!("Click highlight"),
            "instantClickHighlight",
            general
                .get("instantClickHighlight")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ));
    }
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
        window.focus(&self.recording_options_trigger, cx);
        cx.notify();
        true
    }

    pub(super) fn render_recording_options_summary(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let summary = summary(self.mode, &GeneralSettings::load())?;
        let accessible_label = format!("{}: {summary}", t!("Recording options"));
        let theme = self.theme;
        Some(
            focus_ring(
                div()
                    .id("recording-options-summary")
                    .track_focus(&self.recording_options_trigger)
                    .tab_index(0)
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .h(px(28.))
                    .items_center()
                    .gap(px(6.))
                    .px(px(6.))
                    .rounded(px(8.))
                    .text_size(px(12.))
                    .aria_label(accessible_label)
                    .tooltip(move |_, cx| {
                        ui::Tooltip::new(&theme, t!("Recording options")).view(cx)
                    })
                    .tooltip_show_delay(ui::TOOLTIP_SHOW_DELAY)
                    .when(self.recording_options_open, |style| {
                        style.bg(theme.accent).text_color(theme.foreground)
                    })
                    .hover(move |style| style.bg(theme.accent).text_color(theme.foreground))
                    .child(
                        svg()
                            .path("icons/sliders-horizontal.svg")
                            .size(px(14.))
                            .flex_shrink_0()
                            .text_color(theme.muted_foreground),
                    )
                    .child(div().truncate().child(summary)),
                theme,
            )
            .on_click(cx.listener(|this, _, window, cx| {
                if !this.close_recording_options(window, cx) {
                    this.clear_mode_hover();
                    this.recording_options_open = true;
                    this.recording_options_save_failed = false;
                    window.focus(&this.recording_options_focus, cx);
                    cx.notify();
                }
            }))
            .into_any_element(),
        )
    }

    pub(super) fn render_recording_options(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        if !self.recording_options_open
            || self.mode == Mode::Screenshot
            || self.panel.is_some()
            || self.shell.stage() != Stage::Compact
            || self.session.read(cx).phase != crate::session::Phase::Idle
        {
            return None;
        }
        let theme = self.theme;
        let settings = GeneralSettings::load();
        let content = rows(self.mode, &settings).into_iter().map(|row| {
            let key = row.key;
            let label = tr(row.label);
            let options = row
                .choices
                .iter()
                .map(|(label, value)| ui::SegmentOption::new(*label, *value == row.value))
                .collect();
            div()
                .flex()
                .min_h(px(34.))
                .flex_shrink_0()
                .items_center()
                .gap(px(8.))
                .px(px(8.))
                .child(div().flex_1().min_w_0().text_size(px(12.)).child(label))
                .child(
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
                .child(
                    div()
                        .text_size(px(10.))
                        .text_color(theme.muted_foreground)
                        .child(t!(
                            "Auto-stop and click highlight are saved for a future update."
                        )),
                )
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
        assert_eq!(
            summary(Mode::Instant, &settings).as_deref(),
            Some("1440p · 30 FPS · 5 s")
        );
        settings.max_fps = 60;
        settings.recording_countdown = None;
        assert_eq!(
            summary(Mode::Instant, &settings).as_deref(),
            Some("1440p · 60 FPS")
        );
        assert_eq!(summary(Mode::Screenshot, &settings), None);
        settings.max_fps = 48;
        settings.instant_mode_max_resolution = 2000;
        settings.recording_countdown = Some(7);
        assert_eq!(
            summary(Mode::Instant, &settings).as_deref(),
            Some("2000 px · 48 FPS · 7 s")
        );
        settings.recording_countdown = Some(0);
        for (quality, expected) in [
            (StudioQuality::Compatibility, "Smaller · 48 FPS"),
            (StudioQuality::Balanced, "Balanced · 48 FPS"),
            (StudioQuality::Ultra, "Maximum · 48 FPS"),
        ] {
            settings.studio_recording_quality = quality;
            assert_eq!(summary(Mode::Studio, &settings).as_deref(), Some(expected));
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
            let row = rows(mode, &GeneralSettings::load())
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
        assert_eq!(
            summary(Mode::Studio, &settings).as_deref(),
            Some("Maximum · 25 FPS · 10 s")
        );
        choose(Mode::Instant, "instantModeMaxResolution", Value::from(3840));
        choose(Mode::Instant, "recordingAutoStopMinutes", Value::from(15));
        choose(Mode::Instant, "instantClickHighlight", Value::Bool(true));
        assert_eq!(GeneralSettings::load().instant_mode_max_resolution, 3840);
        let saved: Value =
            serde_json::from_slice(&std::fs::read(fixture.0.path().join("store")).unwrap())
                .unwrap();
        assert_eq!(saved["general_settings"]["recordingAutoStopMinutes"], 15);
        assert_eq!(saved["general_settings"]["instantClickHighlight"], true);
        assert_eq!(
            saved["general_settings"]["defaultProjectNameTemplate"],
            "Keep {date}"
        );
        assert!(rows(Mode::Screenshot, &GeneralSettings::load()).is_empty());
        assert!(
            !rows(Mode::Studio, &GeneralSettings::load())
                .iter()
                .any(|row| row.key == "instantClickHighlight")
        );
    }

    #[test]
    fn recording_options_keep_the_file_when_saving_fails() {
        let fixture = TestStore::new();
        let row = rows(Mode::Studio, &GeneralSettings::load()).remove(0);
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
            main.set_mode(Mode::Screenshot, cx);
            assert!(!main.recording_options_open);
            assert!(main.render_recording_options_summary(cx).is_none());
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
        main.read_with(cx, |main, _| assert!(!main.recording_options_open));
    }
}
