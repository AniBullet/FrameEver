mod context;
mod delete;
mod detail;
mod overlay;
mod preview;
mod scrub;

use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

use gpui::{
    AnyElement, AppContext as _, Bounds, ClickEvent, Context, Entity, FocusHandle, FontWeight,
    InteractiveElement, IntoElement, ListAlignment, ListOffset, ListState, MouseButton,
    MouseDownEvent, MouseMoveEvent, ParentElement, Pixels, Point, RenderImage, SharedString,
    StatefulInteractiveElement, Styled, StyledImage as _, Task, Window, deferred, div, img, list,
    prelude::FluentBuilder, px, svg, transparent_black,
};

use super::{MainWindow, app_windows, surfaces::focus_ring};
use crate::{
    doodle,
    i18n::t,
    library::{
        self, MediaKind,
        compress_queue::{CompressQueue, JobState},
        view::{
            self, Clock, DateGroup, DayLabel, Entry, LibraryView, Row, Selection, SortDirection,
            SortKey, StatusFilter, Step, Surface, TILE_GAP, Tab, ViewMode, ViewState,
        },
        watch::LibraryWatcher,
    },
    onboarding_doodles,
    theme::Theme,
    ui,
};

const PADDING: f32 = 12.;
const TILE_PADDING: f32 = 6.;
const MAX_THUMBNAILS: usize = 600;
const FRESH_HOLD: Duration = Duration::from_secs(6);
const WATCH_DEBOUNCE: Duration = Duration::from_millis(300);
const SAVE_DELAY: Duration = Duration::from_millis(400);
const REOPEN_GUARD: Duration = Duration::from_millis(600);
const DETAIL_WIDTH: f32 = 340.;
const JOB_BAR: f32 = 3.;
const LIST_ROW: f32 = 34.;
const LIST_GAP: f32 = 8.;
const LIST_COLUMNS: [(Option<SortKey>, f32); 4] = [
    (None, 84.),
    (Some(SortKey::Duration), 70.),
    (Some(SortKey::Size), 74.),
    (Some(SortKey::Date), 88.),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LibraryMenu {
    Status,
    Sort,
    Import,
}

struct Thumbnail {
    version: Option<u128>,
    image: Arc<RenderImage>,
}

#[derive(Default)]
struct Thumbnails {
    images: HashMap<PathBuf, Thumbnail>,
    requested: HashMap<PathBuf, Option<u128>>,
    order: VecDeque<PathBuf>,
    queued: Vec<ThumbnailJob>,
    running: bool,
}

#[derive(Clone)]
struct ThumbnailJob {
    source: PathBuf,
    version: Option<u128>,
}

struct Marquee {
    origin: Point<Pixels>,
    corner: Point<Pixels>,
    base: Vec<PathBuf>,
}

impl Marquee {
    fn area(&self) -> Bounds<Pixels> {
        Bounds::from_corners(
            Point::new(
                self.origin.x.min(self.corner.x),
                self.origin.y.min(self.corner.y),
            ),
            Point::new(
                self.origin.x.max(self.corner.x),
                self.origin.y.max(self.corner.y),
            ),
        )
    }
}

pub(super) struct LibraryPage {
    entries: Option<Vec<Entry>>,
    state: ViewState,
    view: LibraryView,
    rows: Vec<Row>,
    columns: usize,
    imports: Vec<usize>,
    surface: Option<Surface>,
    surface_shown_at: Instant,
    surfaces_drawn: Vec<Surface>,
    surface_still: bool,
    layout_stale: bool,
    list: ListState,
    menu: Option<LibraryMenu>,
    menu_closed: Option<(LibraryMenu, Instant)>,
    slider: ui::SliderTrack,
    pub(super) slider_dragging: bool,
    thumbnails: Thumbnails,
    scan_task: Option<Task<()>>,
    watch_task: Option<Task<()>>,
    watched: Vec<PathBuf>,
    save_task: Option<Task<()>>,
    fresh: Vec<PathBuf>,
    fresh_shown_at: Instant,
    fresh_task: Option<Task<()>>,
    pub(super) unseen: Vec<PathBuf>,
    first_line: bool,
    selection: Selection,
    order: Vec<PathBuf>,
    items_focus: FocusHandle,
    reveal_lead: bool,
    marquee: Option<Marquee>,
    drawn: Rc<RefCell<HashMap<PathBuf, Bounds<Pixels>>>>,
    context: Option<context::ContextMenu>,
    detail: detail::DetailState,
    overlay: Option<overlay::Overlay>,
    overlay_focus: FocusHandle,
    scrub: scrub::ScrubState,
    delete: delete::DeleteState,
    compress: CompressQueue,
    last_click: Option<PathBuf>,
}

impl LibraryPage {
    pub(super) fn new(
        name_input: Entity<ui::TextInputState>,
        items_focus: FocusHandle,
        overlay_focus: FocusHandle,
    ) -> Self {
        Self {
            entries: None,
            state: crate::store::load().library,
            view: LibraryView::default(),
            rows: Vec::new(),
            columns: 1,
            imports: Vec::new(),
            surface: None,
            surface_shown_at: Instant::now(),
            surfaces_drawn: Vec::new(),
            surface_still: false,
            layout_stale: false,
            list: ListState::new(0, ListAlignment::Top, px(600.)),
            menu: None,
            menu_closed: None,
            slider: Default::default(),
            slider_dragging: false,
            thumbnails: Thumbnails::default(),
            scan_task: None,
            watch_task: None,
            watched: Vec::new(),
            save_task: None,
            fresh: Vec::new(),
            fresh_shown_at: Instant::now(),
            fresh_task: None,
            unseen: Vec::new(),
            first_line: false,
            selection: Selection::default(),
            order: Vec::new(),
            items_focus: items_focus.tab_index(0).tab_stop(true),
            reveal_lead: false,
            marquee: None,
            drawn: Rc::default(),
            context: None,
            detail: detail::DetailState::new(name_input),
            overlay: None,
            overlay_focus,
            scrub: scrub::ScrubState::default(),
            delete: delete::DeleteState::default(),
            compress: CompressQueue::default(),
            last_click: None,
        }
    }

    pub(super) fn menu_open(&self) -> bool {
        self.menu.is_some() || self.context.is_some()
    }

    pub(super) fn items_focus(&self) -> &FocusHandle {
        &self.items_focus
    }

    pub(super) fn overlay_focus(&self) -> &FocusHandle {
        &self.overlay_focus
    }

    pub(super) fn has_selection(&self) -> bool {
        !self.selection.is_empty()
    }

    pub(super) fn dialog_open(&self) -> bool {
        self.delete.dialog_open()
    }

    fn entry(&self, path: &Path) -> Option<&Entry> {
        self.entries
            .as_ref()?
            .iter()
            .find(|entry| entry.path == path)
    }

    fn selected_entries(&self) -> Vec<&Entry> {
        self.selection
            .items()
            .iter()
            .filter_map(|path| self.entry(path))
            .collect()
    }

    fn hide_entries<'a>(&mut self, paths: impl Iterator<Item = &'a PathBuf>) {
        let paths: Vec<&PathBuf> = paths.collect();
        if let Some(entries) = &mut self.entries {
            entries.retain(|entry| !paths.contains(&&entry.path));
        }
    }

    fn lead_entry(&self) -> Option<&Entry> {
        self.entry(self.selection.lead()?)
    }

    fn single_entry(&self) -> Option<&Entry> {
        self.entry(self.selection.single()?)
    }

    fn panel_width(&self) -> f32 {
        if self.selection.is_empty() {
            0.
        } else {
            DETAIL_WIDTH
        }
    }
}

