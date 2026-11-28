use gpui::{
    AppContext as _, Context, Entity, FocusHandle, FontWeight, InteractiveElement, IntoElement,
    ParentElement, SharedString, StatefulInteractiveElement, Styled, Subscription, Window,
    deferred, div, prelude::FluentBuilder, px, svg,
};

use super::{
    LibraryWindow,
    delete::ButtonTone,
    kind_label, status_label,
    view::{Clock, StatusFilter},
};
use crate::{
    i18n::t,
    library::{
        MediaKind,
        rules::{
            Candidate, DAY_CHOICES, DateOp, Field, IsOp, KindValue, Matching, Rule, SmartFolder,
            TextOp,
        },
    },
    main_window::focus_ring,
    ui,
};

const DIALOG_WIDTH: f32 = 560.;
const FIELD_WIDTH: f32 = 130.;
const OP_WIDTH: f32 = 140.;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Part {
    Field,
    Op,
    Value,
}

struct DraftRule {
    rule: Rule,
    text: Entity<ui::TextInputState>,
}

pub(super) struct SmartDraft {
    id: String,
    name: Entity<ui::TextInputState>,
    matching: Matching,
    rules: Vec<DraftRule>,
    kept: Vec<Rule>,
    extra: serde_json::Map<String, serde_json::Value>,
    open: Option<(usize, Part)>,
    focus: FocusHandle,
    _events: Vec<Subscription>,
}

fn field_label(field: Field) -> &'static str {
    match field {
        Field::Kind => t!("Type"),
        Field::Date => t!("Date"),
        Field::Export => t!("Export state"),
        Field::Title => t!("Title"),
    }
}

fn kind_value_label(value: KindValue) -> &'static str {
    match value {
        KindValue::Studio => kind_label(MediaKind::Studio),
        KindValue::Instant => kind_label(MediaKind::Instant),
        KindValue::Screenshot => kind_label(MediaKind::Screenshot),
        KindValue::Audio => t!("Audio only"),
    }
}

fn days_label(days: u32) -> SharedString {
    match days {
        1 => t!("1 day").into(),
        365 => t!("1 year").into(),
        days => t!("{count} days", count = days).into(),
    }
}

pub(super) fn label(rule: &Rule, part: Part) -> SharedString {
    match (rule, part) {
        (rule, Part::Field) => rule.field().map_or("", field_label).into(),
        (Rule::Kind { op, .. } | Rule::Export { op, .. }, Part::Op) => match op {
            IsOp::Is => t!("is").into(),
            IsOp::IsNot => t!("is not").into(),
        },
        (Rule::Date { op, .. }, Part::Op) => match op {
            DateOp::InTheLast => t!("in the last").into(),
            DateOp::OlderThan => t!("older than").into(),
        },
        (Rule::Title { op, .. }, Part::Op) => match op {
            TextOp::Contains => t!("contains").into(),
            TextOp::DoesNotContain => t!("does not contain").into(),
            TextOp::Is => t!("is").into(),
            TextOp::StartsWith => t!("starts with").into(),
        },
        (Rule::Kind { value, .. }, Part::Value) => kind_value_label(*value).into(),
        (Rule::Date { days, .. }, Part::Value) => days_label(*days),
        (Rule::Export { value, .. }, Part::Value) => status_label(*value).into(),
        (Rule::Title { value, .. }, Part::Value) => value.clone().into(),
        (Rule::Unknown(_), _) => SharedString::default(),
    }
}

