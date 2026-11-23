use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use gpui::{
    App, AppContext as _, Bounds, Context, Empty, FocusHandle, FontWeight, Global,
    InteractiveElement, IntoElement, ParentElement, Pixels, Point, Render, RenderImage,
    SharedString, StatefulInteractiveElement, Styled, StyledImage as _, Task, Window, WindowBounds,
    WindowHandle, WindowKind, WindowOptions, div, img, point, prelude::FluentBuilder, px, size,
    svg,
};

use crate::{
    app_windows,
    i18n::{t, tc},
    library::{self, Deleted, MediaKind, view::Entry},
    platform,
    theme::{CONTROL_HEIGHT, Theme},
    ui,
};

const CARD_WIDTH: f32 = 300.;
const THUMB_HEIGHT: f32 = 169.;
const BODY_HEIGHT: f32 = 76.;
const BORDER: f32 = 1.;
const GAP: f32 = 8.;
const EDGE: f32 = 16.;
const MAX_CARDS: usize = 3;
const HOLD: Duration = Duration::from_secs(8);
const THUMB_DECODE: (u32, u32) = (600, 338);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardKind {
    Studio,
    Instant,
    Screenshot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Act {
    Edit,
    Open,
    Copy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Job {
    Copy,
    Delete,
    Restore,
}

#[derive(Clone, Debug, PartialEq)]
enum CardState {
    Ready,
    Working(Job),
    Done(Job),
    Failed(Job),
    Deleted(Vec<Deleted>),
}

impl CardKind {
    fn from_media(kind: MediaKind) -> Self {
        match kind {
            MediaKind::Studio => Self::Studio,
            MediaKind::Instant => Self::Instant,
            MediaKind::Screenshot => Self::Screenshot,
        }
    }

    fn acts(self) -> &'static [Act] {
        match self {
            Self::Studio => &[Act::Edit],
            Self::Instant => &[Act::Open, Act::Copy],
            Self::Screenshot => &[Act::Edit, Act::Copy],
        }
    }

    fn drags(self) -> bool {
        self != Self::Studio
    }

    fn reveal_label(self) -> &'static str {
        match self {
            Self::Studio => tc!("result card", "Show in library"),
            Self::Instant | Self::Screenshot => t!("Show in folder"),
        }
    }
}

fn meta_line(
    kind: CardKind,
    seconds: Option<f64>,
    bytes: Option<u64>,
    pixels: Option<(u32, u32)>,
) -> String {
    let clock = seconds.map(library::view::format_clock);
    match kind {
        CardKind::Studio => clock.unwrap_or_default(),
        CardKind::Instant => [clock, bytes.map(library::view::format_size)]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" · "),
        CardKind::Screenshot => pixels
            .map(|(width, height)| format!("{width} × {height}"))
            .unwrap_or_default(),
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Item {
    kind: CardKind,
    bundle: PathBuf,
    file: Option<PathBuf>,
    name: SharedString,
    meta: SharedString,
    chip: Option<SharedString>,
    thumbnail: Option<PathBuf>,
}

impl Item {
    fn read(entry: &Entry) -> Self {
        let kind = CardKind::from_media(entry.kind);
        let file = match kind {
            CardKind::Studio => None,
            CardKind::Instant => Some(library::instant_output_path(&entry.path)),
            CardKind::Screenshot => entry.screenshot.clone(),
        };
        let bytes = file
            .as_deref()
            .and_then(|file| std::fs::metadata(file).ok())
            .map(|metadata| metadata.len());
        let pixels = entry
            .screenshot
            .as_deref()
            .and_then(|png| image::image_dimensions(png).ok());
        Self {
            kind,
            bundle: entry.path.clone(),
            name: entry.name.clone().into(),
            meta: meta_line(kind, entry.duration, bytes, pixels).into(),
            chip: entry
                .duration
                .filter(|_| kind != CardKind::Screenshot)
                .map(|seconds| library::view::format_clock(seconds).into()),
            thumbnail: entry.thumbnail.clone(),
            file,
        }
    }
}

struct Card {
    id: u64,
    item: Item,
    image: Option<Arc<RenderImage>>,
    state: CardState,
    since: Instant,
    hovered: bool,
}

impl Card {
    fn holds(&self) -> bool {
        self.hovered || matches!(self.state, CardState::Working(_) | CardState::Failed(_))
    }

    fn left(&self, now: Instant) -> Option<Duration> {
        (!self.holds()).then(|| HOLD.saturating_sub(now.saturating_duration_since(self.since)))
    }
}

#[derive(Default)]
struct Stack {
    cards: Vec<Card>,
    next_id: u64,
}

impl Stack {
    fn push(&mut self, item: Item, now: Instant) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.cards.push(Card {
            id,
            item,
            image: None,
            state: CardState::Ready,
            since: now,
            hovered: false,
        });
        if self.cards.len() > MAX_CARDS {
            self.cards.remove(0);
        }
        id
    }

    fn card_mut(&mut self, id: u64) -> Option<&mut Card> {
        self.cards.iter_mut().find(|card| card.id == id)
    }

    fn set_state(&mut self, id: u64, state: CardState, now: Instant) {
        if let Some(card) = self.card_mut(id) {
            card.state = state;
            card.since = now;
        }
    }

    fn leave_all(&mut self, now: Instant) -> bool {
        let mut left = false;
        for card in self.cards.iter_mut().filter(|card| card.hovered) {
            card.hovered = false;
            card.since = now;
            left = true;
        }
        left
    }

    fn set_hovered(&mut self, id: u64, hovered: bool, now: Instant) {
        if let Some(card) = self.card_mut(id)
            && card.hovered != hovered
        {
            card.hovered = hovered;
            card.since = now;
        }
    }

    fn remove(&mut self, id: u64) {
        self.cards.retain(|card| card.id != id);
    }

    fn prune(&mut self, now: Instant) {
        self.cards
            .retain(|card| card.left(now).is_none_or(|left| !left.is_zero()));
    }

    fn next_wake(&self, now: Instant) -> Option<Duration> {
        self.cards.iter().filter_map(|card| card.left(now)).min()
    }
}