fn job_label(job: JobState) -> &'static str {
    match job {
        JobState::Waiting => t!("Waiting"),
        JobState::Running { .. } => t!("Compressing…"),
    }
}

fn tab_label(tab: Tab) -> &'static str {
    match tab {
        Tab::All => t!("All"),
        Tab::Studio => t!("Studio"),
        Tab::Instant => t!("Instant"),
        Tab::Screenshots => t!("Screenshots"),
    }
}

fn status_label(status: StatusFilter) -> &'static str {
    match status {
        StatusFilter::Exported => t!("Exported"),
        StatusFilter::NotExported => t!("Not exported"),
        StatusFilter::CompressedCopy => t!("Has a compressed copy"),
        StatusFilter::CompressedOnly => t!("Compressed copy only"),
    }
}

fn sort_label(sort: SortKey) -> &'static str {
    match sort {
        SortKey::Date => t!("Date"),
        SortKey::Name => t!("Name"),
        SortKey::Size => t!("Size"),
        SortKey::Duration => t!("Duration"),
    }
}

fn direction_label(sort: SortKey, direction: SortDirection) -> &'static str {
    match (sort, direction) {
        (SortKey::Date, SortDirection::Descending) => t!("Newest first"),
        (SortKey::Date, SortDirection::Ascending) => t!("Oldest first"),
        (SortKey::Name, SortDirection::Ascending) => t!("A to Z"),
        (SortKey::Name, SortDirection::Descending) => t!("Z to A"),
        (SortKey::Size, SortDirection::Descending) => t!("Largest first"),
        (SortKey::Size, SortDirection::Ascending) => t!("Smallest first"),
        (SortKey::Duration, SortDirection::Descending) => t!("Longest first"),
        (SortKey::Duration, SortDirection::Ascending) => t!("Shortest first"),
    }
}

fn column_label(sort: Option<SortKey>) -> &'static str {
    match sort {
        Some(sort) => sort_label(sort),
        None => t!("Type"),
    }
}

fn day_label(clock: &Clock, millis: f64) -> SharedString {
    match clock.day_of(millis) {
        DayLabel::Today => t!("Today").into(),
        DayLabel::Yesterday => t!("Yesterday").into(),
        DayLabel::Date(date) => date.format("%Y-%m-%d").to_string().into(),
    }
}

fn group_label(group: DateGroup) -> SharedString {
    match group {
        DateGroup::Today => t!("Today").into(),
        DateGroup::Yesterday => t!("Yesterday").into(),
        DateGroup::ThisWeek => t!("This week").into(),
        DateGroup::Month { year, month } => {
            t!("{month} {year}", month = month_label(month), year = year).into()
        }
    }
}

fn month_label(month: u32) -> &'static str {
    match month {
        1 => t!("January"),
        2 => t!("February"),
        3 => t!("March"),
        4 => t!("April"),
        5 => t!("May"),
        6 => t!("June"),
        7 => t!("July"),
        8 => t!("August"),
        9 => t!("September"),
        10 => t!("October"),
        11 => t!("November"),
        _ => t!("December"),
    }
}

fn kind_icon(kind: MediaKind) -> &'static str {
    match kind {
        MediaKind::Studio => "icons/clapperboard.svg",
        MediaKind::Instant => "icons/zap.svg",
        MediaKind::Screenshot => "icons/image.svg",
    }
}

fn kind_label(kind: MediaKind) -> &'static str {
    match kind {
        MediaKind::Studio => t!("Studio"),
        MediaKind::Instant => t!("Instant"),
        MediaKind::Screenshot => t!("Screenshot"),
    }
}

fn shown_label(count: usize, size: u64) -> String {
    let size = view::format_size(size);
    if count == 1 {
        t!("1 item · {size}", size = size).to_string()
    } else {
        t!("{count} items · {size}", count = count, size = size).to_string()
    }
}

fn import_wanted(tab: Tab, kind: crate::import::ImportKind) -> bool {
    match kind {
        crate::import::ImportKind::Video => tab != Tab::Screenshots,
        crate::import::ImportKind::Image => matches!(tab, Tab::All | Tab::Screenshots),
    }
}

