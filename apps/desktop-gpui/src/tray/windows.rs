use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use gpui::{App, Global, WindowHandle};
use tray_icon::{
    Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{
        self, IconMenuItem, IsMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem,
        Submenu,
    },
};

use super::{
    Entry, PreviousItem, SurfaceState, TrayItem, build_state_menu, decorate_accelerators,
    handle_item,
    native::{self, StateIcon},
    popup::{self, TrayPopup},
    scan_previous,
};
use crate::{
    app_windows,
    i18n::t,
    main_window::Mode,
    platform,
    session::{Phase, RecordingSession},
};

const TRAY_ID: &str = "cap-gpui-tray";

enum Event {
    Menu(MenuId),
    Click,
    Open,
}

struct NativeMenu {
    menu: Menu,
    actions: HashMap<MenuId, TrayItem>,
}

struct Tray {
    icon: TrayIcon,
    native_menu: NativeMenu,
    mode: Mode,
    previous: Vec<PreviousItem>,
    previous_generation: u64,
    popup: Option<WindowHandle<TrayPopup>>,
    state: SurfaceState,
    icon_state: Option<StateIcon>,
    icon_updated: Instant,
    entries: Vec<Entry>,
    popup_id: Option<scap_targets::WindowId>,
}

impl Global for Tray {}

pub fn init(cx: &mut App) {
    if cx.has_global::<Tray>() {
        return;
    }
    let mode = Mode::from_store();
    let state = state_snapshot(cx);
    let entries = live_entries(cx, mode, &[], &state);
    let popup_result = if std::env::var_os("FRAMEEVER_GPUI_TRAY_FORCE_NATIVE").is_some() {
        Err(anyhow::anyhow!("injected tray popup creation failure"))
    } else {
        popup::create(cx)
    };
    let mut popup = match popup_result {
        Ok(handle) => Some(handle),
        Err(error) => {
            tracing::error!(%error, "tray popup creation failed; using the native menu");
            None
        }
    };
    let popup_id = popup
        .and_then(|handle| {
            handle
                .update(cx, |_, window, _| platform::window_number(window))
                .ok()
                .flatten()
        })
        .and_then(|number| number.to_string().parse().ok());
    if popup.is_some() && popup_id.is_none() {
        tracing::error!("tray popup has no capture window id; using native menu");
        if let Some(handle) = popup.take() {
            handle
                .update(cx, |_, window, _| window.remove_window())
                .ok();
        }
    }
    let tray = match create_tray(mode, state, entries, popup, popup_id) {
        Ok(tray) => tray,
        Err(error) => {
            tracing::error!("failed to create the Windows tray: {error:#}");
            return;
        }
    };
    if let Err(error) = native::install_keyboard_bridge(tray.icon.window_handle()) {
        tracing::error!(%error, "tray keyboard bridge failed");
    }
    let (tx, rx) = flume::unbounded();
    let menu_tx = tx.clone();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        if let Err(error) = menu_tx.send(Event::Menu(event.id)) {
            tracing::error!(%error, "tray menu event could not be delivered");
        }
    }));
    let suppress_next_click = std::sync::atomic::AtomicBool::new(false);
    TrayIconEvent::set_event_handler(Some(move |event| {
        if matches!(&event, TrayIconEvent::DoubleClick { id, button: MouseButton::Left, .. } if id.as_ref() == TRAY_ID)
        {
            suppress_next_click.store(true, std::sync::atomic::Ordering::Relaxed);
            return;
        }
        if activates_tray(&event)
            && suppress_next_click.swap(false, std::sync::atomic::Ordering::Relaxed)
        {
            return;
        }
        let next = if activates_tray(&event) {
            Some(Event::Click)
        } else if opens_tray(&event) {
            Some(Event::Open)
        } else {
            None
        };
        if let Some(event) = next
            && let Err(error) = tx.send(event)
        {
            tracing::error!(%error, "tray click could not be delivered");
        }
    }));
    cx.set_global(tray);
    refresh_menu(cx);
    cx.spawn(async move |cx| {
        while let Ok(event) = rx.recv_async().await {
            cx.update(|cx| {
                if !cx.has_global::<Tray>() { return }
                match event {
                    Event::Menu(id) => {
                        let item = cx.global::<Tray>().native_menu.actions.get(&id).cloned();
                        if let Some(item) = item { handle_item(item, cx); }
                    }
                    Event::Click => { let item = left_click_item(&state_snapshot(cx)); handle_item(item, cx); }
                    Event::Open => {
                        let shown = show_popup(cx);
                        cx.spawn(async move |_| {
                            if let Err(error) = shown.await {
                                tracing::error!(%error, "tray popup could not open safely");
                                platform::alert_dialog(t!("Could not open tray menu"), t!("The tray menu could not open safely. Open FrameEver to use its controls."));
                            }
                        }).detach();
                    }
                }
            });
        }
    }).detach();
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(Duration::from_secs(1)).await;
            cx.update(refresh_menu);
        }
    })
    .detach();
    refresh_previous(cx);
    let session = RecordingSession::global(cx);
    cx.observe(&session, |_, cx| refresh_menu(cx)).detach();
    probe_from_env(cx);
}

