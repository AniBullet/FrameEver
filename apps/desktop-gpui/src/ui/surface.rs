//! Surfaces: the card and the two
//! label-plus-control row families the Solid app keeps deliberately distinct
//! (`Setting.tsx`'s bordered card list and `editor/ui.tsx`'s stacked fields).

use gpui::{
    AnyElement, Div, FontWeight, Hsla, IntoElement, ParentElement, RenderOnce, SharedString,
    Styled, Window, div, prelude::FluentBuilder, px,
};

use crate::theme::Theme;

/// `<SectionCard>` / `<SectionRows>` -- `rounded-xl border border-gray-3
/// bg-gray-2`, carrying the `.cap-settings-card` hook the material layer reads.
pub struct Card;

impl Card {
    pub fn settings(theme: &Theme, padded: bool) -> Div {
        div()
            .rounded(px(12.))
            .overflow_hidden()
            .bg(theme.settings_card_bg())
            .when(padded, |this| this.px(px(16.)).py(px(16.)))
    }

    pub fn settings_rows(theme: &Theme, children: Vec<AnyElement>) -> Div {
        let border = theme.settings_border();
        let last = children.len().saturating_sub(1);
        Card::settings(theme, false).flex().flex_col().children(
            children.into_iter().enumerate().map(|(index, child)| {
                div().child(child).when(index != last, |this| {
                    this.child(div().mx(px(12.)).h(px(1.)).bg(border))
                })
            }),
        )
    }
}

/// `<SettingItem>` / `.cap-setting-row { min-height: 46px; padding: 12px }` over
/// `flex flex-row gap-4 justify-between items-center`.
#[derive(IntoElement)]
pub struct SettingRow {
    label: SharedString,
    description: Option<SharedString>,
    control: AnyElement,
    muted: Hsla,
}

impl SettingRow {
    pub fn settings(
        theme: &Theme,
        label: impl Into<SharedString>,
        description: Option<&'static str>,
        control: AnyElement,
    ) -> Self {
        Self {
            label: label.into(),
            description: description.map(SharedString::from),
            control,
            muted: theme.settings_muted(),
        }
    }
}

impl RenderOnce for SettingRow {
    fn render(self, _window: &mut Window, _cx: &mut gpui::App) -> impl IntoElement {
        let SettingRow {
            label,
            description,
            control,
            muted,
        } = self;

        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(16.))
            .min_h(px(46.))
            .p(px(12.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(div().text_size(px(13.)).child(label))
                    .children(description.map(|description| {
                        div()
                            .text_size(px(12.))
                            .line_height(px(16.))
                            .text_color(muted)
                            .child(description)
                    })),
            )
            .child(div().flex().items_center().flex_shrink_0().child(control))
    }
}

/// `<Section>`: a title (with an optional `Pro` pill), an optional description
/// and an optional right-aligned slot, above the section's children.
#[derive(IntoElement)]
pub struct Section {
    title: SharedString,
    description: Option<SharedString>,
    right: Option<AnyElement>,
    children: Vec<AnyElement>,
    pro: bool,
    muted: Hsla,
    accent: Hsla,
}

impl Section {
    pub fn settings(
        theme: &Theme,
        title: impl Into<SharedString>,
        description: Option<&'static str>,
        right: Option<AnyElement>,
        children: Vec<AnyElement>,
    ) -> Self {
        Self {
            title: title.into(),
            description: description.map(SharedString::from),
            right,
            children,
            pro: false,
            muted: theme.settings_muted(),
            accent: Hsla::from(theme.secondary),
        }
    }
}

impl RenderOnce for Section {
    fn render(self, _window: &mut Window, _cx: &mut gpui::App) -> impl IntoElement {
        let Section {
            title,
            description,
            right,
            children,
            pro,
            muted,
            accent,
        } = self;

        div()
            .flex()
            .flex_col()
            // `space-y-2.5`
            .gap(px(10.))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_end()
                    .gap(px(12.))
                    .px(px(4.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .min_w_0()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(title),
                                    )
                                    .when(pro, |this| {
                                        this.child(
                                            div()
                                                .px(px(6.))
                                                .py(px(2.))
                                                .rounded(px(6.))
                                                .bg(accent)
                                                .text_size(px(10.))
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(muted)
                                                .child("PRO"),
                                        )
                                    }),
                            )
                            .children(description.map(|description| {
                                div()
                                    .text_size(px(12.))
                                    .line_height(px(18.))
                                    .text_color(muted)
                                    .child(description)
                            })),
                    )
                    .children(right),
            )
            .children(children)
    }
}
