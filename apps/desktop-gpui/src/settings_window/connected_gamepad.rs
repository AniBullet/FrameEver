use frameever_project::input_display::GamepadKind;
use frameever_recording::gamepad::ConnectedGamepad;
use gpui::Context;

use super::{Page, SettingsWindow};
use crate::i18n::t;

#[derive(Default)]
pub(super) struct ConnectedGamepadLine {
    read: Option<Option<GamepadKind>>,
    watch: Option<(ConnectedGamepad, gpui::Task<()>)>,
}

impl ConnectedGamepadLine {
    pub(super) fn text(&self) -> Option<&'static str> {
        self.read.map(connected_text)
    }
}

fn connected_text(connected: Option<GamepadKind>) -> &'static str {
    match connected {
        Some(GamepadKind::Xbox) => t!("Xbox controller connected"),
        Some(GamepadKind::PlayStation) => t!("PlayStation controller connected"),
        Some(GamepadKind::Switch) => t!("Switch controller connected"),
        None => t!("No gamepad connected"),
    }
}

impl SettingsWindow {
    pub(super) fn watch_connected_gamepad(&mut self, cx: &mut Context<Self>) {
        if self.page != Page::Instant {
            self.connected_gamepad = Default::default();
            return;
        }
        if self.connected_gamepad.watch.is_some() {
            return;
        }
        let (seen, reports) = flume::unbounded();
        let watch = ConnectedGamepad::watch(move |connected| {
            seen.send(connected).ok();
        });
        let task = cx.spawn(async move |this, cx| {
            while let Ok(connected) = reports.recv_async().await {
                let shown = this.update(cx, |this, cx| {
                    this.connected_gamepad.read = Some(connected);
                    cx.notify();
                });
                if shown.is_err() {
                    return;
                }
            }
        });
        self.connected_gamepad.watch = Some((watch, task));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_line_names_the_connected_controller_family_or_says_there_is_none() {
        let line = |read| ConnectedGamepadLine { read, watch: None }.text();
        assert_eq!(line(None), None);
        assert_eq!(line(Some(None)), Some("No gamepad connected"));
        assert_eq!(
            line(Some(Some(GamepadKind::Xbox))),
            Some("Xbox controller connected")
        );
        assert_eq!(
            line(Some(Some(GamepadKind::PlayStation))),
            Some("PlayStation controller connected")
        );
        assert_eq!(
            line(Some(Some(GamepadKind::Switch))),
            Some("Switch controller connected")
        );
    }
}