fn create_tray(
    mode: Mode,
    state: SurfaceState,
    entries: Vec<Entry>,
    popup: Option<WindowHandle<TrayPopup>>,
    popup_id: Option<scap_targets::WindowId>,
) -> anyhow::Result<Tray> {
    let native_menu = build_native_menu(&entries)?;
    let mut builder = TrayIconBuilder::new()
        .with_id(TRAY_ID)
        .with_tooltip("FrameEver")
        .with_icon(app_tray_icon()?)
        .with_menu_on_left_click(false);
    if popup.is_none() {
        builder = builder.with_menu(Box::new(native_menu.menu.clone()));
    }
    Ok(Tray {
        icon: builder.build()?,
        native_menu,
        mode,
        previous: Vec::new(),
        previous_generation: 0,
        popup,
        state,
        icon_state: None,
        icon_updated: Instant::now(),
        entries: Vec::new(),
        popup_id,
    })
}

pub fn popup_window_id(cx: &App) -> Option<scap_targets::WindowId> {
    cx.try_global::<Tray>()
        .and_then(|tray| tray.popup_id.clone())
}
fn state_snapshot(cx: &App) -> SurfaceState {
    if app_windows::onboarding_is_open(cx) && !crate::permissions::necessary_granted() {
        return SurfaceState::Setup;
    }
    let session = RecordingSession::global(cx);
    let session = session.read(cx);
    match session.phase {
        Phase::Starting => SurfaceState::Starting(session.countdown_remaining().unwrap_or(0)),
        Phase::Recording { paused } => SurfaceState::Recording {
            paused,
            elapsed: session.elapsed().as_secs(),
            controls_hidden: app_windows::tray_controls_hidden(cx),
        },
        Phase::Stopping => SurfaceState::Recording {
            paused: false,
            elapsed: session.elapsed().as_secs(),
            controls_hidden: app_windows::tray_controls_hidden(cx),
        },
        Phase::Idle => app_windows::tray_export_status(cx)
            .map(|(title, percent)| SurfaceState::Exporting { title, percent })
            .unwrap_or(SurfaceState::Idle),
    }
}

fn live_entries(
    cx: &App,
    mode: Mode,
    previous: &[PreviousItem],
    state: &SurfaceState,
) -> Vec<Entry> {
    let mut entries = build_state_menu(
        mode,
        previous,
        crate::menus::app_version(),
        state,
        app_windows::tray_last_area_available(cx),
    );
    decorate_accelerators(&mut entries, &|action| {
        crate::hotkeys::accelerator(action, cx)
    });
    entries
}

fn left_click_item(state: &SurfaceState) -> TrayItem {
    match state {
        SurfaceState::Starting(_) => TrayItem::CancelCountdown,
        SurfaceState::Recording { .. } => TrayItem::StopRecording,
        SurfaceState::Exporting { .. } => TrayItem::ShowExport,
        SurfaceState::Setup => TrayItem::RequestPermissions,
        SurfaceState::Idle => TrayItem::OpenCap,
    }
}