impl MainWindow {
    pub(super) fn refresh_library(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.library.scan_task = Some(cx.spawn_in(window, async move |this, cx| {
            let (entries, dirs) = cx
                .background_executor()
                .spawn(async { (library::list_entries(), library::watched_dirs()) })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.set_library_entries(entries, dirs, window, cx);
            })
            .ok();
        }));
    }

    fn set_library_entries(
        &mut self,
        entries: Vec<Entry>,
        dirs: Vec<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.library.fresh.is_empty() && !crate::store::load().library_first_line_shown {
            self.library.first_line = entries.len() <= self.library.fresh.len();
            cx.background_spawn(async {
                crate::store::update(|state| state.library_first_line_shown = true);
            })
            .detach();
        }
        self.library.entries = Some(entries);
        self.library.detail.stale = true;
        self.rebuild_library(cx);
        if self.shell.shows_library() && dirs != self.library.watched {
            self.watch_library(dirs, window, cx);
        }
    }

    fn watch_library(&mut self, dirs: Vec<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        let watcher = LibraryWatcher::start(&dirs);
        self.library.watched = dirs;
        self.library.watch_task = watcher.map(|watcher| {
            cx.spawn_in(window, async move |this, cx| {
                while watcher.changed().await {
                    cx.background_executor().timer(WATCH_DEBOUNCE).await;
                    watcher.settle();
                    if this
                        .update_in(cx, |this, window, cx| this.refresh_library(window, cx))
                        .is_err()
                    {
                        return;
                    }
                }
            })
        });
    }

    pub(super) fn rebuild_library(&mut self, cx: &mut Context<Self>) {
        let page = &mut self.library;
        let Some(entries) = &page.entries else {
            return;
        };
        page.view = view::build(
            entries,
            &page.state,
            &self.search,
            &page.fresh,
            &Clock::now(),
        );
        page.order = page.view.order(entries);
        page.selection.retain(&page.order);
        let rows = view::rows(
            &page.view,
            page.state.view,
            page.columns,
            page.first_line,
            &page.imports,
        );
        let surface = page.view.surface(!page.imports.is_empty());
        if page.surface != Some(surface) {
            page.surface = Some(surface);
            page.surface_shown_at = Instant::now();
            page.surface_still = page.surfaces_drawn.contains(&surface);
            page.surfaces_drawn.push(surface);
        }
        if rows != page.rows {
            let top = page.list.logical_scroll_top();
            page.list.reset(rows.len());
            page.list.scroll_to(ListOffset {
                item_ix: top.item_ix.min(rows.len().saturating_sub(1)),
                offset_in_item: top.offset_in_item,
            });
            page.rows = rows;
        }
        self.sync_library_detail(cx);
        cx.notify();
    }

    pub(super) fn sync_library_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let width = f32::from(window.viewport_size().width) - self.library.panel_width();
        let columns = view::columns(width - PADDING * 2., self.library.state.tile_size);
        let tab = self.library.state.tab;
        let imports: Vec<usize> = crate::import::imports_snapshot(cx)
            .iter()
            .enumerate()
            .filter(|(_, entry)| import_wanted(tab, entry.kind))
            .map(|(index, _)| index)
            .collect();
        if self.library.layout_stale
            || columns != self.library.columns
            || imports != self.library.imports
        {
            self.library.layout_stale = false;
            self.library.columns = columns;
            self.library.imports = imports;
            self.rebuild_library(cx);
        }
        if std::mem::take(&mut self.library.reveal_lead) {
            let page = &self.library;
            let row = page
                .selection
                .lead()
                .and_then(|lead| {
                    page.entries
                        .as_ref()?
                        .iter()
                        .position(|entry| entry.path == lead)
                })
                .and_then(|item| view::row_of(&page.rows, item));
            if let Some(row) = row {
                page.list.scroll_to_reveal_item(row);
            }
        }
        if let Some(entry) = self.library.single_entry().cloned() {
            self.library_thumbnail(&entry, window, cx);
        }
        self.settle_library_overlay(window, cx);
        let bounds = self.library_stage_pixels(window);
        let preview = &mut self.library.detail.preview;
        preview.drop_retired(window);
        preview.sync_bounds(bounds);
    }

    fn change_selection(
        &mut self,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut Selection, &[PathBuf]),
    ) {
        let page = &mut self.library;
        change(&mut page.selection, &page.order);
        self.sync_library_detail(cx);
        cx.notify();
    }

    pub(super) fn clear_library_selection(&mut self, cx: &mut Context<Self>) {
        self.change_selection(cx, |selection, _| selection.clear());
    }

    pub(super) fn select_all_library(&mut self, cx: &mut Context<Self>) {
        self.change_selection(cx, |selection, order| selection.select_all(order));
    }

    pub(super) fn show_library_as(&mut self, mode: ViewMode, cx: &mut Context<Self>) {
        if self.shell.shows_library() {
            self.change_library_view(cx, |state| state.view = mode);
            self.library.reveal_lead = true;
        }
    }

    pub(super) fn step_library(&mut self, step: Step, extend: bool, cx: &mut Context<Self>) {
        if self.library.context.is_some() {
            self.step_library_context(step, cx);
            return;
        }
        let page = &self.library;
        let Some(entries) = &page.entries else {
            return;
        };
        let from = page
            .selection
            .lead()
            .and_then(|lead| entries.iter().position(|entry| entry.path == lead));
        let Some(path) = view::step_from(&page.rows, from, step)
            .and_then(|item| entries.get(item))
            .map(|entry| entry.path.clone())
        else {
            return;
        };
        self.library.reveal_lead = true;
        self.change_selection(cx, |selection, order| {
            if extend {
                selection.extend_to(&path, order);
            } else {
                selection.click(&path);
            }
        });
    }

    pub(super) fn enter_library(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.library.context.is_some() {
            self.run_highlighted_context(window, cx);
        } else if let Some(entry) = self.library.lead_entry().cloned() {
            self.open_library_entry(&entry, window, cx);
        } else {
            cx.propagate();
        }
    }

    pub(super) fn open_library_entry(
        &mut self,
        entry: &Entry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if entry.kind == MediaKind::Instant && entry.screenshot.is_none() {
            self.open_library_overlay(&entry.path, window, cx);
            return;
        }
        self.pause_library_preview(cx);
        open_entry(entry, cx);
    }

    pub(super) fn focus_after_search(&self, cx: &mut Context<Self>) {
        let main = cx.global::<app_windows::AppWindows>().main;
        let in_library = self.shell.shows_library();
        cx.defer(move |cx| {
            main.update(cx, |this, window, cx| {
                let focus = if in_library {
                    &this.library.items_focus
                } else {
                    &this.focus_handle
                };
                window.focus(focus, cx);
            })
            .ok();
        });
    }

    fn click_library_item(
        &mut self,
        path: &Path,
        event: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let modifiers = event.modifiers();
        let first = self.library.last_click.replace(path.to_path_buf());
        if event.click_count() >= 2 && !modifiers.control && !modifiers.shift {
            let target = first.as_deref().unwrap_or(path);
            if let Some(entry) = self.library.entry(target).cloned() {
                self.open_library_entry(&entry, window, cx);
            }
            return;
        }
        self.change_selection(cx, |selection, order| {
            if modifiers.shift {
                selection.extend_to(path, order);
            } else if modifiers.control {
                selection.toggle(path);
            } else {
                selection.click(path);
            }
        });
    }

    fn library_item_events(
        &self,
        item: gpui::Stateful<gpui::Div>,
        path: &Path,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let (click, menu) = (path.to_path_buf(), path.to_path_buf());
        item.on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _, window, cx| {
                cx.stop_propagation();
                window.focus(&this.library.items_focus, cx);
            }),
        )
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                cx.stop_propagation();
                window.focus(&this.library.items_focus, cx);
                this.open_library_item_menu(&menu, event.position, cx);
            }),
        )
        .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
            this.click_library_item(&click, event, window, cx);
        }))
    }

    fn begin_library_marquee(&mut self, event: &MouseDownEvent, cx: &mut Context<Self>) {
        let base = if event.modifiers.control {
            self.library.selection.items().to_vec()
        } else {
            Vec::new()
        };
        if !event.modifiers.control {
            self.clear_library_selection(cx);
        }
        self.library.marquee = Some(Marquee {
            origin: event.position,
            corner: event.position,
            base,
        });
        cx.notify();
    }

    fn drag_library_marquee(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        let page = &mut self.library;
        let Some(marquee) = &mut page.marquee else {
            return;
        };
        marquee.corner = event.position;
        let area = marquee.area();
        let base = marquee.base.clone();
        let hits: Vec<PathBuf> = {
            let drawn = page.drawn.borrow();
            page.order
                .iter()
                .filter(|path| drawn.get(*path).is_some_and(|tile| tile.intersects(&area)))
                .cloned()
                .collect()
        };
        self.change_selection(cx, |selection, order| {
            selection.marquee(&base, &hits, order);
        });
    }

    pub(super) fn render_library_marquee(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let theme = self.theme;
        let area = self.library.marquee.as_ref()?.area();
        Some(
            ui::Slider::drag_layer(
                "library-marquee",
                cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                    this.drag_library_marquee(event, cx);
                }),
                cx.listener(|this, _: &gpui::MouseUpEvent, _, cx| {
                    this.library.marquee = None;
                    cx.notify();
                }),
            )
            .child(
                div()
                    .absolute()
                    .left(area.origin.x)
                    .top(area.origin.y)
                    .w(area.size.width)
                    .h(area.size.height)
                    .rounded(px(2.))
                    .border_1()
                    .border_color(theme.ring)
                    .bg(theme.selection.opacity(0.4)),
            ),
        )
    }

    pub(super) fn open_library_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.library.items_focus, cx);
        let unseen = std::mem::take(&mut self.library.unseen);
        if !unseen.is_empty() {
            self.show_fresh(unseen, window, cx);
        }
        self.refresh_library(window, cx);
    }

    pub(super) fn close_library_page(&mut self, window: &mut Window) {
        let page = &mut self.library;
        page.entries = None;
        page.scan_task = None;
        page.watch_task = None;
        page.watched.clear();
        page.menu = None;
        page.fresh.clear();
        page.fresh_task = None;
        page.first_line = false;
        page.last_click = None;
        page.selection.clear();
        page.order.clear();
        page.marquee = None;
        page.context = None;
        page.detail.clear();
        let overlay = page.overlay.take();
        self.retire_overlay(overlay);
        self.drop_library_strips(window);
        let page = &mut self.library;
        page.detail.preview.drop_retired(window);
        page.drawn.borrow_mut().clear();
        page.rows.clear();
        page.list.reset(0);
        page.view = LibraryView::default();
        page.surface = None;
        page.surfaces_drawn.clear();
        page.thumbnails.queued.clear();
        page.thumbnails.requested.clear();
        page.thumbnails.order.clear();
        for (_, thumbnail) in page.thumbnails.images.drain() {
            if let Err(error) = window.drop_image(thumbnail.image) {
                tracing::debug!("Could not drop a library thumbnail: {error}");
            }
        }
    }

    pub fn note_new_recording(
        &mut self,
        project: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.shell.shows_library() {
            self.show_fresh(vec![project], window, cx);
            self.refresh_library(window, cx);
        } else {
            self.library.unseen.push(project);
            cx.notify();
        }
    }

    fn show_fresh(&mut self, projects: Vec<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        self.library.fresh.extend(projects);
        self.library.fresh_shown_at = Instant::now();
        self.library.fresh_task = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(FRESH_HOLD).await;
            this.update(cx, |this, cx| {
                this.library.fresh.clear();
                this.library.first_line = false;
                this.rebuild_library(cx);
            })
            .ok();
        }));
    }

    pub(super) fn change_library_view(
        &mut self,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut ViewState),
    ) {
        change(&mut self.library.state);
        self.library.list.scroll_to(ListOffset::default());
        self.library.layout_stale = true;
        self.save_library_view(cx);
        cx.notify();
    }

    fn resize_library_tiles(&mut self, size: f32, cx: &mut Context<Self>) {
        self.library.state.set_tile_size(size);
        self.library.layout_stale = true;
        self.save_library_view(cx);
        cx.notify();
    }

    fn save_library_view(&mut self, cx: &mut Context<Self>) {
        let state = self.library.state.clone();
        self.library.save_task = Some(cx.spawn(async move |_, cx| {
            cx.background_executor().timer(SAVE_DELAY).await;
            cx.background_spawn(async move {
                crate::store::update(|stored| stored.library = state);
            })
            .await;
        }));
    }

    pub(super) fn step_library_tiles(&mut self, steps: i32, cx: &mut Context<Self>) {
        if self.shell.shows_library() {
            let size = self.library.state.tile_size + steps as f32 * view::TILE_STEP;
            self.resize_library_tiles(size, cx);
        }
    }

    pub(super) fn focus_library_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.shell.shows_library() && !self.library_overlay_open() {
            let focus = self.search_input.read(cx).focus_handle();
            window.focus(&focus, cx);
        }
    }

    pub(super) fn close_library_menu(&mut self, cx: &mut Context<Self>) {
        if self.library.context.take().is_some() {
            cx.notify();
        }
        if let Some(menu) = self.library.menu.take() {
            self.library.menu_closed = Some((menu, Instant::now()));
            cx.notify();
        }
    }

    fn toggle_library_menu(&mut self, menu: LibraryMenu, cx: &mut Context<Self>) {
        let just_closed = self
            .library
            .menu_closed
            .take()
            .is_some_and(|(closed, at)| closed == menu && at.elapsed() < REOPEN_GUARD);
        self.library.menu = (!just_closed && self.library.menu != Some(menu)).then_some(menu);
        cx.notify();
    }

    fn clear_library_filters(&mut self, cx: &mut Context<Self>) {
        self.clear_search(cx);
        self.change_library_view(cx, ViewState::clear_filters);
    }

    fn library_thumbnail(
        &mut self,
        entry: &Entry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Arc<RenderImage>> {
        let source = entry.thumbnail.clone()?;
        let thumbnails = &mut self.library.thumbnails;
        let held = thumbnails.images.get(&source);
        let image = held.map(|held| held.image.clone());
        let current = held.is_some_and(|held| held.version == entry.thumbnail_version);
        let requested = thumbnails.requested.get(&source) == Some(&entry.thumbnail_version);
        if !current && !requested {
            thumbnails
                .requested
                .insert(source.clone(), entry.thumbnail_version);
            thumbnails.queued.push(ThumbnailJob {
                source,
                version: entry.thumbnail_version,
            });
            self.run_thumbnail_jobs(window, cx);
        }
        image
    }

    fn run_thumbnail_jobs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if std::mem::replace(&mut self.library.thumbnails.running, true) {
            return;
        }
        cx.spawn_in(window, async move |this, cx| {
            loop {
                let Ok(jobs) = this.update(cx, |this, _| this.library.thumbnails.queued.clone())
                else {
                    return;
                };
                if jobs.is_empty() {
                    this.update(cx, |this, _| this.library.thumbnails.running = false)
                        .ok();
                    return;
                }
                let (_decodes, results) =
                    library::spawn_decode_pool(cx.background_executor(), jobs.clone(), |job| {
                        library::decode_thumbnail(&job.source).map(|image| (job, image))
                    });
                while let Ok(first) = results.recv_async().await {
                    let mut batch = vec![first];
                    batch.extend(results.try_iter());
                    if this
                        .update_in(cx, |this, window, cx| {
                            for (job, image) in batch {
                                this.store_thumbnail(job, image, window);
                            }
                            cx.notify();
                        })
                        .is_err()
                    {
                        return;
                    }
                }
                let finished = this.update(cx, |this, _| {
                    this.library
                        .thumbnails
                        .queued
                        .retain(|job| !jobs.iter().any(|done| done.source == job.source));
                });
                if finished.is_err() {
                    return;
                }
            }
        })
        .detach();
    }

    fn store_thumbnail(&mut self, job: ThumbnailJob, image: Arc<RenderImage>, window: &mut Window) {
        if self.library.entries.is_none() {
            if let Err(error) = window.drop_image(image) {
                tracing::debug!("Could not drop a library thumbnail: {error}");
            }
            return;
        }
        let thumbnails = &mut self.library.thumbnails;
        let replaced = thumbnails.images.insert(
            job.source.clone(),
            Thumbnail {
                version: job.version,
                image,
            },
        );
        let mut retired = replaced
            .map(|thumbnail| thumbnail.image)
            .into_iter()
            .collect::<Vec<_>>();
        thumbnails.order.retain(|held| held != &job.source);
        thumbnails.order.push_back(job.source);
        while thumbnails.order.len() > MAX_THUMBNAILS {
            let evicted = thumbnails.order.pop_front().and_then(|path| {
                thumbnails.requested.remove(&path);
                thumbnails.images.remove(&path)
            });
            retired.extend(evicted.map(|thumbnail| thumbnail.image));
        }
        for image in retired {
            if let Err(error) = window.drop_image(image) {
                tracing::debug!("Could not drop a library thumbnail: {error}");
            }
        }
    }

    pub(super) fn render_library_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let (opacity, slide) = self
            .shell
            .library_fade(self.transition_started.elapsed(), cx.reduce_motion());

        div()
            .relative()
            .left(px(slide))
            .opacity(opacity)
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .w_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(self.render_library_toolbar(cx))
            .children(self.render_library_filters(cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .when(self.shows_list_head(), |this| {
                                this.child(self.render_list_head(cx))
                            })
                            .child(self.render_library_items(cx))
                            .child(self.render_library_status(cx)),
                    )
                    .children(self.render_library_detail(cx)),
            )
            .children(self.render_library_context(cx))
    }

    fn ghost_button(&self, id: &'static str, label: &'static str) -> gpui::Stateful<gpui::Div> {
        let theme = self.theme;
        focus_ring(
            div()
                .id(id)
                .tab_index(0)
                .flex()
                .flex_shrink_0()
                .items_center()
                .gap(px(6.))
                .h(px(crate::theme::CONTROL_HEIGHT))
                .rounded(px(6.))
                .text_size(px(12.))
                .text_color(theme.foreground)
                .hover(move |style| style.bg(theme.accent))
                .tooltip(move |_, cx| ui::Tooltip::new(&theme, label).view(cx))
                .tooltip_show_delay(ui::TOOLTIP_SHOW_DELAY),
            theme,
        )
    }

    fn render_library_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let state = &self.library.state;
        let statuses = state.statuses.len();
        let sort_icon = match state.direction {
            SortDirection::Ascending => "icons/arrow-up-wide-narrow.svg",
            SortDirection::Descending => "icons/arrow-down-wide-narrow.svg",
        };
        self.search_input
            .update(cx, |input, _| input.set_placeholder(t!("Search")));

        let status = self
            .ghost_button("library-status", t!("Status"))
            .px(px(10.))
            .child(
                svg()
                    .path("icons/list-filter.svg")
                    .size(px(16.))
                    .text_color(theme.foreground),
            )
            .child(t!("Status"))
            .when(statuses > 0, |this| {
                this.child(library_badge(&theme, statuses))
            })
            .on_click(cx.listener(|this, _, _, cx| {
                this.toggle_library_menu(LibraryMenu::Status, cx);
            }));
        let import = self
            .ghost_button("library-import", t!("Import"))
            .px(px(8.))
            .child(
                svg()
                    .path("icons/import.svg")
                    .size(px(16.))
                    .text_color(theme.foreground),
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.toggle_library_menu(LibraryMenu::Import, cx);
            }));
        let sort = self
            .ghost_button("library-sort", t!("Sort"))
            .px(px(8.))
            .child(
                svg()
                    .path(sort_icon)
                    .size(px(16.))
                    .text_color(theme.foreground),
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.toggle_library_menu(LibraryMenu::Sort, cx);
            }));

        div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap(px(8.))
            .py(px(8.))
            .px(px(PADDING))
            .border_b_1()
            .border_color(theme.border)
            .child(self.render_library_tabs(cx))
            .child(
                div()
                    .relative()
                    .child(status)
                    .when(self.library.menu == Some(LibraryMenu::Status), |this| {
                        this.child(self.render_status_menu(cx))
                    }),
            )
            .child(div().flex_1().min_w_0())
            .child(
                div()
                    .flex()
                    .w(px(190.))
                    .flex_shrink_1()
                    .min_w_0()
                    .child(ui::TextInput::search(
                        &theme,
                        "library-search",
                        &self.search_input,
                    )),
            )
            .child(
                div()
                    .relative()
                    .child(import)
                    .when(self.library.menu == Some(LibraryMenu::Import), |this| {
                        this.child(self.render_import_menu(cx))
                    }),
            )
            .child(
                div()
                    .relative()
                    .child(sort)
                    .when(self.library.menu == Some(LibraryMenu::Sort), |this| {
                        this.child(self.render_sort_menu(cx))
                    }),
            )
            .child(self.render_view_switch(cx))
    }

    fn render_view_switch(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let current = self.library.state.view;
        let options: [(ViewMode, &'static str, &'static str, &'static str); 2] = [
            (
                ViewMode::Grid,
                "icons/layout-grid.svg",
                t!("Grid"),
                "Ctrl+1",
            ),
            (ViewMode::List, "icons/list.svg", t!("List"), "Ctrl+2"),
        ];
        div()
            .flex()
            .flex_shrink_0()
            .p(px(2.))
            .gap(px(2.))
            .rounded(px(6.))
            .bg(theme.muted)
            .children(
                options
                    .into_iter()
                    .enumerate()
                    .map(|(index, (mode, icon, label, keys))| {
                        let selected = mode == current;
                        focus_ring(
                            div()
                                .id(("library-view", index))
                                .tab_index(0)
                                .flex()
                                .items_center()
                                .justify_center()
                                .size(px(crate::theme::CONTROL_HEIGHT - 6.))
                                .rounded(px(5.))
                                .when(selected, |this| this.bg(theme.secondary))
                                .tooltip(move |_, cx| {
                                    ui::Tooltip::new(&theme, format!("{label}  {keys}")).view(cx)
                                })
                                .tooltip_show_delay(ui::TOOLTIP_SHOW_DELAY)
                                .child(svg().path(icon).size(px(14.)).text_color(if selected {
                                    theme.foreground
                                } else {
                                    theme.muted_foreground
                                })),
                            theme,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.show_library_as(mode, cx);
                        }))
                    }),
            )
    }

    fn render_library_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let current = self.library.state.tab;
        div()
            .flex()
            .flex_shrink_0()
            .p(px(2.))
            .gap(px(2.))
            .rounded(px(6.))
            .bg(theme.muted)
            .children(Tab::ALL.into_iter().enumerate().map(|(index, tab)| {
                let selected = tab == current;
                let color = if selected {
                    theme.foreground
                } else {
                    theme.muted_foreground
                };
                focus_ring(
                    div()
                        .id(("library-tab", index))
                        .tab_index(0)
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .h(px(crate::theme::CONTROL_HEIGHT - 6.))
                        .px(px(10.))
                        .rounded(px(5.))
                        .text_size(px(12.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(color)
                        .when(selected, |this| this.bg(theme.secondary))
                        .hover(move |style| style.text_color(theme.foreground))
                        .child(tab_label(tab))
                        .child(
                            div()
                                .font_weight(FontWeight::NORMAL)
                                .opacity(0.8)
                                .child(self.library.view.tab_count(tab).to_string()),
                        ),
                    theme,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.change_library_view(cx, |state| state.tab = tab);
                }))
            }))
    }

    fn library_popup(&self, width: f32, right: bool, cx: &mut Context<Self>) -> gpui::Div {
        let theme = self.theme;
        div()
            .absolute()
            .top(px(crate::theme::CONTROL_HEIGHT + 4.))
            .when(right, |this| this.right_0())
            .when(!right, |this| this.left_0())
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
            .on_mouse_down_out(cx.listener(|this, _, _, cx| this.close_library_menu(cx)))
    }

    fn popup_row(&self, id: impl Into<gpui::ElementId>) -> gpui::Stateful<gpui::Div> {
        let theme = self.theme;
        focus_ring(
            div()
                .id(id.into())
                .tab_index(0)
                .flex()
                .items_center()
                .gap(px(8.))
                .h(px(32.))
                .px(px(8.))
                .rounded(px(5.))
                .hover(move |style| style.bg(theme.accent)),
            theme,
        )
    }

    fn popup_tick(&self, checked: bool) -> impl IntoElement {
        let theme = self.theme;
        div().size(px(14.)).flex_shrink_0().when(checked, |this| {
            this.child(
                svg()
                    .path("icons/check.svg")
                    .size(px(14.))
                    .text_color(theme.popover_foreground),
            )
        })
    }

    fn render_status_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let held = self.library.state.statuses.clone();
        let rows = StatusFilter::ALL
            .into_iter()
            .enumerate()
            .map(|(index, status)| {
                let on = held.contains(&status);
                self.popup_row(("library-status-row", index))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(16.))
                            .flex_shrink_0()
                            .rounded(px(3.))
                            .border_1()
                            .border_color(if on { theme.primary } else { theme.input })
                            .bg(if on { theme.primary } else { theme.background })
                            .when(on, |this| {
                                this.child(
                                    svg()
                                        .path("icons/check.svg")
                                        .size(px(14.))
                                        .text_color(theme.primary_foreground),
                                )
                            }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(status_label(status)),
                    )
                    .child(
                        div()
                            .text_color(theme.muted_foreground)
                            .child(self.library.view.status_count(status).to_string()),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.change_library_view(cx, |state| state.toggle_status(status));
                    }))
            })
            .collect::<Vec<_>>();
        deferred(self.library_popup(230., false, cx).children(rows)).with_priority(1)
    }

    fn render_sort_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let state = &self.library.state;
        let (current, direction) = (state.sort, state.direction);
        let first = current.first_direction();
        let second = match first {
            SortDirection::Ascending => SortDirection::Descending,
            SortDirection::Descending => SortDirection::Ascending,
        };
        let keys = SortKey::ALL
            .into_iter()
            .enumerate()
            .map(|(index, sort)| {
                self.popup_row(("library-sort-key", index))
                    .child(div().flex_1().min_w_0().truncate().child(sort_label(sort)))
                    .child(self.popup_tick(sort == current))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.change_library_view(cx, |state| state.set_sort(sort));
                        this.close_library_menu(cx);
                    }))
            })
            .collect::<Vec<_>>();
        let directions = [first, second]
            .into_iter()
            .enumerate()
            .map(|(index, choice)| {
                self.popup_row(("library-sort-direction", index))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(direction_label(current, choice)),
                    )
                    .child(self.popup_tick(choice == direction))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.change_library_view(cx, |state| state.direction = choice);
                        this.close_library_menu(cx);
                    }))
            })
            .collect::<Vec<_>>();
        deferred(
            self.library_popup(200., true, cx)
                .children(keys)
                .child(div().h(px(1.)).my(px(4.)).bg(theme.border))
                .children(directions),
        )
        .with_priority(1)
    }

    fn render_import_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        deferred(
            self.library_popup(180., true, cx)
                .child(
                    self.popup_row("library-import-video")
                        .child(t!("Import video"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.close_library_menu(cx);
                            cx.defer(crate::import::pick_and_import_video);
                        })),
                )
                .child(
                    self.popup_row("library-import-image")
                        .child(t!("Import image"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.close_library_menu(cx);
                            cx.defer(crate::import::pick_and_import_image);
                        })),
                ),
        )
        .with_priority(1)
    }

    fn render_library_filters(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let theme = self.theme;
        let held = self.library.state.statuses.clone();
        if held.is_empty() {
            return None;
        }
        Some(
            div()
                .flex()
                .flex_shrink_0()
                .items_center()
                .gap(px(6.))
                .py(px(6.))
                .px(px(PADDING))
                .border_b_1()
                .border_color(theme.border)
                .text_size(px(12.))
                .children(held.into_iter().enumerate().map(|(index, status)| {
                    focus_ring(
                        div()
                            .id(("library-filter-tag", index))
                            .tab_index(0)
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .h(px(24.))
                            .px(px(8.))
                            .rounded_full()
                            .bg(theme.selection)
                            .text_color(theme.selection_foreground)
                            .child(status_label(status))
                            .child(
                                svg()
                                    .path("icons/x.svg")
                                    .size(px(14.))
                                    .text_color(theme.selection_foreground),
                            ),
                        theme,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.change_library_view(cx, |state| state.toggle_status(status));
                    }))
                }))
                .child(
                    focus_ring(
                        div()
                            .id("library-clear-filters")
                            .tab_index(0)
                            .px(px(8.))
                            .rounded(px(4.))
                            .text_color(theme.link)
                            .child(t!("Clear filters")),
                        theme,
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.clear_library_filters(cx))),
                ),
        )
    }

    fn shows_list_head(&self) -> bool {
        let page = &self.library;
        page.state.view == ViewMode::List
            && page.entries.is_some()
            && page.surface == Some(Surface::Items)
    }

    fn render_list_head(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let state = &self.library.state;
        let (sort, direction) = (state.sort, state.direction);
        let column = |key: Option<SortKey>, this: &Self, cx: &mut Context<Self>| {
            let Some(key) = key else {
                return div()
                    .id("library-column-type")
                    .child(column_label(None))
                    .into_any_element();
            };
            let on = key == sort;
            let chevron = match direction {
                SortDirection::Ascending => "icons/chevron-up.svg",
                SortDirection::Descending => "icons/chevron-down.svg",
            };
            focus_ring(
                div()
                    .id(("library-column", key as usize))
                    .tab_index(0)
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .rounded(px(4.))
                    .when(on, |this| this.text_color(theme.foreground))
                    .hover(move |style| style.text_color(theme.foreground))
                    .child(column_label(Some(key)))
                    .when(on, |this| {
                        this.child(
                            svg()
                                .path(chevron)
                                .size(px(14.))
                                .text_color(theme.foreground),
                        )
                    }),
                this.theme,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.change_library_view(cx, |state| state.sort_by_column(key));
            }))
            .into_any_element()
        };
        let name = column(Some(SortKey::Name), self, cx);
        div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap(px(LIST_GAP))
            .h(px(LIST_ROW))
            .mx(px(PADDING))
            .px(px(LIST_GAP))
            .text_size(px(12.))
            .text_color(theme.muted_foreground)
            .child(div().flex_1().min_w_0().child(name))
            .children(LIST_COLUMNS.into_iter().map(|(key, width)| {
                div()
                    .flex_shrink_0()
                    .w(px(width))
                    .child(column(key, self, cx))
            }))
    }

    fn render_library_items(&self, cx: &mut Context<Self>) -> AnyElement {
        self.library.drawn.borrow_mut().clear();
        let in_list = self.library.state.view == ViewMode::List;
        let body = focus_ring(
            div()
                .id("library-items")
                .key_context("LibraryItems")
                .track_focus(&self.library.items_focus)
                .relative()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0(),
            self.theme,
        )
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, event: &MouseDownEvent, _, cx| {
                this.close_library_menu(cx);
                this.begin_library_marquee(event, cx);
            }),
        )
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(|this, event: &MouseDownEvent, window, cx| {
                window.focus(&this.library.items_focus, cx);
                this.open_library_blank_menu(event.position, cx);
            }),
        )
        .on_scroll(cx.listener(|this, event: &gpui::ScrollEvent, _, cx| {
            if event.modifiers.control {
                let delta = event.delta.pixel_delta(px(20.));
                if delta.y != px(0.) {
                    this.step_library_tiles(if delta.y > px(0.) { 1 } else { -1 }, cx);
                }
            }
        }));
        if self.library.entries.is_none() {
            return body.child(self.render_library_loading()).into_any_element();
        }
        match self.library.surface {
            Some(Surface::Empty) => body.child(self.render_library_empty(cx)).into_any_element(),
            Some(Surface::NoMatch) => body
                .child(self.render_library_no_match(cx))
                .into_any_element(),
            _ => {
                let top = self.library.list.logical_scroll_top();
                let sticky = view::header_above(&self.library.rows, top.item_ix).filter(|_| {
                    top.offset_in_item > px(0.)
                        || !matches!(self.library.rows.get(top.item_ix), Some(Row::Header(_)))
                });
                body.child(
                    list(
                        self.library.list.clone(),
                        cx.processor(|this, index: usize, window, cx| {
                            this.render_library_row(index, window, cx)
                        }),
                    )
                    .flex_1()
                    .size_full(),
                )
                .children(sticky.filter(|_| !in_list).map(|group| {
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .child(self.render_library_header(group))
                }))
                .into_any_element()
            }
        }
    }

    fn render_library_loading(&self) -> impl IntoElement {
        let theme = self.theme;
        div()
            .flex()
            .flex_wrap()
            .gap(px(TILE_GAP))
            .p(px(PADDING))
            .children((0..6).map(move |_| {
                div()
                    .w(px(self.library.state.tile_size))
                    .h(px(self.library.state.tile_size * 9. / 16.))
                    .rounded(px(6.))
                    .bg(theme.muted)
            }))
    }

    fn library_surface(
        &self,
        art: &'static doodle::Doodle,
        width: f32,
        text: &'static str,
    ) -> gpui::Div {
        let theme = self.theme;
        div()
            .flex()
            .flex_col()
            .flex_1()
            .items_center()
            .justify_center()
            .gap(px(12.))
            .text_size(px(13.))
            .text_color(theme.muted_foreground)
            .child(doodle::doodle_once(
                art,
                doodle::Palette::new(&theme),
                self.library.surface_shown_at,
                self.library.surface_still,
                width,
            ))
            .child(text)
    }

    fn surface_button(&self, id: &'static str, label: &'static str) -> gpui::Stateful<gpui::Div> {
        focus_ring(
            div()
                .id(id)
                .tab_index(0)
                .flex()
                .items_center()
                .h(px(crate::theme::CONTROL_HEIGHT))
                .px(px(12.))
                .rounded(px(6.))
                .child(label),
            self.theme,
        )
    }

    fn render_library_empty(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        self.library_surface(
            &onboarding_doodles::FRAME,
            96.,
            t!("Recordings you make appear here."),
        )
        .child(
            self.surface_button("library-empty-start", t!("Start recording"))
                .bg(theme.primary)
                .text_color(theme.primary_foreground)
                .hover(move |style| style.bg(theme.primary_hover))
                .on_click(cx.listener(|this, _, window, cx| {
                    this.collapse_library(window, cx);
                    this.start_from_chosen_target(cx);
                })),
        )
    }

    fn render_library_no_match(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        self.library_surface(&onboarding_doodles::LENS, 76., t!("No matching items"))
            .child(
                self.surface_button("library-no-match-clear", t!("Clear filters"))
                    .bg(theme.secondary)
                    .text_color(theme.foreground)
                    .hover(move |style| style.bg(theme.accent))
                    .on_click(cx.listener(|this, _, _, cx| this.clear_library_filters(cx))),
            )
    }

    fn render_library_header(&self, group: DateGroup) -> impl IntoElement {
        let theme = self.theme;
        div()
            .w_full()
            .pt(px(10.))
            .pb(px(6.))
            .px(px(PADDING + 2.))
            .bg(theme.background)
            .text_size(px(12.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(theme.muted_foreground)
            .child(group_label(group))
    }

    fn render_library_row(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(row) = self.library.rows.get(index).cloned() else {
            return div().into_any_element();
        };
        match row {
            Row::FirstLine => div()
                .px(px(PADDING))
                .pt(px(10.))
                .pb(px(2.))
                .text_size(px(13.))
                .text_color(self.theme.foreground)
                .child(t!("Your recordings stay on this computer."))
                .into_any_element(),
            Row::Header(group) => self.render_library_header(group).into_any_element(),
            Row::Imports(imports) => {
                let snapshot = crate::import::imports_snapshot(cx);
                let cards = imports
                    .iter()
                    .filter_map(|index| {
                        snapshot
                            .get(*index)
                            .map(|entry| self.render_import_card(*index, entry).into_any_element())
                    })
                    .collect();
                self.library_row(cards, Vec::new())
            }
            Row::Tiles(items) => {
                let tile_width = self.library_tile_width(window);
                let mut paths = Vec::new();
                let cards = items
                    .iter()
                    .filter_map(|index| {
                        let entry = self.library.entries.as_ref()?.get(*index)?.clone();
                        paths.push(entry.path.clone());
                        Some(self.render_library_tile(&entry, tile_width, window, cx))
                    })
                    .collect();
                self.library_row(cards, paths)
            }
            Row::Item(index) => {
                let Some(entry) = self
                    .library
                    .entries
                    .as_ref()
                    .and_then(|entries| entries.get(index))
                    .cloned()
                else {
                    return div().into_any_element();
                };
                let path = entry.path.clone();
                self.record_drawn(div().px(px(PADDING)), vec![path])
                    .child(self.render_library_list_row(&entry, window, cx))
                    .into_any_element()
            }
        }
    }

    fn library_tile_width(&self, window: &Window) -> f32 {
        let columns = self.library.columns.max(1) as f32;
        let content =
            f32::from(window.viewport_size().width) - self.library.panel_width() - PADDING * 2.;
        (content - TILE_GAP * (columns - 1.)) / columns
    }

    fn record_drawn(&self, row: gpui::Div, paths: Vec<PathBuf>) -> gpui::Div {
        let drawn = self.library.drawn.clone();
        row.on_children_prepainted(move |bounds, _, _| {
            let mut drawn = drawn.borrow_mut();
            for (path, bounds) in paths.iter().zip(bounds) {
                drawn.insert(path.clone(), bounds);
            }
        })
    }

    fn library_row(&self, mut cards: Vec<AnyElement>, paths: Vec<PathBuf>) -> AnyElement {
        let columns = self.library.columns.max(1);
        while cards.len() < columns {
            cards.push(div().flex_1().min_w_0().into_any_element());
        }
        self.record_drawn(
            div()
                .flex()
                .w_full()
                .gap(px(TILE_GAP))
                .px(px(PADDING))
                .pb(px(TILE_GAP)),
            paths,
        )
        .children(cards)
        .into_any_element()
    }

    fn selection_look(
        &self,
        item: gpui::Stateful<gpui::Div>,
        path: &Path,
        window: &Window,
    ) -> gpui::Stateful<gpui::Div> {
        let theme = self.theme;
        let selected = self.library.selection.contains(path);
        let lead = self.library.selection.lead() == Some(path)
            && self.library.items_focus.is_focused(window);
        item.border_1()
            .border_color(transparent_black())
            .when(selected, |this| {
                this.bg(theme.selection).border_color(theme.selection)
            })
            .when(!selected, |this| {
                this.hover(move |style| style.bg(theme.accent))
            })
            .when(lead, |this| this.border_color(theme.ring))
    }

    fn render_library_list_row(
        &self,
        entry: &Entry,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = self.theme;
        let selected = self.library.selection.contains(&entry.path);
        let clock = Clock::now();
        let job = self.library.compress.state(&entry.path);
        let cells: [SharedString; 4] = [
            job.map_or(kind_label(entry.kind), job_label).into(),
            entry
                .duration
                .map_or_else(|| "—".to_string(), view::format_clock)
                .into(),
            match job {
                Some(JobState::Running { percent }) => format!("{percent}%"),
                _ => view::format_size(entry.size),
            }
            .into(),
            day_label(&clock, entry.created_millis),
        ];
        let row = div()
            .id(library::row_id(&entry.path))
            .flex()
            .items_center()
            .gap(px(LIST_GAP))
            .h(px(LIST_ROW))
            .px(px(LIST_GAP))
            .rounded(px(6.))
            .cursor_pointer()
            .text_size(px(13.))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .gap(px(8.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(if selected {
                        theme.selection_foreground
                    } else {
                        theme.foreground
                    })
                    .child(
                        svg()
                            .path(kind_icon(entry.kind))
                            .size(px(16.))
                            .flex_shrink_0()
                            .text_color(theme.muted_foreground),
                    )
                    .child(div().min_w_0().truncate().child(entry.name.clone()))
                    .when(entry.exported(), |this| {
                        this.child(
                            svg()
                                .path("icons/check.svg")
                                .size(px(14.))
                                .flex_shrink_0()
                                .text_color(theme.muted_foreground),
                        )
                    }),
            )
            .children(
                LIST_COLUMNS
                    .into_iter()
                    .zip(cells)
                    .map(|((_, width), cell)| {
                        div()
                            .flex_shrink_0()
                            .w(px(width))
                            .truncate()
                            .text_size(px(12.))
                            .text_color(theme.muted_foreground)
                            .child(cell)
                    }),
            );
        let row = self.selection_look(row, &entry.path, window);
        self.library_item_events(row, &entry.path, cx)
    }

    fn render_library_tile(
        &mut self,
        entry: &Entry,
        tile_width: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let fresh = view::is_fresh(&self.library.fresh, &entry.path);
        let image = self.library_thumbnail(entry, window, cx);
        let thumb_height = ((tile_width - TILE_PADDING * 2. - 2.) * 9. / 16.).max(1.);
        let mut meta = format!(
            "{} · {}",
            kind_label(entry.kind),
            view::format_size(entry.size)
        );
        if entry.clip_count > 1 {
            meta = format!("{meta} · {}", t!("{count} clips", count = entry.clip_count));
        }
        let (scrub_frame, scrub_line) = self
            .library_scrub_of(entry)
            .map_or((None, None), |(frame, line)| (frame, Some(line)));
        let image = scrub_frame.or(image);
        let job = self.library.compress.state(&entry.path);
        if let Some(job) = job {
            meta = match job {
                JobState::Waiting => job_label(job).to_string(),
                JobState::Running { percent } => t!("Compressing… {percent}%", percent = percent),
            };
        }
        let selected = self.library.selection.contains(&entry.path);
        let tile = self.selection_look(
            div()
                .id(library::row_id(&entry.path))
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .p(px(TILE_PADDING))
                .rounded(px(6.))
                .cursor_pointer()
                .when(fresh && !selected, |this| this.bg(theme.accent)),
            &entry.path,
            window,
        );

        let (hovered, left) = (entry.path.clone(), entry.path.clone());
        self.library_item_events(tile, &entry.path, cx)
            .child(
                div()
                    .id("library-thumb")
                    .relative()
                    .w_full()
                    .h(px(thumb_height))
                    .when(entry.kind != MediaKind::Screenshot, |this| {
                        this.on_mouse_move(cx.listener(
                            move |this, event: &MouseMoveEvent, window, cx| {
                                this.scrub_library_tile(&hovered, event.position.x, window, cx);
                            },
                        ))
                        .on_hover(cx.listener(
                            move |this, hovered: &bool, _, cx| {
                                if !hovered {
                                    this.leave_library_tile(&left, cx);
                                }
                            },
                        ))
                    })
                    .rounded(px(6.))
                    .overflow_hidden()
                    .bg(theme.muted)
                    .child(match image {
                        Some(image) => img(image)
                            .size_full()
                            .object_fit(gpui::ObjectFit::Cover)
                            .into_any_element(),
                        None => div()
                            .flex()
                            .size_full()
                            .items_center()
                            .justify_center()
                            .child(
                                svg()
                                    .path(kind_icon(entry.kind))
                                    .size(px(24.))
                                    .text_color(theme.muted_foreground),
                            )
                            .into_any_element(),
                    })
                    .child(
                        tile_corner(&theme).top(px(4.)).left(px(4.)).child(
                            svg()
                                .path(kind_icon(entry.kind))
                                .size(px(14.))
                                .text_color(gpui::white()),
                        ),
                    )
                    .when(entry.exported(), |this| {
                        this.child(
                            tile_corner(&theme).top(px(4.)).right(px(4.)).child(
                                svg()
                                    .path("icons/check.svg")
                                    .size(px(14.))
                                    .text_color(gpui::white()),
                            ),
                        )
                    })
                    .when_some(entry.duration, |this, duration| {
                        this.child(
                            tile_corner(&theme)
                                .bottom(px(4.))
                                .right(px(4.))
                                .child(view::format_clock(duration)),
                        )
                    })
                    .when(fresh, |this| {
                        this.child(
                            div()
                                .absolute()
                                .left(px(4.))
                                .bottom(px(4.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .size(px(24.))
                                .rounded(px(3.))
                                .bg(theme.popover)
                                .child(doodle::doodle_once(
                                    &onboarding_doodles::TICK,
                                    doodle::Palette::new(&theme),
                                    self.library.fresh_shown_at,
                                    false,
                                    20.,
                                )),
                        )
                    })
                    .when_some(scrub_line.filter(|_| job.is_none()), |this, line| {
                        this.child(
                            div()
                                .absolute()
                                .left_0()
                                .bottom_0()
                                .h(px(2.))
                                .w(gpui::relative(line))
                                .bg(theme.fixed_white),
                        )
                    })
                    .when_some(job, |this, job| {
                        let percent = match job {
                            JobState::Waiting => 0,
                            JobState::Running { percent } => percent,
                        };
                        this.child(
                            div()
                                .absolute()
                                .left_0()
                                .right_0()
                                .bottom_0()
                                .h(px(JOB_BAR))
                                .bg(theme.muted)
                                .child(
                                    div()
                                        .h_full()
                                        .w(gpui::relative(percent as f32 / 100.))
                                        .bg(theme.primary),
                                ),
                        )
                    }),
            )
            .child(
                div()
                    .mt(px(6.))
                    .w_full()
                    .truncate()
                    .text_size(px(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(if selected {
                        theme.selection_foreground
                    } else {
                        theme.foreground
                    })
                    .child(entry.name.clone()),
            )
            .child(
                div()
                    .w_full()
                    .truncate()
                    .text_size(px(12.))
                    .text_color(theme.muted_foreground)
                    .child(meta),
            )
            .into_any_element()
    }

    fn render_library_status(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let page = &self.library;
        let has_items = page
            .entries
            .as_ref()
            .is_some_and(|entries| !entries.is_empty());
        let track = page.slider.clone();
        div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap(px(12.))
            .h(px(32.))
            .px(px(PADDING))
            .border_t_1()
            .border_color(theme.border)
            .text_size(px(12.))
            .text_color(theme.muted_foreground)
            .child(shown_label(page.view.shown, page.view.shown_size))
            .child(div().flex_1())
            .when(has_items && page.state.view == ViewMode::Grid, |this| {
                this.child(
                    ui::Slider::new("library-tile-size", page.state.tile_fraction(), track)
                        .row_width(px(110.))
                        .track(px(4.), theme.muted.into())
                        .fill(theme.primary.into())
                        .thumb(px(14.), theme.foreground.into(), None)
                        .on_drag_start(cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                            this.library.slider_dragging = true;
                            this.drag_library_tiles(event.position.x, cx);
                        })),
                )
            })
    }

    fn drag_library_tiles(&mut self, x: gpui::Pixels, cx: &mut Context<Self>) {
        let Some(fraction) = self
            .library
            .slider
            .get()
            .and_then(|track| ui::fraction_from_x(x, track))
        else {
            return;
        };
        let size = ui::snap_to_step(
            ui::value_from_fraction(fraction, view::TILE_MIN, view::TILE_MAX),
            view::TILE_MIN,
            view::TILE_MAX,
            view::TILE_STEP,
        );
        if size != self.library.state.tile_size {
            self.resize_library_tiles(size, cx);
        }
    }

    pub(super) fn render_library_slider_layer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        ui::Slider::drag_layer(
            "library-tile-drag",
            cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                this.drag_library_tiles(event.position.x, cx);
            }),
            cx.listener(|this, _: &gpui::MouseUpEvent, _, cx| {
                this.library.slider_dragging = false;
                cx.notify();
            }),
        )
    }
}

pub(super) fn library_badge(theme: &Theme, count: usize) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_center()
        .min_w(px(16.))
        .h(px(16.))
        .px(px(4.))
        .rounded_full()
        .bg(theme.primary)
        .text_size(px(11.))
        .text_color(theme.primary_foreground)
        .child(count.to_string())
}

fn tile_corner(theme: &Theme) -> gpui::Div {
    div()
        .absolute()
        .flex()
        .items_center()
        .h(px(18.))
        .px(px(4.))
        .rounded(px(3.))
        .bg(theme.badge)
        .text_size(px(11.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(gpui::white())
}

fn open_entry(entry: &Entry, cx: &mut gpui::App) {
    let path = entry.path.clone();
    match (&entry.screenshot, entry.kind) {
        (Some(png), _) => {
            let png = png.clone();
            cx.defer(move |cx| app_windows::open_screenshot_editor(png, cx));
        }
        (None, _) if entry.opens_editor => {
            cx.defer(move |cx| app_windows::open_editor(path, cx));
        }
        _ => {}
    }
}
