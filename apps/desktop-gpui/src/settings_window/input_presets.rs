use gpui::{
    Context, Entity, IntoElement, ParentElement, SharedString, Styled, Window, div, prelude::*, px,
};
use serde_json::Value;

use super::SettingsWindow;
use crate::{
    editor_input_display::{
        self, INPUT_PRESETS, ListedPreset, confirm_preset_removal, import_presets_from,
        import_refusal_message, maker_seed, remove_preset, rename_preset,
    },
    i18n::{t, tr},
    ui,
};
use frameever_project::input_display::BuiltinPresetId;
use frameever_rendering::input_presets::UserPreset;

pub(super) struct PresetRename {
    preset: String,
    input: Entity<ui::TextInputState>,
    _events: gpui::Subscription,
}

pub(super) fn own_presets() -> Vec<UserPreset> {
    frameever_rendering::input_presets::presets_dir()
        .map(|dir| frameever_rendering::input_presets::list(&dir))
        .unwrap_or_default()
}

pub(super) fn preset_after_removal<'a>(
    chosen: &'a str,
    removed: &str,
    built_in: BuiltinPresetId,
) -> &'a str {
    if chosen == removed {
        built_in.id()
    } else {
        chosen
    }
}

pub(super) fn preset_menu_labels(presets: &[ListedPreset]) -> Vec<SharedString> {
    presets
        .iter()
        .map(|(_, name)| name.clone())
        .chain([SharedString::from(t!("Manage presets…"))])
        .collect()
}

impl SettingsWindow {
    pub(super) fn reload_input_presets(&mut self, cx: &mut Context<Self>) {
        self.input_presets = editor_input_display::listed_keyboard_presets();
        self.gamepad_presets = editor_input_display::listed_gamepad_presets();
        self.own_input_presets = own_presets();
        cx.notify();
    }

    fn open_preset_maker(
        &mut self,
        preset: Option<UserPreset>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(settings) = window.window_handle().downcast::<Self>() else {
            return;
        };
        let seed = maker_seed(preset);
        let open = self.input_maker;
        cx.defer(move |cx| {
            let maker = crate::app_windows::open_input_preset_maker(
                open,
                seed,
                std::rc::Rc::new(move |_, cx| {
                    settings
                        .update(cx, |settings, _, cx| {
                            settings.input_preset_note = None;
                            settings.reload_input_presets(cx);
                        })
                        .ok();
                }),
                cx,
            );
            settings
                .update(cx, |settings, _, _| settings.input_maker = maker)
                .ok();
        });
    }