#[cfg(test)]
fn click_item(recording: bool) -> Option<TrayItem> {
    (!recording).then_some(TrayItem::OpenCap)
}

fn activates_tray(event: &TrayIconEvent) -> bool {
    matches!(event, TrayIconEvent::Click { id, button: MouseButton::Left, button_state: MouseButtonState::Up, .. } if id.as_ref() == TRAY_ID)
}

fn opens_tray(event: &TrayIconEvent) -> bool {
    matches!(event, TrayIconEvent::Click { id, button: MouseButton::Right, button_state: MouseButtonState::Up, .. } if id.as_ref() == TRAY_ID)
}

struct PreparedPopup {
    handle: WindowHandle<TrayPopup>,
    native_window: platform::NativeWindow,
    hwnd: isize,
    bounds: [f64; 4],
    strict_exclusion: bool,
    started: Instant,
}

fn show_popup(cx: &mut App) -> gpui::Task<anyhow::Result<()>> {
    let prepared = match prepare_popup(cx) {
        Ok(Some(prepared)) => prepared,
        Ok(None) => return gpui::Task::ready(Ok(())),
        Err(error) => return gpui::Task::ready(Err(error)),
    };
    cx.spawn(async move |cx| {
        let hwnd = prepared.hwnd as windows_sys::Win32::Foundation::HWND;
        platform::set_window_capture_hidden(&prepared.native_window, true);
        if prepared.strict_exclusion
            && let Err(error) = native::confirmed_exclusion(hwnd)
        {
            platform::hide_native(&prepared.native_window);
            return Err(error.into());
        }
        let bounds = prepared.bounds;
        platform::set_window_frame(
            &prepared.native_window,
            bounds[0],
            bounds[1],
            bounds[2],
            bounds[3],
        );
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow(
                hwnd,
                windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOW,
            );
        }
        if std::env::var_os("FRAMEEVER_GPUI_TRAY_NATIVE_ACTIVATION_ONLY").is_some() {
            unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow(hwnd);
            }
        } else {
            prepared
                .handle
                .update(cx, |_, window, _| window.activate_window())?;
        }
        cx.background_executor()
            .timer(Duration::from_millis(16))
            .await;
        if unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow() } != hwnd {
            platform::hide_native(&prepared.native_window);
            anyhow::bail!("tray popup could not take foreground focus for dismissal");
        }
        prepared
            .handle
            .update(cx, |_, window, _| window.refresh())?;
        tracing::info!(
            elapsed_us = prepared.started.elapsed().as_micros(),
            "tray popup show dispatched"
        );
        Ok(())
    })
}

fn prepare_popup(cx: &mut App) -> anyhow::Result<Option<PreparedPopup>> {
    refresh_menu(cx);
    let tray = cx.global::<Tray>();
    let Some(handle) = tray.popup else {
        return Ok(None);
    };
    let icon = tray
        .icon
        .rect()
        .ok_or_else(|| anyhow::anyhow!("tray icon position unavailable"))?;
    let entries = tray.entries.clone();
    let export_progress = matches!(tray.state, SurfaceState::Exporting { .. });
    let warning = platform::window_capture_exclusion_unavailable_reason()
        .map(|_| t!("This menu may appear in recordings on this desktop.").to_string());
    let height = popup::menu_height(&entries, warning.is_some(), export_progress);
    let (bounds, _) = native::placement(icon, 280., height)?;
    let started = Instant::now();
    let prepared = handle.update(cx, |view, window, cx| -> anyhow::Result<_> {
        let native_window = platform::native_window(window)
            .ok_or_else(|| anyhow::anyhow!("tray popup has no native window"))?;
        let hwnd = platform::window_number(window)
            .ok_or_else(|| anyhow::anyhow!("tray popup has no window id"))?;
        view.reset(window, cx);
        view.opened_at = Some(started);
        Ok((native_window, hwnd))
    })??;
    Ok(Some(PreparedPopup {
        handle,
        native_window: prepared.0,
        hwnd: prepared.1,
        bounds,
        strict_exclusion: warning.is_none(),
        started,
    }))
}
fn tray_icon_size() -> u32 {
    use windows_sys::Win32::UI::HiDpi::{GetDpiForSystem, GetSystemMetricsForDpi};
    use windows_sys::Win32::UI::WindowsAndMessaging::SM_CXSMICON;
    let size = unsafe { GetSystemMetricsForDpi(SM_CXSMICON, GetDpiForSystem()) };
    size.max(16) as u32
}