fn stack_height(cards: usize) -> f32 {
    if cards == 0 {
        return 0.;
    }
    let full = THUMB_HEIGHT + BODY_HEIGHT + BORDER * 2.;
    let small = BODY_HEIGHT + BORDER * 2.;
    full + (cards - 1) as f32 * (small + GAP)
}

fn stack_bounds(work_area: Bounds<Pixels>, cards: usize) -> Bounds<Pixels> {
    let width = px(CARD_WIDTH);
    let height = px(stack_height(cards));
    Bounds {
        origin: point(
            work_area.right() - px(EDGE) - width,
            work_area.bottom() - px(EDGE) - height,
        ),
        size: size(width, height),
    }
}

struct Cards(WindowHandle<ResultCardView>);

impl Global for Cards {}

pub fn show(bundle: PathBuf, display: Option<scap_targets::Display>, cx: &mut App) {
    cx.spawn(async move |cx| {
        let read = cx
            .background_executor()
            .spawn(async move {
                let entry = library::entry_at(&bundle)?;
                let item = Item::read(&entry);
                let image = item.thumbnail.as_deref().and_then(|path| match item.kind {
                    CardKind::Screenshot => library::decode_still(path, THUMB_DECODE),
                    CardKind::Studio | CardKind::Instant => library::decode_thumbnail(path),
                });
                Some((item, image))
            })
            .await;
        let Some((item, image)) = read else {
            tracing::warn!("the result card has nothing to show for this project");
            return;
        };
        cx.update(|cx| add(item, image, display, cx));
    })
    .detach();
}

fn add(
    item: Item,
    image: Option<Arc<RenderImage>>,
    display: Option<scap_targets::Display>,
    cx: &mut App,
) {
    let open_handle = cx.try_global::<Cards>().map(|Cards(handle)| *handle);
    if let Some(handle) = open_handle
        && handle
            .update(cx, |view, window, cx| {
                view.add(item.clone(), image.clone(), window, cx)
            })
            .is_ok()
    {
        return;
    }
    open(item, image, display, cx);
}

