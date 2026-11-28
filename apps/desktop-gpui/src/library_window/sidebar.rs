use std::{collections::HashMap, path::PathBuf};

use chrono::NaiveDate;
use gpui::{
    Context, Entity, ExternalPaths, FocusHandle, FontWeight, InteractiveElement, IntoElement,
    MouseButton, MouseDownEvent, ParentElement, SharedString, StatefulInteractiveElement, Styled,
    Task, Window, div, prelude::FluentBuilder, px, svg,
};

use super::{LibraryWindow, drag_out::DraggedItem};
use crate::{
    i18n::t,
    library::{
        self,
        folders::{self, Folder, LibraryFile, LoadOutcome, UpdateError},
        places::{Place, Places},
        rules::SmartFolder,
        view::Clock,
        visuals::Visual,
    },
    main_window::focus_ring,
    ui,
};

pub(super) const SIDEBAR_WIDTH: f32 = 196.;
pub(super) const NARROW_WIDTH: f32 = 1100.;
const ROW_HEIGHT: f32 = 28.;
const DAY_CHECK: std::time::Duration = std::time::Duration::from_secs(60);
const FOLDER_UNDO_HOLD: std::time::Duration = std::time::Duration::from_secs(6);
const TOAST_WIDTH: f32 = 340.;
const TOAST_BOTTOM: f32 = 44.;

gpui::actions!(library_sidebar, [ToggleSidebar, RenameFolder, DeleteFolder]);

pub fn bind_keys(cx: &mut gpui::App) {
    cx.bind_keys([gpui::KeyBinding::new(
        "ctrl-\\",
        ToggleSidebar,
        Some("LibraryWindow"),
    )]);
    let sidebar = Some("LibrarySidebar");
    cx.bind_keys([
        gpui::KeyBinding::new("f2", RenameFolder, sidebar),
        gpui::KeyBinding::new("delete", DeleteFolder, sidebar),
    ]);
}

pub(super) enum Removed {
    Folder(usize, Folder),
    Smart(usize, SmartFolder),
}

pub(super) struct FolderToast {
    removed: Removed,
    _expires: Task<()>,
}

pub(super) enum After {
    Stay,
    Show(Place),
    Rename(String),
}

pub(super) struct Organizer {
    pub(super) file: LibraryFile,
    pub(super) places: Places,
    pub(super) place: Place,
    pub(super) renaming: Option<String>,
    pub(super) rename_input: Entity<ui::TextInputState>,
    pub(super) narrow: bool,
    pub(super) narrow_shown: bool,
    pub(super) focus: FocusHandle,
    pub(super) removed: Option<FolderToast>,
    pub(super) day: NaiveDate,
    pub(super) visuals: HashMap<PathBuf, Visual>,
    pub(super) visuals_task: Option<Task<()>>,
    pub(super) dragging_out: bool,
}

impl Organizer {
    pub(super) fn new(rename_input: Entity<ui::TextInputState>, focus: FocusHandle) -> Self {
        Self {
            file: LibraryFile::default(),
            places: Places::default(),
            place: Place::All,
            renaming: None,
            rename_input,
            narrow: false,
            narrow_shown: false,
            focus: focus.tab_index(0).tab_stop(true),
            removed: None,
            day: Clock::now().today,
            visuals: HashMap::new(),
            visuals_task: None,
            dragging_out: false,
        }
    }

    pub(super) fn shows_sidebar(&self, hidden: bool) -> bool {
        sidebar_shown(self.narrow, self.narrow_shown, hidden)
    }

    pub(super) fn place_name(&self, place: &Place) -> Option<SharedString> {
        match place {
            Place::All => None,
            Place::Pinned => Some(t!("Pinned").into()),
            Place::Folder(id) => self
                .file
                .folders
                .iter()
                .find(|folder| &folder.id == id)
                .map(|folder| folder.name.clone().into()),
            Place::Smart(id) => self
                .file
                .smart_folders
                .iter()
                .find(|folder| &folder.id == id)
                .map(|folder| folder.name.clone().into()),
        }
    }
}

fn sidebar_shown(narrow: bool, narrow_shown: bool, hidden: bool) -> bool {
    if narrow { narrow_shown } else { !hidden }
}