fn app_tray_icon() -> anyhow::Result<Icon> {
    let size = tray_icon_size();
    tracing::info!(size, "loading the tray icon from the app icon resource");
    Ok(Icon::from_resource(1, Some((size, size)))?)
}

fn decode_menu_icon(bytes: &[u8]) -> anyhow::Result<menu::Icon> {
    let image = image::load_from_memory(bytes)?.into_rgba8();
    let (width, height) = image.dimensions();
    Ok(menu::Icon::from_rgba(image.into_raw(), width, height)?)
}

fn decode_icon_or_warn(bytes: &[u8]) -> Option<menu::Icon> {
    decode_menu_icon(bytes)
        .inspect_err(|error| tracing::warn!("failed to load a Windows tray menu icon: {error:#}"))
        .ok()
}

fn menu_text(title: &str) -> String {
    title.replace('&', "&&")
}

fn build_native_menu(entries: &[Entry]) -> anyhow::Result<NativeMenu> {
    let menu = Menu::new();
    let mut actions = HashMap::new();
    for entry in entries {
        let item = build_native_item(entry, &mut actions)?;
        menu.append(item.as_ref())?;
    }
    Ok(NativeMenu { menu, actions })
}

fn build_native_item(
    entry: &Entry,
    actions: &mut HashMap<MenuId, TrayItem>,
) -> anyhow::Result<Box<dyn IsMenuItem>> {
    match entry {
        Entry::Separator => Ok(Box::new(PredefinedMenuItem::separator())),
        Entry::Item {
            title,
            item,
            enabled,
            icon,
            accelerator,
        } => {
            let accelerator = accelerator.map(|accelerator| {
                menu::accelerator::Accelerator::new(accelerator.modifiers, accelerator.code)
            });
            let icon = icon.as_deref().and_then(decode_icon_or_warn);
            let enabled = *enabled && item.is_some();
            let native_item: Box<dyn IsMenuItem> = match icon {
                Some(icon) => Box::new(IconMenuItem::new(
                    menu_text(title),
                    enabled,
                    Some(icon),
                    accelerator,
                )),
                None => Box::new(MenuItem::new(menu_text(title), enabled, accelerator)),
            };
            if enabled && let Some(item) = item {
                let _ = actions.insert(native_item.id().clone(), item.clone());
            }
            Ok(native_item)
        }
        Entry::Submenu {
            title,
            enabled,
            items,
            icon,
        } => {
            let submenu = Submenu::new(menu_text(title), *enabled);
            if let Some(icon) = icon.as_deref().and_then(decode_icon_or_warn) {
                submenu.set_icon(Some(icon));
            }
            for entry in items {
                let item = build_native_item(entry, actions)?;
                submenu.append(item.as_ref())?;
            }
            Ok(Box::new(submenu))
        }
    }
}

pub fn set_recording(_recording: bool, cx: &mut App) {
    refresh_menu(cx);
}
pub fn mode_changed(mode: Mode, cx: &mut App) {
    if !cx.has_global::<Tray>() {
        return;
    }
    cx.global_mut::<Tray>().mode = mode;
    refresh_menu(cx);
}

pub fn refresh_previous(cx: &mut App) {
    if !cx.has_global::<Tray>() {
        return;
    }
    let tray = cx.global_mut::<Tray>();
    tray.previous_generation = tray.previous_generation.wrapping_add(1);
    let generation = tray.previous_generation;
    cx.spawn(async move |cx| {
        let previous = cx
            .background_executor()
            .spawn(async { scan_previous(true) })
            .await;
        cx.update(|cx| {
            if !cx.has_global::<Tray>() || cx.global::<Tray>().previous_generation != generation {
                return;
            }
            cx.global_mut::<Tray>().previous = previous;
            refresh_menu(cx);
        });
    })
    .detach();
}