fn open(
    item: Item,
    image: Option<Arc<RenderImage>>,
    display: Option<scap_targets::Display>,
    cx: &mut App,
) {
    let display = display
        .or_else(scap_targets::Display::get_containing_cursor)
        .unwrap_or_else(scap_targets::Display::primary);
    let Some(work_area) = app_windows::display_work_area(Some(&display), cx) else {
        tracing::warn!("the result card has no display to open on");
        return;
    };
    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(stack_bounds(work_area, 1))),
            display_id: app_windows::gpui_display_id(Some(&display), cx),
            titlebar: None,
            kind: WindowKind::PopUp,
            focus: false,
            show: false,
            is_resizable: false,
            is_minimizable: false,
            window_background: gpui::WindowBackgroundAppearance::Transparent,
            ..Default::default()
        },
        |window, cx| cx.new(|cx| ResultCardView::new(item, image, window, cx)),
    );
    let handle = match opened {
        Ok(handle) => handle,
        Err(error) => {
            tracing::error!("the result card window failed to open: {error:#}");
            return;
        }
    };
    cx.set_global(Cards(handle));
    let shown_on = display.id();
    tracing::info!(display = %shown_on, "result card shown");
    handle
        .update(cx, |_, window, _| {
            if let Some(native) = platform::native_window(window) {
                #[cfg(target_os = "windows")]
                if let Err(error) = platform::prepare_countdown_panel(&native, true) {
                    tracing::warn!("the result card could not be left out of captures: {error}");
                }
                #[cfg(target_os = "macos")]
                platform::set_window_capture_hidden(&native, true);
            }
            platform::show_window_without_focus(window);
        })
        .ok();
}

struct DragGhost;

impl Render for DragGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

struct DraggedFile(PathBuf);

pub struct ResultCardView {
    stack: Stack,
    focus: HashMap<u64, FocusHandle>,
    shown: usize,
    wake: Option<Task<()>>,
}

impl ResultCardView {
    fn new(
        item: Item,
        image: Option<Arc<RenderImage>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        crate::theme::bind_window(window, cx);
        let mut view = Self {
            stack: Stack::default(),
            focus: HashMap::new(),
            shown: 1,
            wake: None,
        };
        let id = view.stack.push(item, Instant::now());
        if let Some(card) = view.stack.card_mut(id) {
            card.image = image;
        }
        view.settle(window, cx);
        view
    }

    fn add(
        &mut self,
        item: Item,
        image: Option<Arc<RenderImage>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let id = self.stack.push(item, Instant::now());
        if let Some(card) = self.stack.card_mut(id) {
            card.image = image;
        }
        self.settle(window, cx);
    }