pub(super) fn read_library_file() -> LibraryFile {
    let loaded = folders::load(&library::storage().library_file());
    if let LoadOutcome::MovedAside(aside) = &loaded.outcome {
        tracing::warn!(path = %aside.display(), "moved an unreadable library file aside");
    }
    loaded.file
}

type Learned = Vec<(PathBuf, String, Option<PathBuf>)>;

fn organize_on_disk(
    file_path: &std::path::Path,
    known: Vec<(PathBuf, Option<String>)>,
    edit: impl FnOnce(&mut LibraryFile, &[String]) -> After,
) -> (Learned, Result<LibraryFile, UpdateError>, After) {
    let mut learned = Vec::new();
    let mut after = After::Stay;
    let saved = folders::update(file_path, |file| {
        let mut ids = Vec::new();
        for (path, id) in known {
            match id {
                Some(id) => ids.push(id),
                None => match frameever_utils::storage::ensure_project_id(&path) {
                    Ok(id) => {
                        let rebuilt = frameever_utils::storage::kept_copy(&path);
                        learned.push((path, id.clone(), rebuilt));
                        ids.push(id);
                    }
                    Err(error) => {
                        tracing::warn!(path = %path.display(), %error, "could not give a project an id");
                    }
                },
            }
        }
        after = edit(file, &ids);
    });
    (learned, saved, after)
}

fn save_failed(error: &UpdateError) -> (&'static str, &'static str) {
    match error {
        UpdateError::ReadOnly => (
            t!("Couldn't change library folders"),
            t!("A newer FrameEver saved them. Update FrameEver to change them here."),
        ),
        UpdateError::Io(_) => (
            t!("Couldn't change library folders"),
            t!("The storage folder may be read-only. Try again."),
        ),
    }
}

impl LibraryWindow {
    pub(super) fn toggle_library_sidebar(&mut self, cx: &mut Context<Self>) {
        let organizer = &mut self.library.organizer;
        if organizer.narrow {
            organizer.narrow_shown = !organizer.narrow_shown;
            self.library.layout_stale = true;
            cx.notify();
        } else {
            self.change_library_view(cx, |state| state.sidebar_hidden = !state.sidebar_hidden);
        }
    }

    pub(super) fn sync_sidebar_width(&mut self, width: f32) {
        let narrow = width < NARROW_WIDTH;
        let organizer = &mut self.library.organizer;
        if organizer.narrow != narrow {
            organizer.narrow = narrow;
            organizer.narrow_shown = false;
            self.library.layout_stale = true;
        }
    }

    pub(super) fn sidebar_width(&self) -> f32 {
        if self
            .library
            .organizer
            .shows_sidebar(self.library.state.sidebar_hidden)
        {
            SIDEBAR_WIDTH
        } else {
            0.
        }
    }