pub fn previous_items(cx: &App) -> Vec<PreviousItem> {
    if !cx.has_global::<Tray>() {
        return Vec::new();
    }
    cx.global::<Tray>().previous.clone()
}

pub fn menu_snapshot(cx: &App) -> Vec<Entry> {
    if !cx.has_global::<Tray>() {
        return Vec::new();
    }
    let tray = cx.global::<Tray>();
    live_entries(cx, tray.mode, &tray.previous, &state_snapshot(cx))
}

fn icon_for_state(state: &SurfaceState) -> Option<StateIcon> {
    match state {
        SurfaceState::Recording { paused: true, .. } => Some(StateIcon::Paused),
        SurfaceState::Recording { .. } => Some(StateIcon::Recording),
        SurfaceState::Exporting { percent, .. } => Some(StateIcon::Export(
            ((u16::from(*percent) * 8 / 101) as u8).min(7),
        )),
        _ => None,
    }
}
pub fn refresh_menu(cx: &mut App) {
    if !cx.has_global::<Tray>() {
        return;
    }
    let state = state_snapshot(cx);
    let tray = cx.global::<Tray>();
    let entries = live_entries(cx, tray.mode, &tray.previous, &state);
    let changed = entries != tray.entries;
    let state_changed = state != tray.state;
    if !changed && !state_changed && icon_for_state(&state) == tray.icon_state {
        return;
    }
    let native_menu = if changed {
        match build_native_menu(&entries) {
            Ok(menu) => Some(menu),
            Err(error) => {
                tracing::error!(%error, "failed to rebuild Windows tray menu");
                None
            }
        }
    } else {
        None
    };
    let tray = cx.global_mut::<Tray>();
    if let Some(native_menu) = native_menu {
        if tray.popup.is_none() {
            tray.icon.set_menu(Some(Box::new(native_menu.menu.clone())));
        }
        tray.native_menu = native_menu;
    }
    let icon_state = icon_for_state(&state);
    let throttled = matches!(
        (tray.icon_state, icon_state),
        (Some(StateIcon::Export(_)), Some(StateIcon::Export(_)))
    ) && tray.icon_updated.elapsed() < Duration::from_secs(2);
    if icon_state != tray.icon_state && !throttled {
        let icon = match icon_state {
            Some(state) => {
                let size = tray_icon_size();
                Icon::from_rgba(native::icon_pixels(state, size), size, size)
                    .map_err(anyhow::Error::from)
            }
            None => app_tray_icon(),
        };
        match icon.and_then(|icon| tray.icon.set_icon(Some(icon)).map_err(anyhow::Error::from)) {
            Ok(()) => {
                tray.icon_state = icon_state;
                tray.icon_updated = Instant::now();
            }
            Err(error) => tracing::error!(%error, "Windows tray icon update failed"),
        }
    }
    let tooltip = match &state {
        SurfaceState::Idle | SurfaceState::Setup => "FrameEver".to_string(),
        SurfaceState::Starting(_) => t!("FrameEver · Starting").into(),
        SurfaceState::Recording { paused: true, .. } => t!("FrameEver · Paused").into(),
        SurfaceState::Recording { .. } => t!("FrameEver · Recording").into(),
        SurfaceState::Exporting { percent, .. } => {
            t!("FrameEver · Exporting {percent}%", percent = percent)
        }
    };
    if let Err(error) = tray.icon.set_tooltip(Some(tooltip)) {
        tracing::error!(%error, "Windows tray tooltip update failed");
    }
    tray.state = state;
    tray.entries = entries.clone();
    let popup_state = tray.state.clone();
    let (popup, mode) = (tray.popup, tray.mode);
    if let Some(popup) = popup {
        let warning = platform::window_capture_exclusion_unavailable_reason()
            .map(|_| t!("This menu may appear in recordings on this desktop.").to_string());
        if let Err(error) = popup.update(cx, |view, window, cx| {
            view.set_entries(entries, mode, warning, &popup_state, window, cx)
        }) {
            tracing::error!(%error, "tray popup update failed");
        }
    }
}