    fn settle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let now = Instant::now();
        self.stack.prune(now);
        let ids: Vec<u64> = self.stack.cards.iter().map(|card| card.id).collect();
        self.focus.retain(|id, _| ids.contains(id));
        let cards = self.stack.cards.len();
        if cards == 0 {
            if cx.try_global::<Cards>().is_some_and(|Cards(handle)| {
                handle.window_id() == window.window_handle().window_id()
            }) {
                cx.remove_global::<Cards>();
            }
            self.wake = None;
            tracing::info!("result card closed");
            window.remove_window();
            return;
        }
        if cards != self.shown {
            tracing::info!(cards, "result card stack changed");
            self.shown = cards;
            fit_height(window, stack_height(cards), cx);
        }
        self.wake = self.stack.next_wake(now).map(|left| {
            cx.spawn_in(window, async move |this, cx| {
                cx.background_executor().timer(left).await;
                this.update_in(cx, |this, window, cx| this.settle(window, cx))
                    .ok();
            })
        });
        cx.notify();
    }

    fn close(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        self.stack.remove(id);
        self.settle(window, cx);
    }

    fn item(&self, id: u64) -> Option<Item> {
        self.stack
            .cards
            .iter()
            .find(|card| card.id == id)
            .map(|card| card.item.clone())
    }

    fn act(&mut self, id: u64, act: Act, window: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = self.item(id) else {
            return;
        };
        match act {
            Act::Copy => return self.copy(id, item, window, cx),
            Act::Edit => {
                let path = match item.kind {
                    CardKind::Screenshot => item.file.unwrap_or(item.bundle),
                    CardKind::Studio | CardKind::Instant => item.bundle,
                };
                cx.defer(move |cx| match item.kind {
                    CardKind::Screenshot => app_windows::open_screenshot_editor(path, cx),
                    CardKind::Studio | CardKind::Instant => app_windows::open_editor(path, cx),
                });
            }
            Act::Open => {
                let bundle = item.bundle;
                cx.defer(move |cx| app_windows::play_in_library(bundle, cx));
            }
        }
        self.close(id, window, cx);
    }

    fn copy(&mut self, id: u64, item: Item, window: &mut Window, cx: &mut Context<Self>) {
        let Some(file) = item.file else {
            return;
        };
        if item.kind == CardKind::Instant {
            let state = match platform::copy_file_to_clipboard(&file, cx) {
                Ok(()) => CardState::Done(Job::Copy),
                Err(error) => {
                    tracing::warn!("the result card could not copy the recording: {error}");
                    CardState::Failed(Job::Copy)
                }
            };
            self.stack.set_state(id, state, Instant::now());
            return self.settle(window, cx);
        }
        self.stack
            .set_state(id, CardState::Working(Job::Copy), Instant::now());
        self.settle(window, cx);
        let copied = platform::read_image_into_clipboard(file, cx);
        cx.spawn_in(window, async move |this, cx| {
            let state = match copied.await {
                Ok(()) => CardState::Done(Job::Copy),
                Err(error) => {
                    tracing::warn!("the result card could not copy the screenshot: {error}");
                    CardState::Failed(Job::Copy)
                }
            };
            this.update_in(cx, |this, window, cx| {
                this.stack.set_state(id, state, Instant::now());
                this.settle(window, cx);
            })
            .ok();
        })
        .detach();
    }

    fn reveal(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = self.item(id) else {
            return;
        };
        match (item.kind, item.file) {
            (CardKind::Studio, _) | (_, None) => {
                let bundle = item.bundle;
                cx.defer(move |cx| app_windows::open_library(Some(bundle), cx));
            }
            (_, Some(file)) => library::reveal_in_folder(&file),
        }
        self.close(id, window, cx);
    }

    fn delete(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = self.item(id) else {
            return;
        };
        self.stack
            .set_state(id, CardState::Working(Job::Delete), Instant::now());
        self.settle(window, cx);
        cx.spawn_in(window, async move |this, cx| {
            let executor = cx.background_executor().clone();
            let deleted = cx
                .background_executor()
                .spawn(
                    async move { library::recycle_project_retrying(&item.bundle, &executor).await },
                )
                .await;
            this.update_in(cx, |this, window, cx| {
                match deleted {
                    Ok(deleted) => {
                        this.stack.set_state(
                            id,
                            CardState::Deleted(deleted.into_iter().collect()),
                            Instant::now(),
                        );
                    }
                    Err(error) => {
                        tracing::error!("the result card could not delete the project: {error}");
                        this.stack.set_state(id, CardState::Ready, Instant::now());
                        platform::alert_dialog(
                            t!("Couldn't delete it"),
                            t!("Close what has it open and try again."),
                        );
                    }
                }
                this.settle(window, cx);
                cx.defer(app_windows::refresh_library_everywhere);
            })
            .ok();
        })
        .detach();
    }

    fn undo(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(CardState::Deleted(deleted)) = self
            .stack
            .cards
            .iter()
            .find(|card| card.id == id)
            .map(|card| card.state.clone())
        else {
            return;
        };
        self.stack
            .set_state(id, CardState::Working(Job::Restore), Instant::now());
        self.settle(window, cx);
        cx.spawn_in(window, async move |this, cx| {
            let restored = cx
                .background_executor()
                .spawn(async move { library::recycle::undo(&deleted) })
                .await;
            this.update_in(cx, |this, window, cx| {
                match restored {
                    Ok(()) => this.stack.set_state(id, CardState::Ready, Instant::now()),
                    Err(error) => {
                        tracing::error!("restoring from the Recycle Bin failed: {error}");
                        this.stack.remove(id);
                        platform::alert_dialog(
                            t!("Couldn't restore what you deleted"),
                            t!("Open the Recycle Bin and restore it from there."),
                        );
                    }
                }
                this.settle(window, cx);
                cx.defer(app_windows::refresh_library_everywhere);
            })
            .ok();
        })
        .detach();
    }

    fn start_drag(&self, file: &Path, window: &mut Window, cx: &mut Context<Self>) {
        #[cfg(target_os = "windows")]
        {
            let source = platform::native_window(window);
            let files = vec![file.to_path_buf()];
            cx.spawn_in(window, async move |_, cx| {
                if let Err(error) = platform::drag_files_out(source, &files) {
                    tracing::warn!("result card drag-out failed: {error}");
                }
                cx.update(|window, cx| cx.stop_active_drag(window)).ok();
            })
            .detach();
        }
        #[cfg(not(target_os = "windows"))]
        let _ = (file, window, cx);
    }
}