pub(super) fn options(rule: &Rule, part: Part) -> Vec<Rule> {
    match (rule, part) {
        (_, Part::Field) => [Field::Kind, Field::Date, Field::Export, Field::Title]
            .into_iter()
            .map(|field| match (rule.field() == Some(field), field) {
                (true, _) => rule.clone(),
                (false, field) => Rule::starting(field),
            })
            .collect(),
        (Rule::Kind { value, .. }, Part::Op) => [IsOp::Is, IsOp::IsNot]
            .map(|op| Rule::Kind { op, value: *value })
            .to_vec(),
        (Rule::Export { value, .. }, Part::Op) => [IsOp::Is, IsOp::IsNot]
            .map(|op| Rule::Export { op, value: *value })
            .to_vec(),
        (Rule::Date { days, .. }, Part::Op) => [DateOp::InTheLast, DateOp::OlderThan]
            .map(|op| Rule::Date { op, days: *days })
            .to_vec(),
        (Rule::Title { value, .. }, Part::Op) => [
            TextOp::Contains,
            TextOp::DoesNotContain,
            TextOp::Is,
            TextOp::StartsWith,
        ]
        .map(|op| Rule::Title {
            op,
            value: value.clone(),
        })
        .to_vec(),
        (Rule::Kind { op, .. }, Part::Value) => [
            KindValue::Studio,
            KindValue::Instant,
            KindValue::Screenshot,
            KindValue::Audio,
        ]
        .map(|value| Rule::Kind { op: *op, value })
        .to_vec(),
        (Rule::Date { op, .. }, Part::Value) => DAY_CHOICES
            .map(|days| Rule::Date { op: *op, days })
            .to_vec(),
        (Rule::Export { op, .. }, Part::Value) => StatusFilter::ALL
            .map(|value| Rule::Export { op: *op, value })
            .to_vec(),
        (Rule::Title { .. } | Rule::Unknown(_), _) => Vec::new(),
    }
}

impl SmartDraft {
    fn folder(&self, cx: &gpui::App) -> SmartFolder {
        let rules = self
            .rules
            .iter()
            .map(|draft| match &draft.rule {
                Rule::Title { op, .. } => Rule::Title {
                    op: *op,
                    value: draft.text.read(cx).text().to_owned(),
                },
                rule => rule.clone(),
            })
            .chain(self.kept.iter().cloned())
            .collect();
        SmartFolder {
            id: self.id.clone(),
            name: self.name.read(cx).text().trim().to_owned(),
            matching: self.matching,
            rules,
            extra: self.extra.clone(),
        }
    }
}

impl LibraryWindow {
    fn draft_rule(
        &self,
        rule: Rule,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (DraftRule, Subscription) {
        let value = match &rule {
            Rule::Title { value, .. } => value.clone(),
            _ => String::new(),
        };
        let text = cx.new(|cx| {
            let mut input = ui::TextInputState::single_line(window, cx);
            input.set_text(value, cx);
            input.set_placeholder(t!("Text"));
            input
        });
        let events = cx.subscribe(&text, |_, _, _: &ui::TextInputEvent, cx| cx.notify());
        (DraftRule { rule, text }, events)
    }

    pub(super) fn open_smart_dialog(
        &mut self,
        id: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_library_menu(cx);
        let held = id.as_ref().and_then(|id| {
            self.library
                .organizer
                .file
                .smart_folders
                .iter()
                .find(|folder| &folder.id == id)
                .cloned()
        });
        let folder = held.unwrap_or_else(|| SmartFolder {
            id: String::new(),
            name: t!("Untitled smart folder").to_owned(),
            matching: Matching::All,
            rules: vec![Rule::starting(Field::Date)],
            extra: Default::default(),
        });
        let name = cx.new(|cx| {
            let mut input = ui::TextInputState::single_line(window, cx);
            input.set_text(folder.name.clone(), cx);
            input
        });
        let mut events = vec![cx.subscribe(&name, |_, _, _: &ui::TextInputEvent, cx| cx.notify())];
        let (known, kept): (Vec<Rule>, Vec<Rule>) = folder
            .rules
            .into_iter()
            .partition(|rule| rule.field().is_some());
        let rules = known
            .into_iter()
            .map(|rule| {
                let (draft, subscription) = self.draft_rule(rule, window, cx);
                events.push(subscription);
                draft
            })
            .collect();
        let focus = cx.focus_handle();
        name.update(cx, |input, cx| input.focus_and_select_all(window, cx));
        self.library.smart = Some(SmartDraft {
            id: folder.id,
            name,
            matching: folder.matching,
            rules,
            kept,
            extra: folder.extra,
            open: None,
            focus,
            _events: events,
        });
        cx.notify();
    }

    pub(super) fn close_smart_dialog(&mut self, cx: &mut Context<Self>) {
        if self.library.smart.take().is_some() {
            self.focus_library_items(cx);
            cx.notify();
        }
    }

    fn save_smart_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(draft) = self.library.smart.as_ref() else {
            return;
        };
        let folder = draft.folder(cx);
        if folder.name.is_empty() {
            return;
        }
        self.close_smart_dialog(cx);
        self.save_smart_folder(folder, window, cx);
    }