fn probe_from_env(cx: &mut App) {
    let Some(path) = std::env::var_os("FRAMEEVER_GPUI_TRAY_PROBE") else {
        return;
    };
    cx.spawn(async move |cx| {
        cx.background_executor().timer(Duration::from_secs(3)).await;
        let has_popup = cx.update(|cx| cx.global::<Tray>().popup.is_some());
        let mut samples = Vec::new();
        if has_popup {
            for _ in 0..5 {
                let started = Instant::now();
                let shown = cx.update(show_popup);
                let opened = shown.await.map(|_| started.elapsed().as_micros()).map_err(|error| error.to_string());
                cx.background_executor().timer(Duration::from_millis(100)).await;
                let sample = cx.update(|cx| {
                    let handle = cx.global::<Tray>().popup.unwrap();
                    let hwnd = popup_window_id(cx).and_then(|id| id.to_string().parse::<isize>().ok()).unwrap_or(0) as windows_sys::Win32::Foundation::HWND;
                    let mut affinity = 0;
                    let affinity_ok = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetWindowDisplayAffinity(hwnd, &mut affinity) } != 0;
                    let visible = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::IsWindowVisible(hwnd) } != 0;
                    let render_us = handle.read(cx).ok().and_then(|view| view.last_render_us);
                    unsafe { windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(hwnd, windows_sys::Win32::UI::WindowsAndMessaging::WM_ACTIVATE, 0, 0); }
                    serde_json::json!({"show_dispatch_us": opened, "open_to_render_us": render_us, "visible_after_open": visible, "foreground_matches_popup": unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow() } == hwnd, "affinity_read_ok": affinity_ok, "affinity": affinity, "streamed_exclusion_reason": platform::window_capture_exclusion_unavailable_reason()})
                });
                cx.background_executor().timer(Duration::from_millis(100)).await;
                let hidden = cx.update(|cx| {
                    let hwnd = popup_window_id(cx).and_then(|id| id.to_string().parse::<isize>().ok()).unwrap_or(0) as windows_sys::Win32::Foundation::HWND;
                    unsafe { windows_sys::Win32::UI::WindowsAndMessaging::IsWindowVisible(hwnd) == 0 }
                });
                let mut sample = sample;
                sample["hidden_after_injected_deactivation"] = serde_json::json!(hidden);
                samples.push(sample);
            }
        }
        let keyboard = if has_popup {
            cx.update(|cx| {
                let hwnd = cx.global::<Tray>().icon.window_handle();
                unsafe { windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(hwnd, 6002, 0, windows_sys::Win32::UI::WindowsAndMessaging::WM_CONTEXTMENU as isize); }
            });
            cx.background_executor().timer(Duration::from_millis(100)).await;
            let opened = cx.update(|cx| {
                let hwnd = popup_window_id(cx).and_then(|id| id.to_string().parse::<isize>().ok()).unwrap_or(0) as windows_sys::Win32::Foundation::HWND;
                let visible = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::IsWindowVisible(hwnd) } != 0;
                let scan = unsafe { windows_sys::Win32::UI::Input::KeyboardAndMouse::MapVirtualKeyW(27, 0) };
                unsafe { windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(hwnd, windows_sys::Win32::UI::WindowsAndMessaging::WM_KEYDOWN, 27, ((scan << 16) | 1) as isize); }
                visible
            });
            cx.background_executor().timer(Duration::from_millis(100)).await;
            let hidden = cx.update(|cx| {
                let hwnd = popup_window_id(cx).and_then(|id| id.to_string().parse::<isize>().ok()).unwrap_or(0) as windows_sys::Win32::Foundation::HWND;
                let hidden = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::IsWindowVisible(hwnd) } == 0;
                if let Some(handle) = cx.global::<Tray>().popup { handle.update(cx, |_, window, _| popup::hide(window)).ok(); }
                hidden
            });
            serde_json::json!({"injected_contextmenu_opened": opened, "injected_escape_dismissed": hidden})
        } else { serde_json::Value::Null };
        let result = cx.update(|cx| {
            use tray_icon::menu::ContextMenu as _;
            let tray = cx.global::<Tray>();
            let native_quit_command = if tray.popup.is_none() && matches!(tray.entries.last(), Some(Entry::Item { item: Some(TrayItem::Quit), .. })) {
                let menu = tray.native_menu.menu.hpopupmenu() as windows_sys::Win32::UI::WindowsAndMessaging::HMENU;
                let count = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetMenuItemCount(menu) };
                let command = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetMenuItemID(menu, count - 1) };
                (count > 0 && command != u32::MAX).then_some(command)
            } else { None };
            (serde_json::json!({"samples": samples, "keyboard_message_probe": keyboard, "native_fallback": tray.popup.is_none(), "native_quit_command": native_quit_command, "owner_visual_acceptance": false, "real_taskbar_click_tested": false, "activation_strategy": if std::env::var_os("FRAMEEVER_GPUI_TRAY_NATIVE_ACTIVATION_ONLY").is_some() { "native" } else { "gpui" }, "streamed_exclusion_reason": platform::window_capture_exclusion_unavailable_reason()}), native_quit_command, tray.icon.window_handle() as isize)
        });
        if let Err(error) = std::fs::write(&path, result.0.to_string()) { tracing::error!(%error, "tray probe output failed"); }
        if let Some(command) = result.1 {
            unsafe { windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(result.2 as windows_sys::Win32::Foundation::HWND, windows_sys::Win32::UI::WindowsAndMessaging::WM_COMMAND, command as usize, 0); }
        }
    }).detach();
}
#[cfg(test)]
mod tests {
    use super::*;
    use tray_icon::Rect;