fn fit_height(window: &Window, height: f32, cx: &mut App) {
    let Some(native) = platform::native_window(window) else {
        return;
    };
    let scale = f64::from(window.scale_factor());
    cx.spawn(async move |_| {
        let (x, y, width, current) = platform::window_frame(&native);
        #[cfg(target_os = "windows")]
        {
            let height = f64::from(height) * scale;
            platform::set_window_frame(&native, x, y + current - height, width, height);
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (current, scale);
            platform::set_window_frame(&native, x, y, width, f64::from(height));
        }
    })
    .detach();
}

fn icon_button(
    theme: Theme,
    id: impl Into<gpui::ElementId>,
    icon: &'static str,
    label: &'static str,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(CONTROL_HEIGHT))
        .rounded(px(6.))
        .cursor_pointer()
        .hover(move |style| style.bg(theme.secondary_hover))
        .tooltip(move |_, cx| ui::Tooltip::new(&theme, label).view(cx))
        .tooltip_show_delay(ui::TOOLTIP_SHOW_DELAY)
        .child(svg().path(icon).size(px(16.)).text_color(theme.foreground))
}

fn text_button(
    theme: Theme,
    id: impl Into<gpui::ElementId>,
    icon: Option<&'static str>,
    label: &'static str,
    grow: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .gap(px(6.))
        .h(px(CONTROL_HEIGHT))
        .px(px(10.))
        .rounded(px(6.))
        .when(grow, |this| this.flex_1().min_w_0())
        .bg(theme.secondary)
        .hover(move |style| style.bg(theme.secondary_hover))
        .cursor_pointer()
        .text_color(theme.secondary_foreground)
        .font_weight(FontWeight::MEDIUM)
        .when_some(icon, |this, icon| {
            this.child(
                svg()
                    .path(icon)
                    .size(px(14.))
                    .text_color(theme.secondary_foreground),
            )
        })
        .child(label)
}

fn act_icon(act: Act) -> (&'static str, &'static str) {
    match act {
        Act::Edit => ("icons/pencil.svg", t!("Edit")),
        Act::Open => ("icons/play.svg", t!("Open")),
        Act::Copy => ("icons/copy.svg", t!("Copy")),
    }
}

fn status_text(state: &CardState) -> Option<&'static str> {
    match state {
        CardState::Working(Job::Copy) => Some(t!("Copying")),
        CardState::Working(Job::Delete) => Some(t!("Deleting")),
        CardState::Working(Job::Restore) => Some(t!("Restoring")),
        CardState::Done(Job::Copy) => Some(t!("Copied")),
        CardState::Failed(Job::Copy) => Some(t!("Couldn't copy")),
        CardState::Deleted(_) => Some(t!("Deleted")),
        CardState::Ready
        | CardState::Done(Job::Delete | Job::Restore)
        | CardState::Failed(Job::Delete | Job::Restore) => None,
    }
}

