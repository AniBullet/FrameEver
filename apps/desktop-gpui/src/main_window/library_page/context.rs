use std::{cell::RefCell, path::Path, rc::Rc};

use gpui::{
    Bounds, Context, InteractiveElement, IntoElement, ParentElement, Pixels, Point,
    StatefulInteractiveElement, Styled, Window, anchored, canvas, deferred, div, point,
    prelude::FluentBuilder, px, svg,
};

use super::{
    MainWindow, SortDirection, SortKey, Step, ViewMode, column_label, direction_label, library,
    open_entry, t,
};
use crate::{i18n::tc, library::MediaKind, library::view::Entry};

const MENU_WIDTH: f32 = 230.;
const SUBMENU_WIDTH: f32 = 180.;
const ROW_HEIGHT: f32 = 32.;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Command {
    Open,
    Export,
    Compress,
    Rename,
    CopyFile,
    Reveal,
    Delete,
    SortBy,
    View,
    Refresh,
    OpenStorage,
    Sort(SortKey),
    Direction(SortDirection),
    ShowAs(ViewMode),
}

impl Command {
    fn submenu(self) -> Option<Submenu> {
        match self {
            Self::SortBy => Some(Submenu::Sort),
            Self::View => Some(Submenu::View),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct ItemActions {
    opens: bool,
    compressible: bool,
    copyable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Target {
    Blank,
    One(ItemActions),
    Many,
}

impl Target {
    fn of(entry: &Entry) -> Self {
        Self::One(ItemActions {
            opens: entry.opens_editor || entry.kind == MediaKind::Instant,
            compressible: entry.compressible,
            copyable: library::file_to_copy(entry).is_some(),
        })
    }
}

pub(super) fn commands(target: Target) -> Vec<Option<Command>> {
    match target {
        Target::Blank => vec![
            Some(Command::SortBy),
            Some(Command::View),
            Some(Command::Refresh),
            None,
            Some(Command::OpenStorage),
        ],
        Target::Many => vec![Some(Command::Reveal), None, Some(Command::Delete)],
        Target::One(actions) => {
            let mut rows = Vec::new();
            if actions.opens {
                rows.extend([Some(Command::Open), None, Some(Command::Export)]);
                if actions.compressible {
                    rows.push(Some(Command::Compress));
                }
                rows.push(None);
            }
            rows.push(Some(Command::Rename));
            if actions.copyable {
                rows.push(Some(Command::CopyFile));
            }
            rows.extend([Some(Command::Reveal), None, Some(Command::Delete)]);
            rows
        }
    }
}

fn sort_commands(sort: SortKey) -> Vec<Option<Command>> {
    let first = sort.first_direction();
    SortKey::ALL
        .into_iter()
        .map(|key| Some(Command::Sort(key)))
        .chain([
            None,
            Some(Command::Direction(first)),
            Some(Command::Direction(first.reversed())),
        ])
        .collect()
}

fn view_commands() -> Vec<Option<Command>> {
    vec![
        Some(Command::ShowAs(ViewMode::Grid)),
        Some(Command::ShowAs(ViewMode::List)),
    ]
}

fn next_row(rows: &[Option<Command>], from: Option<usize>, forward: bool) -> Option<usize> {
    let count = rows.len();
    if count == 0 {
        return None;
    }
    let mut at = from.unwrap_or(if forward { count - 1 } else { 0 });
    for _ in 0..count {
        at = if forward {
            (at + 1) % count
        } else {
            (at + count - 1) % count
        };
        if rows[at].is_some() {
            return Some(at);
        }
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Submenu {
    Sort,
    View,
}

pub(super) struct ContextMenu {
    position: Point<Pixels>,
    rows: Vec<Option<Command>>,
    highlight: Option<usize>,
    submenu: Option<(Submenu, Option<usize>)>,
    panels: Rc<RefCell<Vec<Bounds<Pixels>>>>,
}

impl ContextMenu {
    fn new(position: Point<Pixels>, target: Target) -> Self {
        Self {
            position,
            rows: commands(target),
            highlight: None,
            submenu: None,
            panels: Rc::default(),
        }
    }
}

impl MainWindow {
    pub(super) fn open_library_item_menu(
        &mut self,
        path: &Path,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.change_selection(cx, |selection, _| selection.context_click(path));
        let target = if self.library.selection.items().len() > 1 {
            Target::Many
        } else {
            self.library.entry(path).map_or(Target::Blank, Target::of)
        };
        self.library.context = Some(ContextMenu::new(position, target));
        cx.notify();
    }

    pub(super) fn open_library_blank_menu(
        &mut self,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.library.context = Some(ContextMenu::new(position, Target::Blank));
        cx.notify();
    }

    pub(in crate::main_window) fn open_library_menu_from_keys(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let lead = self.library.selection.lead().map(Path::to_path_buf);
        let drawn = lead
            .as_ref()
            .and_then(|lead| self.library.drawn.borrow().get(lead).copied());
        match (lead, drawn) {
            (Some(lead), Some(bounds)) => {
                self.open_library_item_menu(&lead, bounds.center(), cx);
            }
            _ => self.open_library_blank_menu(window.mouse_position(), cx),
        }
        if let Some(menu) = &mut self.library.context {
            menu.highlight = next_row(&menu.rows, None, true);
        }
    }

    fn submenu_rows(&self, submenu: Submenu) -> Vec<Option<Command>> {
        match submenu {
            Submenu::Sort => sort_commands(self.library.state.sort),
            Submenu::View => view_commands(),
        }
    }

    pub(super) fn step_library_context(&mut self, step: Step, cx: &mut Context<Self>) {
        let Some(open) = self.library.context.as_ref().map(|menu| menu.submenu) else {
            return;
        };
        let sub_rows = open.map(|(submenu, _)| self.submenu_rows(submenu));
        let Some(menu) = &mut self.library.context else {
            return;
        };
        match (step, &mut menu.submenu, sub_rows) {
            (Step::Up | Step::Down, Some((_, highlight)), Some(rows)) => {
                *highlight = next_row(&rows, *highlight, step == Step::Down);
            }
            (Step::Up | Step::Down, None, _) => {
                menu.highlight = next_row(&menu.rows, menu.highlight, step == Step::Down);
            }
            (Step::Left, Some(_), _) => menu.submenu = None,
            (Step::Right, None, _) => {
                let submenu = menu
                    .highlight
                    .and_then(|row| menu.rows[row])
                    .and_then(Command::submenu);
                menu.submenu = submenu.map(|submenu| (submenu, Some(0)));
            }
            _ => {}
        }
        cx.notify();
    }

    pub(super) fn run_highlighted_context(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(open) = self
            .library
            .context
            .as_ref()
            .map(|menu| (menu.submenu, menu.highlight.and_then(|row| menu.rows[row])))
        else {
            return;
        };
        let command = match open {
            (Some((submenu, highlight)), _) => highlight
                .and_then(|row| self.submenu_rows(submenu).get(row).copied())
                .flatten(),
            (None, command) => command,
        };
        if let Some(command) = command {
            self.run_library_command(command, window, cx);
        }
    }

    fn run_library_command(
        &mut self,
        command: Command,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(submenu) = command.submenu() {
            if let Some(menu) = &mut self.library.context {
                menu.submenu = Some((submenu, None));
            }
            cx.notify();
            return;
        }
        self.close_library_menu(cx);
        match command {
            Command::Open => {
                if let Some(entry) = self.library.lead_entry().cloned() {
                    open_entry(&entry, cx);
                }
            }
            Command::Export => self.export_library_selection(window, cx),
            Command::Compress => self.compress_library_selection(window, cx),
            Command::Rename => self.rename_library_selection(window, cx),
            Command::CopyFile => self.copy_library_file(cx),
            Command::Reveal => self.reveal_library_selection(cx),
            Command::Delete => self.delete_library_selection(window, cx),
            Command::Refresh => self.refresh_library(window, cx),
            Command::OpenStorage => {
                library::open_with_system(&library::storage().storage_dir(), cx);
            }
            Command::Sort(sort) => self.change_library_view(cx, |state| state.set_sort(sort)),
            Command::Direction(direction) => {
                self.change_library_view(cx, |state| state.direction = direction);
            }
            Command::ShowAs(mode) => self.show_library_as(mode, cx),
            Command::SortBy | Command::View => {}
        }
    }

    fn command_label(&self, command: Command) -> &'static str {
        match command {
            Command::Open => t!("Open"),
            Command::Export => t!("Export…"),
            Command::Compress => t!("Compress"),
            Command::Rename => t!("Rename"),
            Command::CopyFile => t!("Copy file"),
            Command::Reveal => t!("Show in folder"),
            Command::Delete => t!("Delete…"),
            Command::SortBy => t!("Sort by"),
            Command::View => tc!("library menu", "View"),
            Command::Refresh => t!("Refresh"),
            Command::OpenStorage => t!("Open storage folder"),
            Command::Sort(sort) => column_label(Some(sort)),
            Command::Direction(direction) => direction_label(self.library.state.sort, direction),
            Command::ShowAs(ViewMode::Grid) => t!("Grid"),
            Command::ShowAs(ViewMode::List) => t!("List"),
        }
    }

    fn command_keys(&self, command: Command) -> Option<&'static str> {
        match command {
            Command::Open => Some("Enter"),
            Command::Rename => Some("F2"),
            Command::CopyFile => Some("Ctrl+C"),
            Command::Delete => Some("Del"),
            _ => None,
        }
    }

    fn command_ticked(&self, command: Command) -> bool {
        let state = &self.library.state;
        match command {
            Command::Sort(sort) => sort == state.sort,
            Command::Direction(direction) => direction == state.direction,
            Command::ShowAs(mode) => mode == state.view,
            _ => false,
        }
    }

    fn render_command_rows(
        &self,
        rows: &[Option<Command>],
        highlight: Option<usize>,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::AnyElement> {
        let theme = self.theme;
        rows.iter()
            .enumerate()
            .map(|(index, row)| {
                let Some(command) = *row else {
                    return div()
                        .h(px(1.))
                        .my(px(4.))
                        .bg(theme.border)
                        .into_any_element();
                };
                let opens_submenu = command.submenu().is_some();
                let ticked = self.command_ticked(command);
                self.popup_row(("library-context-row", index))
                    .when(highlight == Some(index), |this| this.bg(theme.accent))
                    .when(command == Command::Delete, |this| {
                        this.text_color(theme.danger_text)
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(self.command_label(command)),
                    )
                    .children(self.command_keys(command).map(|keys| {
                        div()
                            .text_size(px(11.))
                            .text_color(theme.muted_foreground)
                            .child(keys)
                    }))
                    .when(opens_submenu, |this| {
                        this.child(
                            svg()
                                .path("icons/chevron-right.svg")
                                .size(px(14.))
                                .text_color(theme.muted_foreground),
                        )
                    })
                    .when(ticked, |this| this.child(self.popup_tick(true)))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.run_library_command(command, window, cx);
                    }))
                    .into_any_element()
            })
            .collect()
    }

    fn context_panel(&self, width: f32, cx: &mut Context<Self>) -> gpui::Div {
        let theme = self.theme;
        let panels = self
            .library
            .context
            .as_ref()
            .map(|menu| menu.panels.clone())
            .unwrap_or_default();
        div()
            .relative()
            .w(px(width))
            .p(px(4.))
            .flex()
            .flex_col()
            .rounded(px(8.))
            .border_1()
            .border_color(theme.border)
            .bg(theme.popover)
            .text_size(px(12.))
            .text_color(theme.popover_foreground)
            .child(
                canvas(
                    move |bounds, _, _| panels.borrow_mut().push(bounds),
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .on_mouse_down_out(cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                let inside = this.library.context.as_ref().is_some_and(|menu| {
                    menu.panels
                        .borrow()
                        .iter()
                        .any(|panel| panel.contains(&event.position))
                });
                if !inside {
                    this.close_library_menu(cx);
                }
            }))
    }

    pub(super) fn render_library_context(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let menu = self.library.context.as_ref()?;
        menu.panels.borrow_mut().clear();
        let rows = self.render_command_rows(&menu.rows, menu.highlight, cx);
        let main = anchored()
            .position(menu.position)
            .snap_to_window_with_margin(px(8.))
            .child(self.context_panel(MENU_WIDTH, cx).children(rows));
        let sub = menu.submenu.map(|(submenu, highlight)| {
            let opener = menu
                .rows
                .iter()
                .position(|row| row.and_then(Command::submenu) == Some(submenu));
            let rows = self.render_command_rows(&self.submenu_rows(submenu), highlight, cx);
            anchored()
                .position(
                    menu.position
                        + point(
                            px(MENU_WIDTH - 4.),
                            px(opener.unwrap_or(0) as f32 * ROW_HEIGHT),
                        ),
                )
                .snap_to_window_with_margin(px(8.))
                .child(self.context_panel(SUBMENU_WIDTH, cx).children(rows))
        });
        Some(deferred(div().child(main).children(sub)).with_priority(2))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn one(opens: bool, compressible: bool, copyable: bool) -> Target {
        Target::One(ItemActions {
            opens,
            compressible,
            copyable,
        })
    }

    fn listed(target: Target) -> Vec<Command> {
        commands(target).into_iter().flatten().collect()
    }

    #[test]
    fn a_studio_project_offers_open_and_export_but_not_compress() {
        let rows = listed(one(true, false, true));
        assert_eq!(
            rows,
            [
                Command::Open,
                Command::Export,
                Command::Rename,
                Command::CopyFile,
                Command::Reveal,
                Command::Delete,
            ]
        );
    }

    #[test]
    fn a_studio_project_with_no_export_has_no_file_to_copy() {
        assert!(!listed(one(true, false, false)).contains(&Command::CopyFile));
    }

    #[test]
    fn only_a_compressible_item_offers_compress() {
        assert!(listed(one(true, true, true)).contains(&Command::Compress));
        assert!(!listed(one(true, false, true)).contains(&Command::Compress));
    }

    #[test]
    fn a_recording_still_in_progress_can_only_be_named_shown_or_deleted() {
        assert_eq!(
            listed(one(false, false, false)),
            [Command::Rename, Command::Reveal, Command::Delete]
        );
    }

    #[test]
    fn several_items_and_blank_space_have_their_own_menus() {
        assert_eq!(listed(Target::Many), [Command::Reveal, Command::Delete]);
        assert_eq!(
            listed(Target::Blank),
            [
                Command::SortBy,
                Command::View,
                Command::Refresh,
                Command::OpenStorage,
            ]
        );
    }

    #[test]
    fn the_keyboard_skips_separators_and_wraps() {
        let rows = commands(Target::Blank);
        assert_eq!(next_row(&rows, None, true), Some(0));
        assert_eq!(next_row(&rows, None, false), Some(4));
        assert_eq!(next_row(&rows, Some(2), true), Some(4));
        assert_eq!(next_row(&rows, Some(4), true), Some(0));
        assert_eq!(next_row(&rows, Some(4), false), Some(2));
    }
}