    #[test]
    fn menu_titles_preserve_literal_ampersands() {
        assert_eq!(
            menu_text("Research & Development"),
            "Research && Development"
        );
        assert_eq!(menu_text("Studio"), "Studio");
    }

    #[test]
    fn left_click_opens_the_window_unless_it_stops_a_recording() {
        assert!(matches!(click_item(false), Some(TrayItem::OpenCap)));
        assert!(click_item(true).is_none());
    }

    fn click(id: &str, button: MouseButton, button_state: MouseButtonState) -> TrayIconEvent {
        TrayIconEvent::Click {
            id: id.into(),
            position: Default::default(),
            rect: Rect::default(),
            button,
            button_state,
        }
    }

    #[test]
    fn recording_stop_uses_one_left_click_event_for_this_tray() {
        assert!(activates_tray(&click(
            TRAY_ID,
            MouseButton::Left,
            MouseButtonState::Up
        )));
        assert!(!activates_tray(&click(
            TRAY_ID,
            MouseButton::Left,
            MouseButtonState::Down
        )));
        assert!(!activates_tray(&click(
            "another-tray",
            MouseButton::Left,
            MouseButtonState::Up
        )));
        assert!(!activates_tray(&TrayIconEvent::DoubleClick {
            id: TRAY_ID.into(),
            position: Default::default(),
            rect: Rect::default(),
            button: MouseButton::Left,
        }));
    }

    #[test]
    fn right_and_middle_clicks_do_not_stop_a_recording() {
        for button in [MouseButton::Right, MouseButton::Middle] {
            for state in [MouseButtonState::Down, MouseButtonState::Up] {
                assert!(!activates_tray(&click(TRAY_ID, button, state)));
            }
        }
    }

    fn relative_luminance(rgb: [f32; 3]) -> f32 {
        let [r, g, b] = rgb.map(|channel| {
            if channel <= 0.04045 {
                channel / 12.92
            } else {
                ((channel + 0.055) / 1.055).powf(2.4)
            }
        });
        0.2126 * r + 0.7152 * g + 0.0722 * b
    }