impl ResultCardView {
    fn render_card(
        &mut self,
        index: usize,
        newest: bool,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let card = &self.stack.cards[index];
        let id = card.id;
        let item = card.item.clone();
        let state = card.state.clone();
        let image = card.image.clone();
        let focus = self
            .focus
            .entry(id)
            .or_insert_with(|| cx.focus_handle().tab_index(0).tab_stop(true))
            .clone();

        let close = icon_button(theme, ("close", id), "icons/x.svg", t!("Close"))
            .size(px(24.))
            .bg(gpui::rgba(0x00000099))
            .child(
                svg()
                    .path("icons/x.svg")
                    .size(px(14.))
                    .text_color(gpui::white()),
            )
            .on_click(cx.listener(move |this, _, window, cx| this.close(id, window, cx)));

        let thumbnail = newest.then(|| {
            let drag_file = item.file.clone().filter(|_| item.kind.drags());
            let mut thumb = div()
                .id(("thumb", id))
                .relative()
                .w_full()
                .h(px(THUMB_HEIGHT))
                .overflow_hidden()
                .bg(theme.muted)
                .when_some(image, |this, image| {
                    this.child(img(image).size_full().object_fit(gpui::ObjectFit::Cover))
                })
                .when_some(item.chip.clone(), |this, chip| {
                    this.child(
                        div()
                            .absolute()
                            .left(px(8.))
                            .bottom(px(8.))
                            .px(px(6.))
                            .rounded(px(4.))
                            .bg(gpui::rgba(0x00000099))
                            .text_color(gpui::white())
                            .child(chip),
                    )
                })
                .child(div().absolute().top(px(6.)).right(px(6.)).child(close));
            if let Some(file) = drag_file {
                let view = cx.entity().downgrade();
                let label = t!("Drag to another app");
                thumb = thumb
                    .cursor_grab()
                    .tooltip(move |_, cx| ui::Tooltip::new(&theme, label).view(cx))
                    .tooltip_show_delay(ui::TOOLTIP_SHOW_DELAY)
                    .on_drag(
                        DraggedFile(file),
                        move |dragged: &DraggedFile, _: Point<Pixels>, window, cx| {
                            view.update(cx, |this, cx| this.start_drag(&dragged.0, window, cx))
                                .ok();
                            cx.new(|_| DragGhost)
                        },
                    );
            }
            thumb
        });

        let name = div()
            .flex()
            .items_baseline()
            .gap(px(8.))
            .min_w_0()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .text_size(px(14.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.foreground)
                    .child(item.name.clone()),
            )
            .child(
                div()
                    .flex_none()
                    .text_color(theme.muted_foreground)
                    .child(item.meta.clone()),
            )
            .when(!newest, |this| {
                this.child(
                    icon_button(theme, ("close-small", id), "icons/x.svg", t!("Close"))
                        .size(px(20.))
                        .on_click(
                            cx.listener(move |this, _, window, cx| this.close(id, window, cx)),
                        ),
                )
            });

        let row = match &state {
            CardState::Ready => {
                let acts = item.kind.acts().iter().enumerate().map(|(slot, act)| {
                    let (icon, label) = act_icon(*act);
                    let act = *act;
                    text_button(
                        theme,
                        ("act", id * 4 + slot as u64),
                        Some(icon),
                        label,
                        true,
                    )
                    .on_click(cx.listener(move |this, _, window, cx| this.act(id, act, window, cx)))
                });
                div()
                    .flex()
                    .gap(px(6.))
                    .children(acts)
                    .child(
                        icon_button(
                            theme,
                            ("reveal", id),
                            "icons/folder-open.svg",
                            item.kind.reveal_label(),
                        )
                        .on_click(
                            cx.listener(move |this, _, window, cx| this.reveal(id, window, cx)),
                        ),
                    )
                    .child(
                        icon_button(theme, ("delete", id), "icons/trash.svg", t!("Delete"))
                            .on_click(
                                cx.listener(move |this, _, window, cx| this.delete(id, window, cx)),
                            ),
                    )
                    .into_any_element()
            }
            other => {
                let (color, icon) = match other {
                    CardState::Done(_) => (theme.success_text, Some("icons/circle-check.svg")),
                    CardState::Failed(_) => (theme.danger_text, Some("icons/triangle-alert.svg")),
                    _ => (theme.muted_foreground, None),
                };
                let action = match other {
                    CardState::Done(_) => Some((StatusAction::Reveal, item.kind.reveal_label())),
                    CardState::Failed(_) => Some((StatusAction::Retry, t!("Try again"))),
                    CardState::Deleted(_) => Some((StatusAction::Undo, t!("Undo"))),
                    _ => None,
                };
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .h(px(CONTROL_HEIGHT))
                    .text_color(color)
                    .when_some(icon, |this, icon| {
                        this.child(svg().path(icon).size(px(14.)).text_color(color))
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(status_text(other).unwrap_or_default()),
                    )
                    .when_some(action, |this, (action, label)| {
                        this.child(
                            text_button(theme, ("status", id), None, label, false).on_click(
                                cx.listener(move |this, _, window, cx| match action {
                                    StatusAction::Reveal => this.reveal(id, window, cx),
                                    StatusAction::Retry => this.act(id, Act::Copy, window, cx),
                                    StatusAction::Undo => this.undo(id, window, cx),
                                }),
                            ),
                        )
                    })
                    .into_any_element()
            }
        };

        div()
            .id(("card", id))
            .track_focus(&focus)
            .w(px(CARD_WIDTH))
            .flex_none()
            .overflow_hidden()
            .border_1()
            .border_color(theme.border)
            .rounded(px(10.))
            .bg(theme.popover)
            .text_size(px(12.))
            .text_color(theme.popover_foreground)
            .when(!newest, |this| this.opacity(0.92))
            .on_hover(cx.listener(move |this, hovered: &bool, window, cx| {
                this.stack.set_hovered(id, *hovered, Instant::now());
                this.settle(window, cx);
            }))
            .on_key_down(
                cx.listener(move |this, event: &gpui::KeyDownEvent, window, cx| {
                    if event.keystroke.key == "escape" {
                        cx.stop_propagation();
                        this.close(id, window, cx);
                    }
                }),
            )
            .children(thumbnail)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .h(px(BODY_HEIGHT))
                    .p(px(10.))
                    .child(name)
                    .child(row),
            )
    }
}