    fn import_presets(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
                .spawn(async move { import_presets_from(&folder) })
                .await;
            this.update(cx, |this, cx| {
                this.input_preset_note = match result {
                    Ok(_) => None,
                    Err(refusal) => {
                        tracing::warn!(%refusal, "input preset import refused");
                        Some(import_refusal_message(&refusal).into())
                    }
                };
                this.reload_input_presets(cx);
            })
            .ok();
        })
        .detach();
    }

    fn start_preset_rename(
        &mut self,
        preset: &UserPreset,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = cx.new(|cx| {
            let mut input = ui::TextInputState::single_line(window, cx);
            input.set_text(preset.name.clone(), cx);
            input.focus_and_select_all(window, cx);
            input
        });
        let events = cx.subscribe_in(
            &input,
            window,
            |this: &mut Self, _, event: &ui::TextInputEvent, window, cx| match event {
                ui::TextInputEvent::Confirmed => this.save_preset_rename(window, cx),
                ui::TextInputEvent::Cancelled => {
                    this.input_preset_rename = None;
                    this.focus_root(window, cx);
                    cx.notify();
                }
                ui::TextInputEvent::Blurred => {
                    this.input_preset_rename = None;
                    cx.notify();
                }
                ui::TextInputEvent::Changed => {}
            },
        );
        self.input_preset_rename = Some(PresetRename {
            preset: preset.id.clone(),
            input,
            _events: events,
        });
        cx.notify();
    }

    fn save_preset_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(rename) = &self.input_preset_rename else {
            return;
        };
        let name = rename.input.read(cx).text().trim().to_string();
        if name.is_empty() {
            return;
        }
        self.input_preset_note = (!rename_preset(&rename.preset, &name))
            .then(|| t!("Couldn't rename this preset. Try again.").into());
        self.input_preset_rename = None;
        self.focus_root(window, cx);
        self.reload_input_presets(cx);
    }

    fn remove_own_preset(&mut self, preset: String, window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_in(window, async move |this, cx| {
            if !confirm_preset_removal() {
                return;
            }
            let removed = cx
                .background_executor()
                .spawn({
                    let preset = preset.clone();
                    async move { remove_preset(&preset) }
                })
                .await;
            this.update(cx, |this, cx| {
                if removed {
                    let next = preset_after_removal(
                        &this.settings.instant_input_preset,
                        &preset,
                        BuiltinPresetId::WasdMouse,
                    )
                    .to_string();
                    if next != this.settings.instant_input_preset {
                        this.settings.instant_input_preset = next.clone();
                        this.write("instantInputPreset", Value::from(next), cx);
                    }
                    let next = preset_after_removal(
                        &this.settings.instant_input_gamepad_preset,
                        &preset,
                        BuiltinPresetId::Gamepad,
                    )
                    .to_string();
                    if next != this.settings.instant_input_gamepad_preset {
                        this.settings.instant_input_gamepad_preset = next.clone();
                        this.write("instantInputGamepadPreset", Value::from(next), cx);
                    }
                    this.input_preset_note = None;
                } else {
                    this.input_preset_note =
                        Some(t!("Couldn't remove this preset. Try again.").into());
                }
                this.reload_input_presets(cx);
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn render_input_presets(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let line = || {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.))
                .min_h(px(32.))
                .text_size(px(13.))
        };
        let built_in = INPUT_PRESETS.iter().map(|(_, label)| {
            line()
                .child(div().flex_1().min_w_0().truncate().child(tr(label)))
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(theme.settings_muted())
                        .child(t!("Built in")),
                )
                .into_any_element()
        });
        let own = self
            .own_input_presets
            .iter()
            .enumerate()
            .map(|(index, preset)| {
                let editable = preset.layout.is_some();
                let edited = preset.clone();
                let renamed = preset.clone();
                let removed = preset.id.clone();
                if let Some(rename) = &self.input_preset_rename
                    && rename.preset == preset.id
                {
                    return line()
                        .child(
                            ui::TextInput::settings(
                                &theme,
                                ("input-preset-name", index),
                                &rename.input,
                            )
                            .height(px(28.)),
                        )
                        .into_any_element();
                }
                line()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(preset.name.clone()),
                    )
                    .when(!editable, |this| {
                        this.child(self.button(
                            ("input-preset-rename", index),
                            (ui::ButtonVariant::Gray, Some("icons/pencil.svg")),
                            t!("Rename"),
                            false,
                            cx,
                            move |this, window, cx| {
                                this.start_preset_rename(&renamed, window, cx);
                            },
                        ))
                    })
                    .when(editable, |this| {
                        this.child(self.button(
                            ("input-preset-edit", index),
                            (ui::ButtonVariant::Gray, Some("icons/pencil.svg")),
                            t!("Edit"),
                            false,
                            cx,
                            move |this, window, cx| {
                                this.open_preset_maker(Some(edited.clone()), window, cx);
                            },
                        ))
                    })
                    .child(self.button(
                        ("input-preset-remove", index),
                        (ui::ButtonVariant::Gray, Some("icons/trash.svg")),
                        t!("Remove"),
                        false,
                        cx,
                        move |this, window, cx| {
                            this.remove_own_preset(removed.clone(), window, cx);
                        },
                    ))
                    .into_any_element()
            });
        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .children(built_in)
            .children(own)
            .children(self.input_preset_note.clone().map(|note| {
                div()
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .text_color(theme.warning_text)
                    .child(note)
            }))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(8.))
                    .pt(px(6.))
                    .child(self.button(
                        "input-preset-new",
                        (ui::ButtonVariant::Gray, Some("icons/plus.svg")),
                        t!("New preset…"),
                        false,
                        cx,
                        |this, window, cx| this.open_preset_maker(None, window, cx),
                    ))
                    .child(self.button(
                        "input-preset-import",
                        (ui::ButtonVariant::Gray, Some("icons/import.svg")),
                        t!("Import…"),
                        false,
                        cx,
                        |this, window, cx| this.import_presets(window, cx),
                    )),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removing_the_preset_instant_recordings_use_falls_back_to_wasd_and_mouse() {
        let after =
            |chosen, removed| preset_after_removal(chosen, removed, BuiltinPresetId::WasdMouse);
        assert_eq!(after("user:stick", "user:stick"), "wasd-mouse");
        assert_eq!(after("user:stick", "user:pad"), "user:stick");
        assert_eq!(after("gamepad", "user:pad"), "gamepad");
        assert_eq!(
            preset_after_removal("user:pad", "user:pad", BuiltinPresetId::Gamepad),
            "gamepad"
        );
    }

    #[test]
    fn every_list_of_presets_ends_with_the_way_to_the_place_that_manages_them() {
        let presets = vec![("wasd-mouse".to_string(), SharedString::from("WASD + mouse"))];
        assert_eq!(
            preset_menu_labels(&presets),
            ["WASD + mouse", "Manage presets…"]
        );
        assert_eq!(preset_menu_labels(&[]), ["Manage presets…"]);
    }

    #[test]
    fn a_new_preset_from_settings_starts_from_the_same_seed_as_one_from_the_editor() {
        let seed = maker_seed(None);
        assert_eq!(seed.editing, None);
        assert_eq!(seed.layout, None);
        let edited = maker_seed(Some(UserPreset {
            id: "user:stick".into(),
            name: "Stick".into(),
            layout: None,
        }));
        assert_eq!(edited.editing.as_deref(), Some("user:stick"));
        assert_eq!(edited.name, "Stick");
    }
}