    fn contrast(a: [f32; 3], b: [f32; 3]) -> f32 {
        let (a, b) = (relative_luminance(a), relative_luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    fn over(pixel: &[u8], background: [f32; 3]) -> [f32; 3] {
        let alpha = f32::from(pixel[3]) / 255.;
        [0, 1, 2].map(|channel| {
            f32::from(pixel[channel]) / 255. * alpha + background[channel] * (1. - alpha)
        })
    }

    #[test]
    fn recording_icon_stands_out_on_dark_and_light_taskbars() {
        const DARK_TASKBAR: [f32; 3] = [0.11, 0.11, 0.11];
        const LIGHT_TASKBAR: [f32; 3] = [0.95, 0.95, 0.95];
        const SMALL_ICON_SIZES: [u32; 4] = [16, 20, 24, 32];
        const LEGIBLE_CONTRAST: f32 = 3.;
        const MINIMUM_LEGIBLE_SHARE: f32 = 0.25;

        for size in SMALL_ICON_SIZES {
            let pixels = native::icon_pixels(StateIcon::Recording, size);
            for taskbar in [DARK_TASKBAR, LIGHT_TASKBAR] {
                let visible = pixels
                    .chunks_exact(4)
                    .filter(|pixel| contrast(over(pixel, taskbar), taskbar) >= LEGIBLE_CONTRAST)
                    .count();
                let share = visible as f32 / (size * size) as f32;
                assert!(
                    share >= MINIMUM_LEGIBLE_SHARE,
                    "{size} px on {taskbar:?}: only {share} of the pixels stand out"
                );
            }
        }
    }
}

pub(super) fn fit_popup(
    window: &mut gpui::Window,
    width: f64,
    height: f64,
    cx: &App,
) -> anyhow::Result<bool> {
    let icon = cx
        .global::<Tray>()
        .icon
        .rect()
        .ok_or_else(|| anyhow::anyhow!("tray icon position unavailable"))?;
    let (bounds, scale) = native::placement(icon, width, height)?;
    let (main_bounds, _) = native::placement(icon, 280., height)?;
    let on_left = main_bounds[0] - bounds[0] >= 280. * scale;
    let native_window = platform::native_window(window)
        .ok_or_else(|| anyhow::anyhow!("tray popup has no native window"))?;
    cx.spawn(async move |_| {
        platform::set_window_frame(&native_window, bounds[0], bounds[1], bounds[2], bounds[3])
    })
    .detach();
    Ok(on_left)
}

fn permits_action(state: &SurfaceState, item: &TrayItem) -> bool {
    match state {
        SurfaceState::Starting(_) => matches!(item, TrayItem::CancelCountdown),
        SurfaceState::Recording { paused, .. } => match item {
            TrayItem::StopRecording | TrayItem::RestartRecording | TrayItem::ToggleControls => true,
            TrayItem::PauseRecording => !paused,
            TrayItem::ResumeRecording => *paused,
            _ => false,
        },
        SurfaceState::Setup => matches!(item, TrayItem::RequestPermissions | TrayItem::Quit),
        SurfaceState::Idle | SurfaceState::Exporting { .. } => match item {
            TrayItem::StopRecording
            | TrayItem::CancelCountdown
            | TrayItem::PauseRecording
            | TrayItem::ResumeRecording
            | TrayItem::RestartRecording
            | TrayItem::ToggleControls => false,
            TrayItem::ShowExport => matches!(state, SurfaceState::Exporting { .. }),
            _ => true,
        },
    }
}

pub(super) fn action_available(item: &TrayItem, cx: &App) -> bool {
    let session = RecordingSession::global(cx);
    if session.read(cx).phase == Phase::Stopping {
        return matches!(item, TrayItem::StopRecording);
    }
    permits_action(&state_snapshot(cx), item)
}
#[cfg(test)]
mod stale_action_tests {
    use super::*;

    #[test]
    fn tray_stale_idle_actions_cannot_run_after_recording_starts() {
        let state = SurfaceState::Recording {
            paused: false,
            elapsed: 5,
            controls_hidden: false,
        };
        assert!(!permits_action(&state, &TrayItem::Quit));
        assert!(!permits_action(&state, &TrayItem::ModeScreenshot));
        assert!(!permits_action(&state, &TrayItem::ImportMedia));
        assert!(permits_action(&state, &TrayItem::StopRecording));
        assert!(permits_action(&state, &TrayItem::PauseRecording));
        assert!(!permits_action(&state, &TrayItem::ResumeRecording));
        assert!(!permits_action(
            &SurfaceState::Idle,
            &TrayItem::RestartRecording
        ));
    }
}