    fn pick_rule_part(&mut self, index: usize, rule: Rule, cx: &mut Context<Self>) {
        let Some(draft) = self.library.smart.as_mut() else {
            return;
        };
        draft.open = None;
        if let Some(held) = draft.rules.get_mut(index) {
            if matches!(rule, Rule::Title { .. }) && !matches!(held.rule, Rule::Title { .. }) {
                held.text.update(cx, |input, cx| input.set_text("", cx));
            }
            held.rule = rule;
        }
        cx.notify();
    }

    fn add_smart_rule(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (draft, subscription) = self.draft_rule(Rule::starting(Field::Kind), window, cx);
        if let Some(smart) = self.library.smart.as_mut() {
            smart.rules.push(draft);
            smart._events.push(subscription);
            smart.open = None;
        }
        cx.notify();
    }

    fn rule_trigger(
        &self,
        index: usize,
        part: Part,
        width: f32,
        rule: &Rule,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let theme = self.theme;
        let open = self
            .library
            .smart
            .as_ref()
            .is_some_and(|draft| draft.open == Some((index, part)));
        let id = match part {
            Part::Field => ("smart-rule-field", index),
            Part::Op => ("smart-rule-op", index),
            Part::Value => ("smart-rule-value", index),
        };
        let choices = options(rule, part);
        let rows: Vec<gpui::AnyElement> = choices
            .into_iter()
            .enumerate()
            .map(|(row, choice)| {
                let ticked = choice == *rule;
                let text = label(&choice, part);
                self.popup_row(("smart-rule-choice", index * 16 + row))
                    .aria_label(text.clone())
                    .child(div().flex_1().min_w_0().truncate().child(text))
                    .child(self.popup_tick(ticked))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.pick_rule_part(index, choice.clone(), cx);
                    }))
                    .into_any_element()
            })
            .collect();
        div()
            .relative()
            .w(px(width))
            .flex_shrink_0()
            .child(
                focus_ring(
                    div()
                        .id(id)
                        .role(gpui::Role::ComboBox)
                        .aria_label(label(rule, part))
                        .tab_index(0)
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .h(px(crate::theme::CONTROL_HEIGHT))
                        .px(px(10.))
                        .rounded(px(6.))
                        .border_1()
                        .border_color(theme.input)
                        .bg(theme.background)
                        .cursor_pointer()
                        .text_size(px(13.))
                        .child(div().flex_1().min_w_0().truncate().child(label(rule, part)))
                        .child(
                            svg()
                                .path("icons/chevron-down.svg")
                                .size(px(14.))
                                .text_color(theme.muted_foreground),
                        ),
                    theme,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(draft) = this.library.smart.as_mut() {
                        draft.open = (draft.open != Some((index, part))).then_some((index, part));
                    }
                    cx.notify();
                })),
            )
            .when(open, |this| {
                this.child(
                    deferred(
                        div()
                            .absolute()
                            .top(px(crate::theme::CONTROL_HEIGHT + 4.))
                            .left_0()
                            .w(px(width.max(160.)))
                            .p(px(4.))
                            .flex()
                            .flex_col()
                            .rounded(px(8.))
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.popover)
                            .text_size(px(12.))
                            .text_color(theme.popover_foreground)
                            .occlude()
                            .children(rows),
                    )
                    .with_priority(3),
                )
            })
    }

    fn render_rule_row(
        &self,
        index: usize,
        draft: &DraftRule,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let theme = self.theme;
        let rule = &draft.rule;
        let value: gpui::AnyElement = if matches!(rule, Rule::Title { .. }) {
            div()
                .flex_1()
                .min_w_0()
                .child(ui::TextInput::settings(
                    &theme,
                    ("smart-rule-text", index),
                    &draft.text,
                ))
                .into_any_element()
        } else {
            div()
                .flex_1()
                .min_w_0()
                .child(self.rule_trigger(index, Part::Value, 160., rule, cx))
                .into_any_element()
        };
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(self.rule_trigger(index, Part::Field, FIELD_WIDTH, rule, cx))
            .child(self.rule_trigger(index, Part::Op, OP_WIDTH, rule, cx))
            .child(value)
            .child(
                focus_ring(
                    div()
                        .id(("smart-rule-remove", index))
                        .role(gpui::Role::Button)
                        .aria_label(t!("Remove rule"))
                        .tab_index(0)
                        .flex()
                        .flex_shrink_0()
                        .items_center()
                        .justify_center()
                        .size(px(28.))
                        .rounded(px(6.))
                        .cursor_pointer()
                        .hover(move |style| style.bg(theme.accent))
                        .tooltip(move |_, cx| ui::Tooltip::new(&theme, t!("Remove rule")).view(cx))
                        .tooltip_show_delay(ui::TOOLTIP_SHOW_DELAY)
                        .child(
                            svg()
                                .path("icons/minus.svg")
                                .size(px(14.))
                                .text_color(theme.muted_foreground),
                        ),
                    theme,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(draft) = this.library.smart.as_mut()
                        && index < draft.rules.len()
                    {
                        draft.rules.remove(index);
                        draft.open = None;
                    }
                    cx.notify();
                })),
            )
    }

    fn matching_switch(
        &self,
        matching: Matching,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let theme = self.theme;
        let options = [(Matching::All, t!("all")), (Matching::Any, t!("any"))];
        div()
            .flex()
            .p(px(2.))
            .gap(px(2.))
            .rounded(px(6.))
            .bg(theme.muted)
            .children(
                options
                    .into_iter()
                    .enumerate()
                    .map(|(index, (option, text))| {
                        let on = option == matching;
                        focus_ring(
                            div()
                                .id(("smart-matching", index))
                                .role(gpui::Role::RadioButton)
                                .aria_label(text)
                                .aria_toggled(if on {
                                    gpui::Toggled::True
                                } else {
                                    gpui::Toggled::False
                                })
                                .tab_index(0)
                                .flex()
                                .items_center()
                                .h(px(crate::theme::CONTROL_HEIGHT - 6.))
                                .px(px(10.))
                                .rounded(px(5.))
                                .cursor_pointer()
                                .text_size(px(13.))
                                .when(on, |this| this.bg(theme.secondary))
                                .text_color(if on {
                                    theme.foreground
                                } else {
                                    theme.muted_foreground
                                })
                                .child(text),
                            theme,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(draft) = this.library.smart.as_mut() {
                                draft.matching = option;
                            }
                            cx.notify();
                        }))
                    }),
            )
    }

    pub(super) fn render_smart_dialog(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let theme = self.theme;
        let draft = self.library.smart.as_ref()?;
        let folder = draft.folder(cx);
        let clock = Clock::now();
        let compiled = folder.compile(&clock);
        let matching = self.library.entries.as_ref().map_or(0, |entries| {
            entries
                .iter()
                .filter(|entry| compiled.matches(&Candidate::new(entry), &clock))
                .count()
        });
        let counted = if matching == 1 {
            SharedString::from(t!("1 item matches now"))
        } else {
            t!("{count} items match now", count = matching).into()
        };
        let can_save = !folder.name.is_empty();
        let title = if draft.id.is_empty() {
            t!("New smart folder")
        } else {
            t!("Edit smart folder")
        };
        let rows: Vec<gpui::AnyElement> = draft
            .rules
            .iter()
            .enumerate()
            .map(|(index, rule)| self.render_rule_row(index, rule, cx).into_any_element())
            .collect();
        Some(
            div()
                .id("library-smart-scrim")
                .key_context("SmartFolderDialog")
                .track_focus(&draft.focus)
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .p(px(16.))
                .occlude()
                .bg(theme.overlay)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(12.))
                        .w(px(DIALOG_WIDTH))
                        .max_w_full()
                        .p(px(20.))
                        .rounded(px(10.))
                        .border_1()
                        .border_color(theme.border)
                        .bg(theme.popover)
                        .text_color(theme.foreground)
                        .child(
                            div()
                                .text_size(px(15.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(title),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(12.))
                                .child(
                                    div()
                                        .w(px(56.))
                                        .text_size(px(13.))
                                        .text_color(theme.muted_foreground)
                                        .child(t!("Name")),
                                )
                                .child(div().flex_1().min_w_0().child(ui::TextInput::settings(
                                    &theme,
                                    "smart-name",
                                    &draft.name,
                                ))),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .text_size(px(13.))
                                .child(t!("Items that match"))
                                .child(self.matching_switch(draft.matching, cx))
                                .child(t!("of these rules")),
                        )
                        .child(div().flex().flex_col().gap(px(6.)).children(rows))
                        .child(
                            div().flex().child(
                                self.ghost_button("smart-add-rule", t!("Add rule"))
                                    .px(px(8.))
                                    .child(
                                        svg()
                                            .path("icons/plus.svg")
                                            .size(px(14.))
                                            .text_color(theme.foreground),
                                    )
                                    .child(t!("Add rule"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.add_smart_rule(window, cx);
                                    })),
                            ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .mt(px(4.))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .text_size(px(12.))
                                        .text_color(theme.muted_foreground)
                                        .child(counted),
                                )
                                .child(
                                    self.dialog_button(
                                        "smart-cancel",
                                        t!("Cancel"),
                                        ButtonTone::Plain,
                                        false,
                                        false,
                                    )
                                    .on_click(cx.listener(
                                        |this, _, _, cx| {
                                            this.close_smart_dialog(cx);
                                        },
                                    )),
                                )
                                .child(
                                    self.dialog_button(
                                        "smart-save",
                                        t!("Save"),
                                        ButtonTone::Primary,
                                        false,
                                        !can_save,
                                    )
                                    .when(can_save, |this| {
                                        this.on_click(cx.listener(|this, _, window, cx| {
                                            this.save_smart_dialog(window, cx);
                                        }))
                                    }),
                                ),
                        ),
                ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_choice_a_trigger_offers_keeps_the_rest_of_the_rule() {
        let rule = Rule::Date {
            op: DateOp::OlderThan,
            days: 30,
        };
        assert!(options(&rule, Part::Value).iter().all(|choice| matches!(
            choice,
            Rule::Date {
                op: DateOp::OlderThan,
                ..
            }
        )));
        assert!(
            options(&rule, Part::Op)
                .iter()
                .all(|choice| matches!(choice, Rule::Date { days: 30, .. }))
        );
        let fields = options(&rule, Part::Field);
        assert_eq!(fields.len(), 4);
        assert!(fields.contains(&rule));
        assert!(options(&Rule::starting(Field::Title), Part::Value).is_empty());
    }

    #[test]
    fn the_export_values_are_the_status_filters() {
        let values = options(&Rule::starting(Field::Export), Part::Value);
        assert_eq!(values.len(), StatusFilter::ALL.len());
        assert_eq!(options(&Rule::starting(Field::Kind), Part::Value).len(), 4);
    }
}