    pub(super) fn watch_library_day(window: &mut Window, cx: &mut Context<Self>) -> Task<()> {
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(DAY_CHECK).await;
                let changed = this.update(cx, |this, cx| {
                    let today = Clock::now().today;
                    if this.library.organizer.day != today {
                        this.library.organizer.day = today;
                        this.rebuild_library(cx);
                    }
                });
                if changed.is_err() {
                    return;
                }
            }
        })
    }

    pub(super) fn show_library_place(&mut self, place: Place, cx: &mut Context<Self>) {
        if self.library.organizer.place == place {
            return;
        }
        self.library.organizer.place = place;
        self.library.selection.clear();
        self.library.list.scroll_to(gpui::ListOffset::default());
        self.rebuild_library(cx);
    }

    fn organize(
        &mut self,
        paths: Vec<PathBuf>,
        edit: impl FnOnce(&mut LibraryFile, &[String]) -> After + Send + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let known: Vec<(PathBuf, Option<String>)> = paths
            .into_iter()
            .map(|path| {
                let id = self.library.entry(&path).and_then(|entry| entry.id.clone());
                (path, id)
            })
            .collect();
        let file_path = library::storage().library_file();
        cx.spawn_in(window, async move |this, cx| {
            let (learned, saved, after) = cx
                .background_executor()
                .spawn(async move { organize_on_disk(&file_path, known, edit) })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.organized(learned, saved, after, window, cx);
            })
            .ok();
        })
        .detach();
    }

    fn organized(
        &mut self,
        learned: Learned,
        saved: Result<LibraryFile, UpdateError>,
        after: After,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(entries) = &mut self.library.entries {
            for (path, id, rebuilt) in learned {
                if let Some(entry) = entries.iter_mut().find(|entry| entry.path == path) {
                    entry.id = Some(id);
                    entry.rebuilt = rebuilt;
                }
            }
        }
        match saved {
            Ok(file) => self.library.organizer.file = file,
            Err(error) => {
                tracing::warn!(?error, "saving the library file failed");
                let (title, message) = save_failed(&error);
                cx.spawn(async move |_, _| crate::platform::alert_dialog(title, message))
                    .detach();
            }
        }
        match after {
            After::Stay => {}
            After::Show(place) => self.library.organizer.place = place,
            After::Rename(id) => {
                self.library.organizer.place = Place::Folder(id.clone());
                self.library.selection.clear();
                self.start_folder_rename(id, window, cx);
            }
        }
        self.rebuild_library(cx);
    }

    pub(super) fn pin_library_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = self.library.selection.items().to_vec();
        self.organize(
            paths,
            |file, ids| {
                file.toggle_pins(ids);
                After::Stay
            },
            window,
            cx,
        );
    }

    pub(super) fn selection_pinned(&self) -> bool {
        let file = &self.library.organizer.file;
        let entries = self.library.selected_entries();
        !entries.is_empty()
            && entries
                .iter()
                .all(|entry| entry.id.as_deref().is_some_and(|id| file.is_pinned(id)))
    }

    pub(super) fn selection_in_folder(&self, folder: &str) -> bool {
        let Some(held) = self
            .library
            .organizer
            .file
            .folders
            .iter()
            .find(|held| held.id == folder)
        else {
            return false;
        };
        let entries = self.library.selected_entries();
        !entries.is_empty()
            && entries.iter().all(|entry| {
                entry
                    .id
                    .as_ref()
                    .is_some_and(|id| held.members.contains(id))
            })
    }

    pub(super) fn toggle_selection_in_folder(
        &mut self,
        folder: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let paths = self.library.selection.items().to_vec();
        self.organize(
            paths,
            move |file, ids| {
                file.toggle_members(&folder, ids);
                After::Stay
            },
            window,
            cx,
        );
    }

    pub(super) fn remove_selection_from_folder(
        &mut self,
        folder: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let paths = self.library.selection.items().to_vec();
        self.remove_from_folder(paths, folder, window, cx);
    }

    pub(super) fn remove_from_folder(
        &mut self,
        paths: Vec<PathBuf>,
        folder: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.organize(
            paths,
            move |file, ids| {
                file.remove_members(&folder, ids);
                After::Stay
            },
            window,
            cx,
        );
    }

    pub(super) fn add_selection_to_folder(
        &mut self,
        folder: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let paths = self.library.selection.items().to_vec();
        self.organize(
            paths,
            move |file, ids| {
                let missing: Vec<String> = ids
                    .iter()
                    .filter(|id| !file.holding(id).contains(&folder.as_str()))
                    .cloned()
                    .collect();
                file.toggle_members(&folder, &missing);
                After::Stay
            },
            window,
            cx,
        );
    }

    pub(super) fn new_library_folder(
        &mut self,
        with_selection: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let paths = if with_selection {
            self.library.selection.items().to_vec()
        } else {
            Vec::new()
        };
        let name = t!("Untitled folder");
        self.organize(
            paths,
            move |file, ids| After::Rename(file.new_folder(name, ids)),
            window,
            cx,
        );
    }

    pub(super) fn start_folder_rename(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let file = &self.library.organizer.file;
        let Some(name) = file
            .folders
            .iter()
            .map(|folder| (&folder.id, &folder.name))
            .chain(
                file.smart_folders
                    .iter()
                    .map(|folder| (&folder.id, &folder.name)),
            )
            .find_map(|(held, name)| (*held == id).then(|| name.clone()))
        else {
            return;
        };
        if self.library.organizer.renaming.as_ref() == Some(&id) {
            return;
        }
        if self.library.organizer.narrow {
            self.library.organizer.narrow_shown = true;
            self.library.layout_stale = true;
        } else if self.library.state.sidebar_hidden {
            self.change_library_view(cx, |state| state.sidebar_hidden = false);
        }
        self.library.organizer.renaming = Some(id);
        self.library.organizer.rename_input.update(cx, |input, cx| {
            input.set_text(name, cx);
            input.focus_and_select_all(window, cx);
        });
        cx.notify();
    }

    pub(super) fn rename_focused_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Place::Folder(id) | Place::Smart(id) = self.library.organizer.place.clone() {
            self.start_folder_rename(id, window, cx);
        }
    }

    pub(super) fn on_folder_name_event(
        &mut self,
        input: &Entity<ui::TextInputState>,
        event: &ui::TextInputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            ui::TextInputEvent::Confirmed | ui::TextInputEvent::Blurred => {
                let Some(id) = self.library.organizer.renaming.take() else {
                    return;
                };
                let name = input.read(cx).text().trim().to_string();
                if !name.is_empty() {
                    self.organize(
                        Vec::new(),
                        move |file, _| {
                            file.rename_folder(&id, &name);
                            After::Stay
                        },
                        window,
                        cx,
                    );
                }
                window.focus(&self.library.organizer.focus, cx);
                cx.notify();
            }
            ui::TextInputEvent::Cancelled => {
                self.library.organizer.renaming = None;
                window.focus(&self.library.organizer.focus, cx);
                cx.notify();
            }
            ui::TextInputEvent::Changed => {}
        }
    }

    pub(super) fn delete_folder(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let file = &self.library.organizer.file;
        let removed = file
            .folders
            .iter()
            .position(|folder| folder.id == id)
            .map(|index| Removed::Folder(index, file.folders[index].clone()))
            .or_else(|| {
                file.smart_folders
                    .iter()
                    .position(|folder| folder.id == id)
                    .map(|index| Removed::Smart(index, file.smart_folders[index].clone()))
            });
        let Some(removed) = removed else {
            return;
        };
        let expires = cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(FOLDER_UNDO_HOLD).await;
            this.update(cx, |this, cx| {
                this.library.organizer.removed = None;
                cx.notify();
            })
            .ok();
        });
        self.library.organizer.removed = Some(FolderToast {
            removed,
            _expires: expires,
        });
        self.library.organizer.place = Place::All;
        self.organize(
            Vec::new(),
            move |file, _| {
                file.delete(&id);
                After::Stay
            },
            window,
            cx,
        );
    }

    pub(super) fn delete_focused_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Place::Folder(id) | Place::Smart(id) = self.library.organizer.place.clone() {
            self.delete_folder(id, window, cx);
        }
    }

    pub(super) fn undo_folder_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(FolderToast { removed, .. }) = self.library.organizer.removed.take() else {
            return;
        };
        self.organize(
            Vec::new(),
            move |file, _| match removed {
                Removed::Folder(index, folder) => {
                    let place = Place::Folder(folder.id.clone());
                    file.folders.insert(index.min(file.folders.len()), folder);
                    After::Show(place)
                }
                Removed::Smart(index, folder) => {
                    let place = Place::Smart(folder.id.clone());
                    file.smart_folders
                        .insert(index.min(file.smart_folders.len()), folder);
                    After::Show(place)
                }
            },
            window,
            cx,
        );
    }

    pub(super) fn save_smart_folder(
        &mut self,
        folder: SmartFolder,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.organize(
            Vec::new(),
            move |file, _| After::Show(Place::Smart(file.save_smart(folder))),
            window,
            cx,
        );
    }

    fn drop_on_folder(&mut self, folder: String, window: &mut Window, cx: &mut Context<Self>) {
        self.library.organizer.dragging_out = false;
        self.add_selection_to_folder(folder, window, cx);
    }

    fn sidebar_row(
        &self,
        id: impl Into<gpui::ElementId>,
        place: Place,
        icon: &'static str,
        label: SharedString,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let theme = self.theme;
        let on = self.library.organizer.place == place;
        let count = self
            .library
            .organizer
            .places
            .count(&place, self.library.view.total);
        let clicked = place.clone();
        div()
            .id(id.into())
            .role(gpui::Role::Button)
            .aria_label(label.clone())
            .aria_selected(on)
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap(px(8.))
            .h(px(ROW_HEIGHT))
            .px(px(8.))
            .rounded(px(4.))
            .cursor_pointer()
            .text_size(px(13.))
            .when(on, |this| {
                this.bg(theme.selection)
                    .text_color(theme.selection_foreground)
            })
            .when(!on, |this| {
                this.text_color(theme.foreground)
                    .hover(move |style| style.bg(theme.accent))
            })
            .child(
                svg()
                    .path(icon)
                    .size(px(14.))
                    .flex_shrink_0()
                    .text_color(if on {
                        theme.selection_foreground
                    } else {
                        theme.muted_foreground
                    }),
            )
            .child(div().flex_1().min_w_0().truncate().child(label))
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(theme.muted_foreground)
                    .child(count.to_string()),
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                window.focus(&this.library.organizer.focus, cx);
                this.show_library_place(clicked.clone(), cx);
            }))
    }

    fn folder_row(
        &self,
        index: usize,
        id: &str,
        name: &str,
        smart: bool,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let theme = self.theme;
        let icon = if smart {
            "icons/folder-search.svg"
        } else {
            "icons/folder.svg"
        };
        if self.library.organizer.renaming.as_deref() == Some(id) {
            return div()
                .flex()
                .flex_shrink_0()
                .items_center()
                .gap(px(8.))
                .h(px(ROW_HEIGHT))
                .px(px(8.))
                .rounded(px(4.))
                .border_1()
                .border_color(theme.ring)
                .bg(theme.background)
                .child(
                    svg()
                        .path(icon)
                        .size(px(14.))
                        .flex_shrink_0()
                        .text_color(theme.muted_foreground),
                )
                .child(
                    div().flex_1().min_w_0().child(
                        ui::TextInput::bare(
                            &theme,
                            ("library-folder-name", index),
                            &self.library.organizer.rename_input,
                        )
                        .height(px(22.))
                        .text_size(px(13.)),
                    ),
                )
                .into_any_element();
        }
        let place = if smart {
            Place::Smart(id.to_owned())
        } else {
            Place::Folder(id.to_owned())
        };
        let element_id = if smart {
            ("library-smart-row", index)
        } else {
            ("library-folder-row", index)
        };
        let (menu_place, dropped, external) = (place.clone(), id.to_owned(), id.to_owned());
        self.sidebar_row(element_id, place, icon, name.to_owned().into(), cx)
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    window.focus(&this.library.organizer.focus, cx);
                    this.show_library_place(menu_place.clone(), cx);
                    this.open_library_folder_menu(event.position, smart, cx);
                }),
            )
            .when(!smart, |this| {
                this.drag_over::<DraggedItem>(move |style, _, _, _| style.bg(theme.accent))
                    .on_drop(cx.listener(move |this, _: &DraggedItem, window, cx| {
                        this.drop_on_folder(dropped.clone(), window, cx);
                    }))
                    .on_drop(cx.listener(move |this, _: &ExternalPaths, window, cx| {
                        if this.library.organizer.dragging_out {
                            this.drop_on_folder(external.clone(), window, cx);
                        }
                    }))
            })
            .into_any_element()
    }

    fn sidebar_head(
        &self,
        label: &'static str,
        id: &'static str,
        tooltip: &'static str,
    ) -> (gpui::Div, gpui::Stateful<gpui::Div>) {
        let theme = self.theme;
        let button = focus_ring(
            div()
                .id(id)
                .role(gpui::Role::Button)
                .aria_label(tooltip)
                .accessibility_id(id)
                .tab_index(0)
                .flex()
                .items_center()
                .justify_center()
                .size(px(22.))
                .rounded(px(4.))
                .cursor_pointer()
                .hover(move |style| style.bg(theme.accent))
                .tooltip(move |_, cx| ui::Tooltip::new(&theme, tooltip).view(cx))
                .tooltip_show_delay(ui::TOOLTIP_SHOW_DELAY)
                .child(
                    svg()
                        .path("icons/plus.svg")
                        .size(px(14.))
                        .text_color(theme.muted_foreground),
                ),
            theme,
        );
        let head = div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .mt(px(12.))
            .pl(px(8.))
            .pr(px(2.))
            .pb(px(2.))
            .text_size(px(12.))
            .text_color(theme.muted_foreground)
            .child(div().flex_1().child(label));
        (head, button)
    }

    pub(super) fn render_library_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let organizer = &self.library.organizer;
        let file = &organizer.file;
        let shows_pinned =
            organizer.places.count(&Place::Pinned, 0) > 0 || organizer.place == Place::Pinned;
        let (folders_head, new_folder) =
            self.sidebar_head(t!("Folders"), "library-new-folder", t!("New folder"));
        let (smart_head, new_smart) = self.sidebar_head(
            t!("Smart folders"),
            "library-new-smart",
            t!("New smart folder…"),
        );
        let folders: Vec<gpui::AnyElement> = file
            .folders
            .iter()
            .enumerate()
            .map(|(index, folder)| self.folder_row(index, &folder.id, &folder.name, false, cx))
            .collect();
        let smart: Vec<gpui::AnyElement> = file
            .smart_folders
            .iter()
            .enumerate()
            .map(|(index, folder)| self.folder_row(index, &folder.id, &folder.name, true, cx))
            .collect();
        div()
            .id("library-sidebar")
            .key_context("LibrarySidebar")
            .track_focus(&organizer.focus)
            .on_action(cx.listener(|this, _: &RenameFolder, window, cx| {
                this.rename_focused_folder(window, cx);
            }))
            .on_action(cx.listener(|this, _: &DeleteFolder, window, cx| {
                this.delete_focused_folder(window, cx);
            }))
            .flex()
            .flex_col()
            .flex_shrink_0()
            .w(px(SIDEBAR_WIDTH))
            .min_h_0()
            .gap(px(1.))
            .p(px(8.))
            .border_r_1()
            .border_color(theme.border)
            .bg(theme.sidebar)
            .overflow_y_scroll()
            .child(self.sidebar_row(
                "library-place-all",
                Place::All,
                "icons/layout-grid.svg",
                t!("All items").into(),
                cx,
            ))
            .when(shows_pinned, |this| {
                this.child(self.sidebar_row(
                    "library-place-pinned",
                    Place::Pinned,
                    "icons/pin.svg",
                    t!("Pinned").into(),
                    cx,
                ))
            })
            .child(
                folders_head.child(new_folder.on_click(cx.listener(|this, _, window, cx| {
                    this.new_library_folder(false, window, cx);
                }))),
            )
            .children(folders)
            .when(file.folders.is_empty(), |this| {
                this.child(
                    div()
                        .px(px(8.))
                        .py(px(4.))
                        .text_size(px(12.))
                        .text_color(theme.muted_foreground)
                        .child(t!("Drop items here to group them")),
                )
            })
            .child(
                smart_head.child(new_smart.on_click(cx.listener(|this, _, window, cx| {
                    this.open_smart_dialog(None, window, cx);
                }))),
            )
            .children(smart)
    }

    pub(super) fn render_place_title(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let theme = self.theme;
        let organizer = &self.library.organizer;
        let name = organizer.place_name(&organizer.place)?;
        let smart = match &organizer.place {
            Place::Smart(id) => Some(id.clone()),
            _ => None,
        };
        Some(
            div()
                .flex()
                .flex_shrink_0()
                .items_center()
                .gap(px(8.))
                .pt(px(10.))
                .px(px(16.))
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(name),
                )
                .when(smart.is_some(), |this| {
                    this.child(
                        div()
                            .text_size(px(12.))
                            .text_color(theme.muted_foreground)
                            .child(t!("Updates by itself")),
                    )
                })
                .child(div().flex_1())
                .when_some(smart, |this, id| {
                    this.child(
                        self.ghost_button("library-edit-rules", t!("Edit rules"))
                            .px(px(8.))
                            .child(
                                svg()
                                    .path("icons/sliders-horizontal.svg")
                                    .size(px(14.))
                                    .text_color(theme.foreground),
                            )
                            .child(t!("Edit rules"))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.open_smart_dialog(Some(id.clone()), window, cx);
                            })),
                    )
                }),
        )
    }

    pub(super) fn render_sidebar_toggle(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let label = if self
            .library
            .organizer
            .shows_sidebar(self.library.state.sidebar_hidden)
        {
            t!("Hide sidebar")
        } else {
            t!("Show sidebar")
        };
        focus_ring(
            div()
                .id("library-sidebar-toggle")
                .role(gpui::Role::Button)
                .aria_label(label)
                .accessibility_id("library-sidebar-toggle")
                .tab_index(0)
                .flex()
                .flex_shrink_0()
                .items_center()
                .justify_center()
                .size(px(crate::theme::CONTROL_HEIGHT))
                .rounded(px(6.))
                .cursor_pointer()
                .hover(move |style| style.bg(theme.accent))
                .tooltip(move |_, cx| {
                    ui::Tooltip::new(&theme, label)
                        .keys(["Ctrl", "\\"])
                        .view(cx)
                })
                .tooltip_show_delay(ui::TOOLTIP_SHOW_DELAY)
                .child(
                    svg()
                        .path("icons/panel-left.svg")
                        .size(px(16.))
                        .text_color(theme.foreground),
                ),
            theme,
        )
        .on_click(cx.listener(|this, _, _, cx| this.toggle_library_sidebar(cx)))
    }

    pub(super) fn render_folder_toast(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let theme = self.theme;
        self.library.organizer.removed.as_ref()?;
        Some(
            div()
                .absolute()
                .bottom(px(TOAST_BOTTOM))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .px(px(16.))
                .child(
                    div()
                        .id("library-folder-toast")
                        .occlude()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .w(px(TOAST_WIDTH))
                        .max_w_full()
                        .py(px(8.))
                        .pl(px(12.))
                        .pr(px(8.))
                        .rounded(px(8.))
                        .border_1()
                        .border_color(theme.border)
                        .bg(theme.popover)
                        .text_size(px(13.))
                        .text_color(theme.foreground)
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(t!("Folder deleted. Its items stay in the library.")),
                        )
                        .child(focus_ring(
                            div()
                                .id("library-folder-undo")
                                .role(gpui::Role::Button)
                                .aria_label(t!("Undo"))
                                .tab_index(0)
                                .flex()
                                .flex_shrink_0()
                                .items_center()
                                .h(px(28.))
                                .px(px(10.))
                                .rounded(px(6.))
                                .cursor_pointer()
                                .font_weight(FontWeight::MEDIUM)
                                .hover(move |style| style.bg(theme.accent))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.undo_folder_delete(window, cx);
                                }))
                                .child(t!("Undo")),
                            theme,
                        )),
                ),
        )
    }

    pub(super) fn render_empty_place(&self) -> impl IntoElement {
        let theme = self.theme;
        let text = match self.library.organizer.place {
            Place::Smart(_) => t!("Nothing matches these rules yet"),
            _ => t!("Nothing here yet"),
        };
        div()
            .flex()
            .flex_1()
            .items_center()
            .justify_center()
            .text_size(px(13.))
            .text_color(theme.muted_foreground)
            .child(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pin_and_a_folder_drop_at_once_on_a_new_project_both_land_on_one_id() {
        for round in 0..10 {
            let dir = tempfile::tempdir().unwrap();
            let file_path = dir.path().join("library.json");
            let launch = folders::update(&file_path, |file| {
                file.new_folder("Launch", &[]);
            })
            .unwrap()
            .folders[0]
                .id
                .clone();
            let project = dir.path().join("Demo.ever");
            std::fs::create_dir_all(&project).unwrap();
            let start = std::sync::Barrier::new(2);
            std::thread::scope(|scope| {
                let pin = scope.spawn(|| {
                    start.wait();
                    organize_on_disk(&file_path, vec![(project.clone(), None)], |file, ids| {
                        file.toggle_pins(ids);
                        After::Stay
                    })
                    .1
                    .unwrap();
                });
                let drop = scope.spawn(|| {
                    start.wait();
                    organize_on_disk(&file_path, vec![(project.clone(), None)], |file, ids| {
                        file.toggle_members(&launch, ids);
                        After::Stay
                    })
                    .1
                    .unwrap();
                });
                pin.join().unwrap();
                drop.join().unwrap();
            });
            let id = frameever_utils::storage::project_id(&project).expect("the project has an id");
            let file = folders::load(&file_path).file;
            assert_eq!(file.pins, vec![id.clone()], "round {round}");
            assert_eq!(file.folders[0].members, vec![id], "round {round}");
        }
    }

    fn library_with_folder(
        cx: &mut gpui::TestAppContext,
    ) -> (Entity<LibraryWindow>, &mut gpui::VisualTestContext, String) {
        let session = cx.update(crate::session::RecordingSession::init);
        let (library, cx) =
            cx.add_window_view(move |window, cx| LibraryWindow::new(session, window, cx));
        let id = library.update(cx, |library, _| {
            library.shown = true;
            let id = library.library.organizer.file.new_folder("Launch", &[]);
            library.library.organizer.place = Place::Folder(id.clone());
            id
        });
        (library, cx, id)
    }

    fn typed(library: &Entity<LibraryWindow>, cx: &mut gpui::VisualTestContext) -> String {
        library.read_with(cx, |library, cx| {
            library
                .library
                .organizer
                .rename_input
                .read(cx)
                .text()
                .to_owned()
        })
    }

    #[gpui::test]
    fn every_library_control_has_a_role_and_a_name(cx: &mut gpui::TestAppContext) {
        let (_library, cx, _) = library_with_folder(cx);
        draw_at(NARROW_WIDTH + 400., cx);
        crate::accessibility::assert_every_control_is_named(cx, &[]);
    }

    #[gpui::test]
    fn pressing_f2_again_while_renaming_keeps_what_was_typed(cx: &mut gpui::TestAppContext) {
        let (library, cx, id) = library_with_folder(cx);
        library.update_in(cx, |library, window, cx| {
            library.rename_focused_folder(window, cx);
        });
        assert_eq!(typed(&library, cx), "Launch");
        library.update(cx, |library, cx| {
            library
                .library
                .organizer
                .rename_input
                .update(cx, |input, cx| input.set_text("Launch v2", cx));
        });

        library.update_in(cx, |library, window, cx| {
            library.rename_focused_folder(window, cx);
        });

        assert_eq!(typed(&library, cx), "Launch v2");
        library.read_with(cx, |library, _| {
            assert_eq!(library.library.organizer.renaming, Some(id));
        });
    }

    fn draw_at(width: f32, cx: &mut gpui::VisualTestContext) {
        cx.simulate_resize(gpui::size(px(width), px(800.)));
        cx.update(|window, cx| {
            window.refresh();
            window.draw(cx).clear(cx);
        });
    }

    fn sidebar_showing(library: &Entity<LibraryWindow>, cx: &mut gpui::VisualTestContext) -> bool {
        library.read_with(cx, |library, _| {
            library
                .library
                .organizer
                .shows_sidebar(library.library.state.sidebar_hidden)
        })
    }

    #[gpui::test]
    fn a_rename_started_with_the_sidebar_hidden_shows_it_first(cx: &mut gpui::TestAppContext) {
        let (library, cx, id) = library_with_folder(cx);
        draw_at(NARROW_WIDTH - 200., cx);
        assert!(!sidebar_showing(&library, cx));
        library.update_in(cx, |library, window, cx| {
            library.start_folder_rename(id.clone(), window, cx);
        });
        draw_at(NARROW_WIDTH - 200., cx);
        assert!(sidebar_showing(&library, cx));

        library.update(cx, |library, _| {
            library.library.organizer.renaming = None;
            library.library.state.sidebar_hidden = true;
        });
        draw_at(NARROW_WIDTH + 200., cx);
        assert!(!sidebar_showing(&library, cx));
        library.update_in(cx, |library, window, cx| {
            library.start_folder_rename(id.clone(), window, cx);
        });
        draw_at(NARROW_WIDTH + 200., cx);
        assert!(sidebar_showing(&library, cx));
    }

    #[test]
    fn the_sidebar_folds_away_when_narrow_and_its_own_switch_shows_it_again() {
        assert!(sidebar_shown(false, false, false));
        assert!(!sidebar_shown(false, false, true));
        assert!(!sidebar_shown(true, false, false));
        assert!(sidebar_shown(true, true, true));
    }
}
