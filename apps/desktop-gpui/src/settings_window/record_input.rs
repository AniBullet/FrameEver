use gpui::{Context, FontWeight, IntoElement, ParentElement, Styled, div, prelude::*, px};

use super::SettingsWindow;
use crate::i18n::{Language, n, t, tr};

pub const NOTICE: [&str; 3] = [
    n!("Saved only in this project's folder on this PC"),
    n!("Read only while recording"),
    n!("Clear it any time in the editor"),
];

pub fn learn_more_url(language: Language) -> String {
    let repository = format!("https://github.com/{}", crate::brand::RELEASES_REPO);
    match language {
        Language::SimplifiedChinese => format!("{repository}/blob/main/README.md#操作记录"),
        Language::English => format!("{repository}/blob/main/README.en.md#input-record"),
    }
}

impl SettingsWindow {
    pub(super) fn render_record_input_row(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let open = self.record_input_info;
        let row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.))
            .min_h(px(46.))
            .p(px(12.))
            .child(div().text_size(px(13.)).child(t!("Record input")))
            .child(self.icon_button(
                "record-input-info",
                "icons/info.svg",
                t!("About the input record"),
                cx,
                |this, cx| {
                    this.record_input_info = !this.record_input_info;
                    cx.notify();
                },
            ))
            .child(div().flex_1())
            .child(self.toggle(
                "capture-keyboard",
                self.settings.capture_keyboard_events,
                cx,
                |this, cx| {
                    this.settings.capture_keyboard_events = !this.settings.capture_keyboard_events;
                    let value = this.settings.capture_keyboard_events;
                    this.write_bool("captureKeyboardEvents", value, cx);
                },
            ));
        let notice = open.then(|| {
            let url = learn_more_url(crate::i18n::current());
            self.note_box()
                .id("record-input-notice")
                .mx(px(12.))
                .mb(px(12.))
                .text_size(px(12.))
                .line_height(px(16.))
                .text_color(theme.settings_muted())
                .children(NOTICE.map(|line| {
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(8.))
                        .child("·")
                        .child(tr(line))
                }))
                .child(
                    div()
                        .id("record-input-learn-more")
                        .tab_index(0)
                        .mt(px(2.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.link)
                        .cursor_pointer()
                        .child(t!("Learn more"))
                        .on_click({
                            let url = url.clone();
                            move |_, _, cx| cx.open_url(&url)
                        })
                        .on_key_down(move |event: &gpui::KeyDownEvent, _, cx| {
                            if event.keystroke.key == "enter" {
                                cx.open_url(&url);
                            }
                        }),
                )
        });
        div()
            .flex()
            .flex_col()
            .child(row)
            .children(notice)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings_window::catalog::Row;

    #[test]
    fn the_record_input_row_is_a_label_and_its_notice_is_three_short_lines() {
        assert_eq!(Row::RecordInput.label(), "Record input");
        assert_eq!(Row::RecordInput.description(), None);
        assert_eq!(NOTICE.len(), 3);
        for line in NOTICE {
            assert!(line.len() <= 48, "{line}");
            assert!(!line.ends_with('.'), "{line}");
        }
    }

    #[test]
    fn learn_more_opens_the_readme_section_in_the_users_language() {
        assert_eq!(
            learn_more_url(Language::English),
            "https://github.com/AniBullet/FrameEver/blob/main/README.en.md#input-record"
        );
        assert_eq!(
            learn_more_url(Language::SimplifiedChinese),
            "https://github.com/AniBullet/FrameEver/blob/main/README.md#操作记录"
        );
    }

    #[test]
    fn both_readmes_have_the_section_learn_more_points_to() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let chinese = std::fs::read_to_string(root.join("README.md")).unwrap();
        let english = std::fs::read_to_string(root.join("README.en.md")).unwrap();

        assert!(chinese.lines().any(|line| line == "## 操作记录"));
        assert!(english.lines().any(|line| line == "## Input record"));
    }
}
