use std::{cell::RefCell, path::Path, rc::Rc};

use gpui::{
    Bounds, Context, InteractiveElement, IntoElement, ParentElement, Pixels, Point, SharedString,
    StatefulInteractiveElement, Styled, Window, anchored, canvas, deferred, div, point,
    prelude::FluentBuilder, px, svg,
};

use super::{
    LibraryWindow, SortDirection, SortKey, Step, ViewMode, column_label, direction_label, library,
    t,
};
use crate::{i18n::tc, library::MediaKind, library::view::Entry};

const MENU_WIDTH: f32 = 230.;
const SUBMENU_WIDTH: f32 = 180.;
const ROW_HEIGHT: f32 = 32.;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Command {
    Open,
    Preview,
    OpenExternally,
    Export,
    Compress,
    SaveAsGif,
    Rename,
    CopyFile,
    CopyFiles,
    Reveal,
    Delete,
    SortBy,
    View,
    Refresh,
    OpenStorage,
    Sort(SortKey),
    Direction(SortDirection),
    ShowAs(ViewMode),
    Pin,
    AddToFolder,
    RemoveFromFolder,
    ToggleFolder(usize),
    NewFolder,
    RenameFolder,
    EditRules,
    CompressAll,
    DeleteFolder,
}

impl Command {
    fn submenu(self) -> Option<Submenu> {
        match self {
            Self::SortBy => Some(Submenu::Sort),
            Self::View => Some(Submenu::View),
            Self::AddToFolder => Some(Submenu::Folders),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct ItemActions {
    opens: bool,
    compressible: bool,
    saves_gif: bool,
    copyable: bool,
    in_folder: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Target {
    Blank,
    One(ItemActions),
    Many {
        compressible: bool,
        copyable: bool,
        in_folder: bool,
    },
    Folders(usize),
    Folder {
        smart: bool,
    },
}

impl Target {
    fn of(entry: &Entry, in_folder: bool) -> Self {
        Self::One(ItemActions {
            opens: entry.opens_editor || entry.kind == MediaKind::Instant,
            compressible: entry.compressible,
            saves_gif: library::gif::can_save_as_gif(entry),
            copyable: library::file_to_share(entry).is_some(),
            in_folder,
        })
    }
}

fn organize_commands(in_folder: bool) -> Vec<Option<Command>> {
    let mut rows = vec![Some(Command::Pin), Some(Command::AddToFolder)];
    if in_folder {
        rows.push(Some(Command::RemoveFromFolder));
    }
    rows.push(None);
    rows
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
        Target::Folders(count) => (0..count)
            .map(|index| Some(Command::ToggleFolder(index)))
            .chain((count > 0).then_some(None))
            .chain([Some(Command::NewFolder)])
            .collect(),
        Target::Folder { smart } => {
            let mut rows = vec![Some(Command::RenameFolder)];
            if smart {
                rows.push(Some(Command::EditRules));
            }
            rows.extend([
                Some(Command::CompressAll),
                None,
                Some(Command::DeleteFolder),
            ]);
            rows
        }
        Target::Many {
            compressible,
            copyable,
            in_folder,
        } => {
            let mut rows = organize_commands(in_folder);
            if compressible {
                rows.push(Some(Command::Compress));
            }
            if copyable {
                rows.push(Some(Command::CopyFiles));
            }
            rows.extend([Some(Command::Reveal), None, Some(Command::Delete)]);
            rows
        }
        Target::One(actions) => {
            let mut rows = Vec::new();
            if actions.opens {
                rows.extend([Some(Command::Open), Some(Command::Preview)]);
                if actions.copyable {
                    rows.push(Some(Command::OpenExternally));
                }
                rows.push(None);
            }
            rows.extend(organize_commands(actions.in_folder));
            if actions.opens {
                rows.push(Some(Command::Export));
                if actions.compressible {
                    rows.push(Some(Command::Compress));
                }
                if actions.saves_gif {
                    rows.push(Some(Command::SaveAsGif));
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
    Folders,
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

impl LibraryWindow {
    pub(super) fn open_library_item_menu(
        &mut self,
        path: &Path,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.change_selection(cx, |selection, _| selection.context_click(path));
        let in_folder = self.open_folder().is_some();
        let target = if self.library.selection.items().len() > 1 {
            Target::Many {
                compressible: !self.library_compressible().is_empty(),
                copyable: !self.library_files_to_share().is_empty(),
                in_folder,
            }
        } else {
            self.library
                .entry(path)
                .map_or(Target::Blank, |entry| Target::of(entry, in_folder))
        };
        self.library.context = Some(ContextMenu::new(position, target));
        cx.notify();
    }

    fn open_folder(&self) -> Option<&library::folders::Folder> {
        let crate::library::places::Place::Folder(id) = &self.library.organizer.place else {
            return None;
        };
        self.library
            .organizer
            .file
            .folders
            .iter()
            .find(|folder| &folder.id == id)
    }

    pub(super) fn open_library_folders_menu(
        &mut self,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let count = self.library.organizer.file.folders.len();
        self.library.context = Some(ContextMenu::new(position, Target::Folders(count)));
        cx.notify();
    }

    pub(super) fn open_library_folder_menu(
        &mut self,
        position: Point<Pixels>,
        smart: bool,
        cx: &mut Context<Self>,
    ) {
        self.library.context = Some(ContextMenu::new(position, Target::Folder { smart }));
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

    pub(super) fn open_library_menu_from_keys(
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
            Submenu::Folders => {
                commands(Target::Folders(self.library.organizer.file.folders.len()))
            }
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
                    self.open_library_entry(&entry, window, cx);
                }
            }
            Command::Preview => self.preview_library_lead(window, cx),
            Command::OpenExternally => self.open_library_file_externally(cx),
            Command::Export => self.export_library_selection(window, cx),
            Command::Compress => self.compress_library_selection(window, cx),
            Command::SaveAsGif => self.save_library_gif(window, cx),
            Command::Rename => self.rename_library_selection(window, cx),
            Command::CopyFile | Command::CopyFiles => self.copy_library_file(cx),
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
            Command::Pin => self.pin_library_selection(window, cx),
            Command::RemoveFromFolder => {
                if let Some(folder) = self.open_folder().map(|folder| folder.id.clone()) {
                    self.remove_selection_from_folder(folder, window, cx);
                }
            }
            Command::ToggleFolder(index) => {
                let folder = self
                    .library
                    .organizer
                    .file
                    .folders
                    .get(index)
                    .map(|folder| folder.id.clone());
                if let Some(folder) = folder {
                    self.toggle_selection_in_folder(folder, window, cx);
                }
            }
            Command::NewFolder => self.new_library_folder(true, window, cx),
            Command::RenameFolder => self.rename_focused_folder(window, cx),
            Command::EditRules => {
                if let crate::library::places::Place::Smart(id) =
                    self.library.organizer.place.clone()
                {
                    self.open_smart_dialog(Some(id), window, cx);
                }
            }
            Command::CompressAll => self.compress_library_place(cx),
            Command::DeleteFolder => self.delete_focused_folder(window, cx),
            Command::SortBy | Command::View | Command::AddToFolder => {}
        }
    }

    fn command_label(&self, command: Command) -> SharedString {
        match command {
            Command::Open => t!("Open").into(),
            Command::Preview => t!("Preview").into(),
            Command::OpenExternally => t!("Open in default app").into(),
            Command::Export => t!("Export…").into(),
            Command::Compress => t!("Compress").into(),
            Command::CompressAll => t!("Compress all").into(),
            Command::SaveAsGif => t!("Save as GIF…").into(),
            Command::Rename => t!("Rename").into(),
            Command::CopyFile => t!("Copy file").into(),
            Command::CopyFiles => t!("Copy files").into(),
            Command::Reveal => t!("Show in folder").into(),
            Command::Delete => t!("Delete…").into(),
            Command::SortBy => t!("Sort by").into(),
            Command::View => tc!("library menu", "View").into(),
            Command::Refresh => t!("Refresh").into(),
            Command::OpenStorage => t!("Open storage folder").into(),
            Command::Sort(sort) => column_label(Some(sort)).into(),
            Command::Direction(direction) => {
                direction_label(self.library.state.sort, direction).into()
            }
            Command::ShowAs(ViewMode::Grid) => t!("Grid").into(),
            Command::ShowAs(ViewMode::List) => t!("List").into(),
            Command::Pin if self.selection_pinned() => t!("Unpin").into(),
            Command::Pin => t!("Pin").into(),
            Command::AddToFolder => t!("Add to folder").into(),
            Command::RemoveFromFolder => self
                .open_folder()
                .map_or_else(SharedString::default, |folder| {
                    t!("Remove from {folder}", folder = folder.name).into()
                }),
            Command::ToggleFolder(index) => self
                .library
                .organizer
                .file
                .folders
                .get(index)
                .map_or_else(SharedString::default, |folder| folder.name.clone().into()),
            Command::NewFolder => t!("New folder…").into(),
            Command::RenameFolder => t!("Rename").into(),
            Command::EditRules => t!("Edit rules…").into(),
            Command::DeleteFolder => match self.library.organizer.place {
                crate::library::places::Place::Smart(_) => t!("Delete smart folder").into(),
                _ => t!("Delete folder").into(),
            },
        }
    }

    fn command_keys(&self, command: Command) -> Option<&'static str> {
        match command {
            Command::Open => Some("Enter"),
            Command::Preview => Some("Space"),
            Command::Rename => Some("F2"),
            Command::CopyFile | Command::CopyFiles => Some("Ctrl+C"),
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
            Command::ToggleFolder(index) => self
                .library
                .organizer
                .file
                .folders
                .get(index)
                .is_some_and(|folder| self.selection_in_folder(&folder.id)),
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
                    .aria_label(self.command_label(command))
                    .when(highlight == Some(index), |this| this.bg(theme.accent))
                    .when(
                        matches!(command, Command::Delete | Command::DeleteFolder),
                        |this| this.text_color(theme.danger_text),
                    )
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
            .occlude()
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
            saves_gif: false,
            copyable,
            in_folder: false,
        })
    }

    #[test]
    fn an_instant_recording_offers_save_as_gif_after_compress() {
        let instant = Target::One(ItemActions {
            opens: true,
            compressible: true,
            saves_gif: true,
            copyable: true,
            in_folder: false,
        });
        assert_eq!(
            listed(instant),
            [
                Command::Open,
                Command::Preview,
                Command::OpenExternally,
                Command::Pin,
                Command::AddToFolder,
                Command::Export,
                Command::Compress,
                Command::SaveAsGif,
                Command::Rename,
                Command::CopyFile,
                Command::Reveal,
                Command::Delete,
            ]
        );
        assert!(!listed(one(true, true, true)).contains(&Command::SaveAsGif));
    }

    fn recording(name: &str) -> Entry {
        Entry {
            path: std::path::PathBuf::from(format!("{name}.ever")),
            kind: MediaKind::Studio,
            name: name.to_owned(),
            size: 0,
            duration: None,
            created_millis: 0.,
            exports: Vec::new(),
            compression: crate::library::view::Compression::None,
            compressible: false,
            clip_count: 1,
            thumbnail: None,
            thumbnail_version: None,
            screenshot: None,
            opens_editor: false,
            audio_only: false,
            id: None,
            rebuilt: None,
        }
    }

    fn draw(cx: &mut gpui::VisualTestContext) {
        cx.update(|window, cx| {
            window.refresh();
            window.draw(cx).clear(cx);
        });
    }

    #[gpui::test]
    fn a_command_picked_over_blank_space_still_runs(cx: &mut gpui::TestAppContext) {
        let session = cx.update(crate::session::RecordingSession::init);
        let (library, cx) =
            cx.add_window_view(move |window, cx| LibraryWindow::new(session, window, cx));
        library.update(cx, |library, _| library.shown = true);
        let held = recording("held");
        let path = held.path.clone();
        library.update_in(cx, |library, _, cx| {
            library.library.entries = Some(vec![held]);
            library.rebuild_library(cx);
        });
        draw(cx);
        let tile = library
            .read_with(cx, |library, _| {
                library.library.drawn.borrow().get(&path).copied()
            })
            .expect("the recording is drawn");
        let blank = point(tile.left(), tile.bottom() + px(60.));
        library.update(cx, |library, cx| {
            library.open_library_item_menu(&path, blank, cx)
        });
        draw(cx);
        let panel = library
            .read_with(cx, |library, _| {
                library
                    .library
                    .context
                    .as_ref()
                    .and_then(|menu| menu.panels.borrow().first().copied())
            })
            .expect("the menu is drawn");
        let delete_row = point(panel.center().x, panel.bottom() - px(4. + ROW_HEIGHT / 2.));

        cx.simulate_click(delete_row, gpui::Modifiers::none());

        library.read_with(cx, |library, _| {
            assert!(library.library.context.is_none());
            assert!(library.library.delete.dialog_open());
        });
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
                Command::Preview,
                Command::OpenExternally,
                Command::Pin,
                Command::AddToFolder,
                Command::Export,
                Command::Rename,
                Command::CopyFile,
                Command::Reveal,
                Command::Delete,
            ]
        );
    }

    #[test]
    fn a_studio_project_with_no_export_has_no_file_to_share() {
        let rows = listed(one(true, false, false));
        assert!(!rows.contains(&Command::CopyFile));
        assert!(!rows.contains(&Command::OpenExternally));
    }

    #[test]
    fn only_a_compressible_item_offers_compress() {
        assert!(listed(one(true, true, true)).contains(&Command::Compress));
        assert!(!listed(one(true, false, true)).contains(&Command::Compress));
    }

    #[test]
    fn a_recording_still_in_progress_can_only_be_organized_named_shown_or_deleted() {
        assert_eq!(
            listed(one(false, false, false)),
            [
                Command::Pin,
                Command::AddToFolder,
                Command::Rename,
                Command::Reveal,
                Command::Delete
            ]
        );
    }

    #[test]
    fn inside_a_folder_the_menu_offers_to_take_items_out() {
        let inside = Target::One(ItemActions {
            in_folder: true,
            ..ItemActions::default()
        });
        assert_eq!(
            listed(inside)[..3],
            [
                Command::Pin,
                Command::AddToFolder,
                Command::RemoveFromFolder
            ]
        );
        assert!(
            listed(Target::Many {
                compressible: false,
                copyable: false,
                in_folder: true,
            })
            .contains(&Command::RemoveFromFolder)
        );
    }

    #[test]
    fn the_folder_list_ends_with_new_folder_and_a_folder_has_its_own_menu() {
        assert_eq!(
            commands(Target::Folders(2)),
            [
                Some(Command::ToggleFolder(0)),
                Some(Command::ToggleFolder(1)),
                None,
                Some(Command::NewFolder)
            ]
        );
        assert_eq!(commands(Target::Folders(0)), [Some(Command::NewFolder)]);
        assert_eq!(
            listed(Target::Folder { smart: true }),
            [
                Command::RenameFolder,
                Command::EditRules,
                Command::CompressAll,
                Command::DeleteFolder
            ]
        );
        assert_eq!(
            listed(Target::Folder { smart: false }),
            [
                Command::RenameFolder,
                Command::CompressAll,
                Command::DeleteFolder
            ]
        );
    }

    #[test]
    fn several_items_and_blank_space_have_their_own_menus() {
        assert_eq!(
            listed(Target::Many {
                compressible: false,
                copyable: false,
                in_folder: false,
            }),
            [
                Command::Pin,
                Command::AddToFolder,
                Command::Reveal,
                Command::Delete
            ]
        );
        assert_eq!(
            listed(Target::Many {
                compressible: true,
                copyable: true,
                in_folder: false,
            }),
            [
                Command::Pin,
                Command::AddToFolder,
                Command::Compress,
                Command::CopyFiles,
                Command::Reveal,
                Command::Delete
            ]
        );
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