#[derive(Clone, Copy)]
enum StatusAction {
    Reveal,
    Retry,
    Undo,
}

impl Render for ResultCardView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::for_window(window, cx, false);
        if !window.is_window_hovered() && self.stack.leave_all(Instant::now()) {
            cx.defer_in(window, |this, window, cx| this.settle(window, cx));
        }
        let count = self.stack.cards.len();
        let cards: Vec<_> = (0..count)
            .map(|index| self.render_card(index, index + 1 == count, theme, cx))
            .collect();
        div()
            .size_full()
            .flex()
            .flex_col()
            .justify_end()
            .gap(px(GAP))
            .font_family(crate::theme::ui_font())
            .children(cards)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(kind: CardKind, name: &str) -> Item {
        Item {
            kind,
            bundle: PathBuf::from(format!("{name}.ever")),
            file: None,
            name: name.to_string().into(),
            meta: SharedString::default(),
            chip: None,
            thumbnail: None,
        }
    }

    fn names(stack: &Stack) -> Vec<&str> {
        stack
            .cards
            .iter()
            .map(|card| card.item.name.as_ref())
            .collect()
    }

    #[test]
    fn each_kind_offers_its_two_likeliest_next_steps() {
        assert_eq!(CardKind::Instant.acts(), &[Act::Open, Act::Copy]);
        assert_eq!(CardKind::Screenshot.acts(), &[Act::Edit, Act::Copy]);
        assert_eq!(CardKind::Studio.acts(), &[Act::Edit]);
    }

    #[test]
    fn a_studio_recording_does_not_drag_because_it_has_no_single_file() {
        assert!(!CardKind::Studio.drags());
        assert!(CardKind::Instant.drags());
        assert!(CardKind::Screenshot.drags());
    }

    #[test]
    fn a_card_goes_after_eight_seconds() {
        let start = Instant::now();
        let mut stack = Stack::default();
        stack.push(item(CardKind::Instant, "Bug report"), start);

        stack.prune(start + Duration::from_secs(7));
        assert_eq!(names(&stack), ["Bug report"]);
        assert_eq!(
            stack.next_wake(start + Duration::from_secs(7)),
            Some(Duration::from_secs(1))
        );

        stack.prune(start + HOLD);
        assert!(stack.cards.is_empty());
    }

    #[test]
    fn a_card_stays_while_hovered_and_counts_again_after_the_pointer_leaves() {
        let start = Instant::now();
        let mut stack = Stack::default();
        let id = stack.push(item(CardKind::Screenshot, "Pricing page"), start);

        stack.set_hovered(id, true, start + Duration::from_secs(5));
        stack.prune(start + Duration::from_secs(60));
        assert_eq!(stack.next_wake(start + Duration::from_secs(60)), None);
        assert_eq!(names(&stack), ["Pricing page"]);

        let left = start + Duration::from_secs(61);
        stack.set_hovered(id, false, left);
        stack.prune(left + Duration::from_secs(7));
        assert_eq!(names(&stack), ["Pricing page"]);
        stack.prune(left + HOLD);
        assert!(stack.cards.is_empty());
    }

    #[test]
    fn a_running_action_and_a_failure_keep_the_card_until_closed() {
        let start = Instant::now();
        let mut stack = Stack::default();
        let working = stack.push(item(CardKind::Screenshot, "Working"), start);
        let failed = stack.push(item(CardKind::Instant, "Failed"), start);
        stack.set_state(working, CardState::Working(Job::Copy), start);
        stack.set_state(failed, CardState::Failed(Job::Copy), start);

        stack.prune(start + Duration::from_secs(600));
        assert_eq!(names(&stack), ["Working", "Failed"]);

        stack.remove(failed);
        assert_eq!(names(&stack), ["Working"]);
    }

    #[test]
    fn done_and_deleted_get_their_own_eight_seconds() {
        let start = Instant::now();
        let mut stack = Stack::default();
        let id = stack.push(item(CardKind::Instant, "Bug report"), start);
        let later = start + Duration::from_secs(6);
        stack.set_state(id, CardState::Deleted(Vec::new()), later);

        stack.prune(start + HOLD);
        assert_eq!(names(&stack), ["Bug report"]);
        stack.prune(later + HOLD);
        assert!(stack.cards.is_empty());
    }

    #[test]
    fn leaving_the_window_lets_every_hovered_card_count_down_again() {
        let start = Instant::now();
        let mut stack = Stack::default();
        let id = stack.push(item(CardKind::Instant, "Bug report"), start);
        stack.set_hovered(id, true, start);
        let left = start + Duration::from_secs(30);

        assert!(stack.leave_all(left));
        assert!(!stack.leave_all(left));
        assert_eq!(stack.next_wake(left), Some(HOLD));
    }

    #[test]
    fn deleting_and_restoring_hold_the_card_while_they_run() {
        let start = Instant::now();
        let mut stack = Stack::default();
        let id = stack.push(item(CardKind::Instant, "Bug report"), start);
        for job in [Job::Delete, Job::Restore] {
            stack.set_state(id, CardState::Working(job), start);
            stack.prune(start + Duration::from_secs(60));
            assert_eq!(names(&stack), ["Bug report"]);
        }
    }

    #[test]
    fn a_fourth_card_replaces_the_oldest() {
        let start = Instant::now();
        let mut stack = Stack::default();
        for name in ["One", "Two", "Three", "Four"] {
            stack.push(item(CardKind::Screenshot, name), start);
        }
        assert_eq!(names(&stack), ["Two", "Three", "Four"]);
    }

    #[test]
    fn the_window_grows_by_one_small_card_per_older_card() {
        assert_eq!(stack_height(0), 0.);
        assert_eq!(stack_height(1), THUMB_HEIGHT + BODY_HEIGHT + 2.);
        assert_eq!(stack_height(3) - stack_height(2), BODY_HEIGHT + 2. + GAP);
    }

    #[test]
    fn the_stack_sits_bottom_right_sixteen_pixels_inside_the_work_area() {
        let work_area = Bounds {
            origin: point(px(1920.), px(0.)),
            size: size(px(1920.), px(1040.)),
        };
        let bounds = stack_bounds(work_area, 2);
        assert_eq!(bounds.right(), px(1920. * 2. - 16.));
        assert_eq!(bounds.bottom(), px(1040. - 16.));
        assert_eq!(bounds.size.width, px(CARD_WIDTH));
        assert_eq!(bounds.size.height, px(stack_height(2)));
    }

    #[test]
    fn the_meta_line_reads_per_kind() {
        assert_eq!(meta_line(CardKind::Studio, Some(222.), None, None), "3:42");
        assert_eq!(
            meta_line(CardKind::Instant, Some(48.), Some(6 * 1024 * 1024), None),
            "0:48 · 6 MB"
        );
        assert_eq!(
            meta_line(CardKind::Screenshot, None, Some(1), Some((1920, 1080))),
            "1920 × 1080"
        );
        assert_eq!(meta_line(CardKind::Instant, None, None, None), "");
    }
}
