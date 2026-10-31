use gpui::{App, Context, Global, Hsla, Rgba, Window, WindowAppearance, rgb, rgba};

use std::sync::LazyLock;

use crate::platform::{self, ForcedAppearance, MaterialKind};
use crate::store::AppTheme;

mod tokens;

pub use tokens::{CONTROL_HEIGHT, Component, ROW_HEIGHT, Semantic};

pub fn ui_font() -> &'static str {
    match crate::i18n::current() {
        crate::i18n::Language::SimplifiedChinese => "HarmonyOS Sans SC",
        crate::i18n::Language::English => "Inter",
    }
}

pub fn chrome_window_background() -> gpui::WindowBackgroundAppearance {
    if cfg!(target_os = "windows") {
        gpui::WindowBackgroundAppearance::Opaque
    } else {
        gpui::WindowBackgroundAppearance::Transparent
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Appearance {
    Light,
    Dark,
}

impl Appearance {
    pub fn from_window(appearance: WindowAppearance) -> Self {
        match appearance {
            WindowAppearance::Light | WindowAppearance::VibrantLight => Self::Light,
            WindowAppearance::Dark | WindowAppearance::VibrantDark => Self::Dark,
        }
    }
}

/// `general_settings.theme`, the same `AppTheme` the Tauri app persists and
/// applies through `commands.setTheme` / `windows::set_theme`.
struct ThemePreference(AppTheme);

impl Global for ThemePreference {}

pub fn init(cx: &mut App) {
    cx.set_global(ThemePreference(crate::store::GeneralSettings::load().theme));
}

pub fn current_preference(cx: &App) -> AppTheme {
    cx.try_global::<ThemePreference>()
        .map(|preference| preference.0)
        .unwrap_or_else(|| crate::store::GeneralSettings::load().theme)
}

pub fn set_preference(preference: AppTheme, cx: &mut App) {
    cx.set_global(ThemePreference(preference));
}

/// Light/Dark force that appearance; System follows the window (and therefore
/// the OS, once `NSWindow.appearance` is cleared).
pub fn resolve_appearance(preference: AppTheme, system: Appearance) -> Appearance {
    match preference {
        AppTheme::System => system,
        AppTheme::Light => Appearance::Light,
        AppTheme::Dark => Appearance::Dark,
    }
}

pub(crate) fn forced_appearance(preference: AppTheme) -> ForcedAppearance {
    match preference {
        AppTheme::System => ForcedAppearance::System,
        AppTheme::Light => ForcedAppearance::Light,
        AppTheme::Dark => ForcedAppearance::Dark,
    }
}

/// `window.set_theme(...)` — native chrome (traffic lights, materials, menus)
/// follows the preference, not only the painted palette.
pub fn apply_native(window: &Window, cx: &App) {
    let appearance = forced_appearance(current_preference(cx));
    #[cfg(target_os = "windows")]
    platform::apply_window_theme(window, appearance, cx.foreground_executor());
    #[cfg(not(target_os = "windows"))]
    platform::apply_window_theme(window, appearance);
}

/// Force the native appearance and invalidate on OS theme changes so a System
/// preference picks up a light/dark flip without waiting for another event.
pub fn bind_window<T: 'static>(window: &mut Window, cx: &mut Context<T>) {
    apply_native(window, cx);
    cx.observe_window_appearance(window, |_, _window, cx| {
        #[cfg(target_os = "windows")]
        _window.defer(cx, |window, cx| apply_native(window, cx));
        cx.notify();
    })
    .detach();
}

/// What the shell paints *over* a native window material, resolved for one
/// appearance and one visual system.
///
/// The web app expresses this as `--macos-settings-*` custom properties
/// (Tauri `src/styles/theme.css`) selected by two data attributes on
/// `<html>`: `data-macos-native-material` ("panel" for the main window,
/// "settings" for the settings window) and `data-macos-visual-system`
/// ("liquid-glass" or "vibrancy").
///
/// The custom properties themselves are set by the *visual system* blocks, not
/// per material, so one token set serves both windows; what differs is which
/// element each variable lands on, plus the two material-specific shell rules
/// (`[..native-material="panel"] .cap-window-shell`). `shell` and `header`
/// below are the panel's; `sidebar` / `content` / `card` are the settings
/// window's surfaces. Material `"teleprompter"` adds no custom properties at
/// all -- its whole block in `theme.css` is two `border-radius` rules (16, and
/// 22 under liquid glass) -- so it reuses this set through
/// [`Theme::teleprompter_shell_bg`] and [`Theme::teleprompter_window_radius`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaterialTokens {
    pub kind: MaterialKind,

    /// `.cap-window-shell`'s background, material "panel".
    pub shell: Rgba,
    /// `.cap-window-header`'s background.
    pub header: Rgba,
    /// `--macos-settings-border`.
    pub border: Rgba,
    /// `--macos-settings-control-fill`.
    pub control_fill: Rgba,
    /// `--macos-settings-control-hover`.
    pub control_hover: Rgba,
    /// `--macos-settings-control-active`.
    pub control_active: Rgba,
    /// `--macos-settings-selection`.
    pub selection: Rgba,
    /// `--macos-settings-text`.
    pub text: Rgba,
    /// `--macos-settings-muted`.
    pub muted: Rgba,
    /// `--macos-settings-fill`.
    pub fill: Rgba,
    /// `--macos-settings-hover`.
    pub hover: Rgba,
    /// `--macos-settings-sidebar`, the settings window's left pane.
    pub sidebar: Rgba,
    /// `--macos-settings-content`, the settings window's right pane.
    pub content: Rgba,
    /// `--macos-settings-card`, a `.cap-settings-card` / `.bg-gray-2` surface
    /// inside that pane.
    pub card: Rgba,
    /// `--macos-settings-window-radius`. The settings shell takes it as-is;
    /// the main window overrides it back to 16
    /// (`[data-macos-native-material="panel"] .cap-window-shell
    /// { border-radius: 16px }`), which is why `MAIN_WINDOW_MATERIAL_RADIUS`
    /// is a constant and this is not.
    pub window_radius: f32,
    /// `--macos-settings-sidebar-radius`. Transcribed for the record: the
    /// liquid-glass settings rule zeroes it again
    /// (`[..visual-system="liquid-glass"][..native-material="settings"]
    /// .cap-settings-sidebar { border-radius: 0 }`), so the pane is square on
    /// both paths and the window's own radius does the clipping.
    pub sidebar_radius: f32,
}

impl MaterialTokens {
    /// Only Liquid Glass remaps the body's gray fills: every
    /// `.cap-window-body .bg-gray-N` rule in `theme.css` is gated on
    /// `[data-macos-visual-system="liquid-glass"]`. Under vibrancy the body
    /// keeps its Radix grays and only the shell and header change.
    fn remaps_body(&self) -> bool {
        self.kind == MaterialKind::LiquidGlass
    }

    /// The panel material under `NSGlassEffectView`.
    fn liquid_glass(appearance: Appearance) -> Self {
        match appearance {
            Appearance::Light => Self {
                kind: MaterialKind::LiquidGlass,
                // `[data-macos-visual-system="liquid-glass"][data-macos-native-material="panel"]
                //  .cap-window-shell { background: rgba(255, 255, 255, 0.55) }`
                // -- a wash *over* the live glass backdrop, not a replacement
                // for it. The base glass rule strips the border and shadow:
                // `.cap-window-shell { background: transparent; border: 0;
                //  box-shadow: none }`.
                shell: rgba(0xffffff8c),
                // `[data-macos-visual-system="liquid-glass"] .cap-window-header
                //  { background: transparent; border-bottom-color: transparent }`
                header: rgba(0x00000000),
                // `[data-macos-visual-system="liquid-glass"] {
                //  --macos-settings-border: rgba(0, 0, 0, 0.14) }` -- the glass
                // block overrides the base `rgba(0, 0, 0, 0.08)`.
                border: rgba(0x00000024),
                // Not overridden by the glass block, so the `:root` values:
                // `--macos-settings-control-fill: rgba(255, 255, 255, 0.78)`
                control_fill: rgba(0xffffffc7),
                // `--macos-settings-control-hover: rgba(255, 255, 255, 0.96)`
                control_hover: rgba(0xfffffff5),
                // `--macos-settings-control-active: rgba(0, 0, 0, 0.08)`
                control_active: rgba(0x00000014),
                // `--macos-settings-selection: rgba(0, 0, 0, 0.1)`
                selection: rgba(0x0000001a),
                // `--macos-settings-text: rgba(0, 0, 0, 0.88)`
                text: rgba(0x000000e0),
                // `:root { --macos-settings-muted: rgba(0, 0, 0, 0.48) }`,
                // not overridden by the glass block.
                muted: rgba(0x0000007a),
                // The glass block *does* override these three:
                // `--macos-settings-fill: rgba(0, 0, 0, 0.045)`
                fill: rgba(0x0000000b),
                // `--macos-settings-hover: rgba(0, 0, 0, 0.065)`
                hover: rgba(0x00000011),
                sidebar: rgba(0xffffffc7),
                // Deliberately not the Tauri `--macos-settings-content` /
                // `--macos-settings-card` values: the GPUI settings window has
                // its own grouped layout.
                content: rgba(0xffffffff),
                card: rgba(0x0000000a),
                // `--macos-settings-window-radius: 26px`
                window_radius: 26.,
                // `--macos-settings-sidebar-radius: 18px`
                sidebar_radius: 18.,
            },
            Appearance::Dark => Self {
                kind: MaterialKind::LiquidGlass,
                // `.dark[data-macos-visual-system="liquid-glass"][data-macos-native-material="panel"]
                //  .cap-window-shell { background: rgba(17, 17, 17, 0.88) }`
                shell: rgba(0x111111e0),
                header: rgba(0x00000000),
                // `:root.dark[data-macos-visual-system="liquid-glass"]`:
                // `--macos-settings-border: rgba(255, 255, 255, 0.07)`
                border: rgba(0xffffff12),
                // `--macos-settings-control-fill: rgba(255, 255, 255, 0.08)`
                control_fill: rgba(0xffffff14),
                // `--macos-settings-control-hover: rgba(255, 255, 255, 0.12)`
                control_hover: rgba(0xffffff1f),
                // `--macos-settings-control-active: rgba(255, 255, 255, 0.06)`
                control_active: rgba(0xffffff0f),
                // `--macos-settings-selection: #2c2c2c`
                selection: rgb(0x2c2c2c),
                // `--macos-settings-text: rgba(255, 255, 255, 0.95)`
                text: rgba(0xfffffff2),
                // `--macos-settings-muted: #a1a1a1`
                muted: rgb(0xa1a1a1),
                // `--macos-settings-fill: rgba(255, 255, 255, 0.05)`
                fill: rgba(0xffffff0d),
                // `--macos-settings-hover: rgba(255, 255, 255, 0.05)`
                hover: rgba(0xffffff0d),
                // `--macos-settings-sidebar: rgba(28, 28, 28, 0.88)`
                sidebar: rgba(0x1c1c1ce0),
                content: rgba(0x1e1e1eff),
                card: rgba(0xffffff0d),
                // The radii are set once, outside the `.dark` block.
                window_radius: 26.,
                sidebar_radius: 18.,
            },
        }
    }

    /// The panel material under `NSVisualEffectView`, i.e. pre-macOS-26. The
    /// shell keeps the border and the header keeps its own wash, because none
    /// of the `liquid-glass` overrides apply.
    fn vibrancy(appearance: Appearance) -> Self {
        match appearance {
            Appearance::Light => Self {
                kind: MaterialKind::Vibrancy,
                // `[data-macos-native-material] .cap-window-shell { background:
                //  var(--macos-settings-window); border: 1px solid
                //  var(--macos-settings-border) }`, and `:root`'s
                // `--macos-settings-window: rgba(244, 244, 243, 0.84)`.
                shell: rgba(0xf4f4f3d6),
                // `[data-macos-native-material] .cap-window-header { background:
                //  rgba(250, 250, 249, 0.72) }`. Its `backdrop-filter:
                //  blur(28px) saturate(1.45)` has no equivalent in this gpui
                // rev (same gap as the recording overlay's `backdrop-blur-xs`).
                header: rgba(0xfafaf9b8),
                // `--macos-settings-border: rgba(0, 0, 0, 0.08)`
                border: rgba(0x00000014),
                control_fill: rgba(0xffffffc7),
                control_hover: rgba(0xfffffff5),
                control_active: rgba(0x00000014),
                selection: rgba(0x0000001a),
                text: rgba(0x000000e0),
                muted: rgba(0x0000007a),
                // The `:root` values, none of them overridden without the
                // glass block: `--macos-settings-fill: rgba(0, 0, 0, 0.055)`
                fill: rgba(0x0000000e),
                // `--macos-settings-hover: rgba(0, 0, 0, 0.055)`
                hover: rgba(0x0000000e),
                // `--macos-settings-sidebar: rgba(250, 250, 249, 0.74)`
                sidebar: rgba(0xfafaf9bd),
                content: rgba(0xfbfbfbff),
                card: rgba(0x0000000a),
                // `--macos-settings-window-radius: 16px`
                window_radius: 16.,
                // `--macos-settings-sidebar-radius: 0px`
                sidebar_radius: 0.,
            },
            Appearance::Dark => Self {
                kind: MaterialKind::Vibrancy,
                // `:root.dark { --macos-settings-window: rgba(17, 17, 17, 0.94) }`
                shell: rgba(0x111111f0),
                // `:root.dark[data-macos-native-material] .cap-window-header
                //  { background: rgba(28, 28, 28, 0.88) }`
                header: rgba(0x1c1c1ce0),
                // `:root.dark { --macos-settings-border: rgba(255, 255, 255, 0.07) }`
                border: rgba(0xffffff12),
                control_fill: rgba(0xffffff14),
                control_hover: rgba(0xffffff1f),
                control_active: rgba(0xffffff0f),
                selection: rgb(0x2c2c2c),
                text: rgba(0xfffffff2),
                // `:root.dark { --macos-settings-muted: #a1a1a1 }`
                muted: rgb(0xa1a1a1),
                // `--macos-settings-fill: rgba(255, 255, 255, 0.05)`
                fill: rgba(0xffffff0d),
                // `--macos-settings-hover: rgba(255, 255, 255, 0.05)`
                hover: rgba(0xffffff0d),
                // `--macos-settings-sidebar: rgba(22, 22, 22, 0.9)` -- the one
                // settings surface the glass block does *not* just re-tint.
                sidebar: rgba(0x161616e6),
                content: rgba(0x1e1e1eff),
                card: rgba(0xffffff0d),
                window_radius: 16.,
                sidebar_radius: 0.,
            },
        }
    }

    pub fn new(kind: MaterialKind, appearance: Appearance) -> Self {
        match kind {
            MaterialKind::LiquidGlass => Self::liquid_glass(appearance),
            MaterialKind::Vibrancy => Self::vibrancy(appearance),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub appearance: Appearance,
    pub material: Option<MaterialTokens>,
    pub semantic: Semantic,
    pub component: Component,
    pub fixed_white: Rgba,
    pub badge: Rgba,

    pub gray_1: Rgba,
    pub gray_2: Rgba,
    pub gray_3: Rgba,
    pub gray_4: Rgba,
    pub gray_5: Rgba,
    pub gray_6: Rgba,
    pub gray_7: Rgba,
    pub gray_8: Rgba,
    pub gray_9: Rgba,
    pub gray_10: Rgba,
    pub gray_11: Rgba,
    pub gray_12: Rgba,

    pub red_2: Rgba,
    pub red_3: Rgba,
    pub red_4: Rgba,
    pub red_9: Rgba,
    pub red_10: Rgba,
    pub red_11: Rgba,
    pub red_300: Rgba,

    pub amber_3: Rgba,
    pub amber_6: Rgba,
    pub amber_11: Rgba,

    pub gray_200_legacy: Rgba,
    pub gray_400_legacy: Rgba,
    pub gray_500_legacy: Rgba,

    pub text_primary: Rgba,

    pub editor: EditorPalette,
}

impl std::ops::Deref for Theme {
    type Target = Semantic;

    fn deref(&self) -> &Semantic {
        &self.semantic
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EditorPalette {
    pub window: Rgba,
    pub card: Rgba,
    pub card_2: Rgba,
    pub stage: Rgba,
    pub line: Rgba,
    pub line_strong: Rgba,
    pub text_1: Rgba,
    pub text_2: Rgba,
    pub text_3: Rgba,
    pub ctl: Rgba,
    pub ctl_hover: Rgba,
    pub ctl_active: Rgba,
    pub accent: Rgba,
    pub accent_hover: Rgba,
    pub accent_active: Rgba,
    pub playhead: Rgba,
    pub thumb: Rgba,
    pub seg_tint: f32,
    pub seg_line: f32,
    pub seg_label_base: Rgba,
    pub seg_label_mix: f32,
}

impl EditorPalette {
    fn new(appearance: Appearance, semantic: &Semantic, component: &Component) -> Self {
        let dark = appearance == Appearance::Dark;
        Self {
            window: semantic.background,
            card: semantic.card,
            card_2: semantic.secondary,
            stage: semantic.preview,
            line: semantic.border,
            line_strong: semantic.border_strong,
            text_1: semantic.foreground,
            text_2: semantic.muted_foreground,
            text_3: semantic.muted_foreground,
            ctl: semantic.secondary,
            ctl_hover: semantic.secondary_hover,
            ctl_active: semantic.secondary_active,
            accent: semantic.primary,
            accent_hover: semantic.primary_hover,
            accent_active: semantic.primary_active,
            playhead: component.playhead,
            thumb: component.slider_thumb,
            seg_tint: if dark { 0.20 } else { 0.13 },
            seg_line: if dark { 0.42 } else { 0.34 },
            seg_label_base: if dark { rgb(0xffffff) } else { rgb(0x000000) },
            seg_label_mix: if dark { 0.55 } else { 0.58 },
        }
    }

    /// `color-mix(in srgb, color <tint>, card)`: the segment fill.
    pub fn seg_fill(&self, color: Rgba) -> Rgba {
        mix(self.card, color, self.seg_tint)
    }

    /// A selected segment: the tint plus ten points.
    pub fn seg_fill_selected(&self, color: Rgba) -> Rgba {
        mix(self.card, color, self.seg_tint + 0.10)
    }

    /// The segment's 1px inset border.
    pub fn seg_border(&self, color: Rgba) -> Rgba {
        rgba_alpha(color, self.seg_line)
    }

    /// Segment label text.
    pub fn seg_label(&self, color: Rgba) -> Rgba {
        mix(self.seg_label_base, color, self.seg_label_mix)
    }

    /// Segment secondary text (durations, amounts): the label colour at 45%
    /// toward `text_3`.
    pub fn seg_muted(&self, color: Rgba) -> Rgba {
        mix(self.text_3, color, 0.45)
    }

    /// The track gutter's icon tile: `color 16%` over transparent.
    pub fn tile_bg(&self, color: Rgba) -> Rgba {
        rgba_alpha(color, 0.16)
    }

    /// The tile's glyph: the track colour pulled 62% toward the label base.
    pub fn tile_fg(&self, color: Rgba) -> Rgba {
        mix(self.seg_label_base, color, 0.62)
    }

    /// Trim handles: the colour at 70% toward the label base.
    pub fn seg_handle(&self, color: Rgba) -> Rgba {
        mix(self.seg_label_base, color, 0.70)
    }
}

/// `color-mix(in srgb, b <t>, a)`: straight sRGB interpolation, which is what
/// the CSS side does.
pub fn mix(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let t = t.clamp(0.0, 1.0);
    Rgba {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    }
}

pub fn rgba_alpha(color: Rgba, alpha: f32) -> Rgba {
    Rgba { a: alpha, ..color }
}

impl Theme {
    pub fn new(appearance: Appearance) -> Self {
        static DARK: LazyLock<Theme> = LazyLock::new(|| Theme::build(Appearance::Dark));
        static LIGHT: LazyLock<Theme> = LazyLock::new(|| Theme::build(Appearance::Light));
        match appearance {
            Appearance::Dark => *DARK,
            Appearance::Light => *LIGHT,
        }
    }

    fn build(appearance: Appearance) -> Self {
        let semantic = Semantic::new(appearance);
        let component = Component::new(appearance);
        let brand = tokens::Brand::of(appearance);
        Self {
            appearance,
            material: None,
            semantic,
            component,
            fixed_white: brand.fixed.white.to_rgba(),
            badge: brand.fixed.badge.to_rgba(),
            editor: EditorPalette::new(appearance, &semantic, &component),

            gray_1: semantic.background,
            gray_2: semantic.card,
            gray_3: semantic.secondary,
            gray_4: semantic.secondary_hover,
            gray_5: semantic.border,
            gray_6: semantic.secondary_active,
            gray_7: semantic.border_strong,
            gray_8: semantic.border_strong,
            gray_9: semantic.muted_foreground,
            gray_10: semantic.muted_foreground,
            gray_11: semantic.muted_foreground,
            gray_12: semantic.foreground,

            red_2: semantic.danger_tint,
            red_3: semantic.danger_tint,
            red_4: semantic.danger_tint,
            red_9: semantic.danger,
            red_10: semantic.danger_hover,
            red_11: semantic.danger_text,
            red_300: semantic.recording,

            amber_3: brand.amber.tint.to_rgba(),
            amber_6: semantic.warning,
            amber_11: semantic.warning_text,

            gray_200_legacy: semantic.card,
            gray_400_legacy: semantic.muted_foreground,
            gray_500_legacy: semantic.foreground,

            text_primary: semantic.foreground,
        }
    }

    pub fn for_window(window: &Window, cx: &App, with_material: bool) -> Self {
        let preference = current_preference(cx);
        let system = platform::window_is_dark(window)
            .map(|dark| {
                if dark {
                    Appearance::Dark
                } else {
                    Appearance::Light
                }
            })
            .unwrap_or_else(|| Appearance::from_window(window.appearance()));
        let theme = Self::new(resolve_appearance(preference, system));
        if with_material {
            theme.with_material(platform::active_material(cx))
        } else {
            theme
        }
    }

    pub fn refresh(&mut self, window: &Window, cx: &App, with_material: bool) -> bool {
        let next = Self::for_window(window, cx, with_material);
        if next.appearance != self.appearance || next.material_kind() != self.material_kind() {
            *self = next;
            true
        } else {
            false
        }
    }

    pub fn light() -> Self {
        Self::new(Appearance::Light)
    }

    pub fn dark() -> Self {
        Self::new(Appearance::Dark)
    }

    /// Resolve the panel material for this theme's appearance.
    /// `None` leaves the opaque shell alone.
    pub fn with_material(mut self, kind: Option<MaterialKind>) -> Self {
        self.material = kind.map(|kind| MaterialTokens::new(kind, self.appearance));
        self
    }

    /// The material's own kind, for the `render`-side "is a material live"
    /// checks that have nothing to paint.
    pub fn material_kind(&self) -> Option<MaterialKind> {
        self.material.map(|material| material.kind)
    }

    /// One Radix gray step by number, so the `.bg-gray-N` remaps below can be
    /// written the way `theme.css` writes them.
    pub fn gray(&self, step: u8) -> Hsla {
        match step {
            1 => self.gray_1,
            2 => self.gray_2,
            3 => self.gray_3,
            4 => self.gray_4,
            5 => self.gray_5,
            6 => self.gray_6,
            7 => self.gray_7,
            8 => self.gray_8,
            9 => self.gray_9,
            10 => self.gray_10,
            11 => self.gray_11,
            _ => self.gray_12,
        }
        .into()
    }

    /// `.cap-window-shell`'s background: the material's tint when one is
    /// installed, `bg-gray-1` otherwise.
    pub fn shell_bg(&self) -> Hsla {
        match self.material {
            Some(material) => material.shell.into(),
            None => self.gray_1.into(),
        }
    }

    /// The shell's `1px solid var(--macos-settings-border)`. Liquid Glass
    /// draws none (`.cap-window-shell { border: 0 }`), and neither does the
    /// bare window.
    pub fn shell_border(&self) -> Option<Hsla> {
        self.material
            .filter(|material| !material.remaps_body())
            .map(|material| material.border.into())
    }

    /// `.cap-window-header`'s background.
    pub fn header_bg(&self) -> Hsla {
        match self.material {
            Some(material) => material.header.into(),
            None => self.background.into(),
        }
    }

    /// The `divide-y divide-gray-5` hairline under the header. Liquid Glass
    /// erases it: `.cap-window-shell > * + * { border-color: transparent }`.
    pub fn header_border(&self) -> Hsla {
        match self.material {
            Some(material) if material.remaps_body() => gpui::transparent_black(),
            Some(material) => material.border.into(),
            None => self.gray_5.into(),
        }
    }

    /// `.cap-window-body`'s inherited text colour.
    pub fn body_text(&self) -> Hsla {
        match self.material {
            Some(material) if material.remaps_body() => material.text.into(),
            _ => self.text_primary.into(),
        }
    }

    /// A `.bg-gray-N` fill inside `.cap-window-body`.
    ///
    /// ```text
    /// .bg-gray-2, .bg-gray-3 { background-color: var(--macos-settings-control-fill) }
    /// .bg-gray-4            { background-color: var(--macos-settings-control-hover) }
    /// .bg-gray-5            { background-color: var(--macos-settings-control-active) }
    /// ```
    ///
    /// (The `.dark\:bg-gray-3` / `.dark\:bg-gray-4` rule lands on the same
    /// `control-fill`, so a dark-mode `bg-gray-3 dark:bg-gray-4` pair --
    /// already collapsed to one token by the time it reaches here -- resolves
    /// identically either way.)
    pub fn body_fill(&self, step: u8) -> Hsla {
        let Some(material) = self.material.filter(MaterialTokens::remaps_body) else {
            return self.gray(step);
        };
        match step {
            2 | 3 => material.control_fill.into(),
            4 => material.control_hover.into(),
            5 => material.control_active.into(),
            _ => self.gray(step),
        }
    }

    /// A `hover:bg-gray-N` fill inside `.cap-window-body`.
    ///
    /// ```text
    /// [class*="hover:bg-gray-4"]:hover, [class*="hover:bg-gray-5"]:hover
    ///   { background-color: var(--macos-settings-control-hover) }
    /// [class*="hover:bg-gray-6"]:hover, [class*="hover:bg-gray-7"]:hover
    ///   { background-color: var(--macos-settings-selection) }
    /// ```
    pub fn body_hover_fill(&self, step: u8) -> Hsla {
        let Some(material) = self.material.filter(MaterialTokens::remaps_body) else {
            return self.gray(step);
        };
        match step {
            4 | 5 => material.control_hover.into(),
            6 | 7 => material.selection.into(),
            _ => self.gray(step),
        }
    }

    /// A `border-gray-N` inside `.cap-window-body`.
    ///
    /// ```text
    /// .border-gray-4, .dark\:border-gray-5 { border-color: var(--macos-settings-border) }
    /// .border-gray-5, .border-gray-6       { border-color: var(--macos-settings-border) }
    /// ```
    pub fn body_border(&self, step: u8) -> Hsla {
        let Some(material) = self.material.filter(MaterialTokens::remaps_body) else {
            return self.gray(step);
        };
        match step {
            4..=6 => material.border.into(),
            _ => self.gray(step),
        }
    }

    /// `ring-offset-gray-1`, the 1px gap between the selected Mode button and
    /// its blue ring. Liquid Glass makes it transparent
    /// (`--tw-ring-offset-color: transparent`) so the ring is not sitting on a
    /// solid disc floating over the material.
    pub fn ring_offset(&self) -> Hsla {
        match self.material {
            Some(material) if material.remaps_body() => gpui::transparent_black(),
            _ => self.gray_1.into(),
        }
    }

    // ---- The settings window's surfaces -------------------------------
    //
    // Same tokens, a different set of elements. Every rule quoted below is
    // under `[data-macos-native-material="settings"]`; the fallbacks are the
    // Tailwind classes the TSX carries when no material was installed, which
    // is what the window would paint on a non-mac.

    /// `.cap-settings-shell`'s corner: `border-radius:
    /// var(--macos-settings-window-radius)` with no per-material override
    /// (unlike `.cap-window-shell`'s `16px` for "panel").
    pub fn settings_window_radius(&self) -> f32 {
        self.material
            .map(|material| material.window_radius)
            .unwrap_or(16.)
    }

    /// `.cap-settings-sidebar { background: var(--macos-settings-sidebar) }`,
    /// `bg-gray-2` without a material.
    pub fn settings_sidebar_bg(&self) -> Hsla {
        match self.material {
            Some(material) => material.sidebar.into(),
            None => self.sidebar.into(),
        }
    }

    /// `.cap-settings-content { background: var(--macos-settings-content) }`.
    /// Bare, the pane has no background of its own and shows the shell's
    /// `bg-gray-1`.
    pub fn settings_content_bg(&self) -> Hsla {
        match self.material {
            Some(material) => material.content.into(),
            None => self.gray_1.into(),
        }
    }

    /// `.cap-settings-card, .cap-settings-content .bg-gray-2
    /// { background-color: var(--macos-settings-card) }`.
    pub fn settings_card_bg(&self) -> Hsla {
        match self.material {
            Some(material) => material.card.into(),
            None => self.gray_2.into(),
        }
    }

    /// `.cap-settings-content .bg-gray-3, .bg-gray-4, .bg-gray-5
    /// { background-color: var(--macos-settings-fill) }` -- selects, inline
    /// code chips, the summary boxes.
    pub fn settings_fill(&self) -> Hsla {
        match self.material {
            Some(material) => material.fill.into(),
            None => self.gray_3.into(),
        }
    }

    /// `.cap-settings-nav-item:hover`, `.cap-settings-profile:hover`
    /// `{ background: var(--macos-settings-hover) }`; `hover:bg-gray-3` bare.
    pub fn settings_hover(&self) -> Hsla {
        match self.material {
            Some(material) => material.hover.into(),
            None => self.gray_3.into(),
        }
    }

    /// `.cap-settings-nav-item.bg-gray-5 { background:
    /// var(--macos-settings-selection) }` -- the selected sidebar row.
    pub fn settings_selection(&self) -> Hsla {
        match self.material {
            Some(material) => material.selection.into(),
            None => self.selection.into(),
        }
    }

    /// `--macos-settings-border`: the row dividers, the account footer's top
    /// rule, and the sidebar/content divider.
    pub fn settings_border(&self) -> Hsla {
        match self.material {
            Some(material) => material.border.into(),
            None => self.border.into(),
        }
    }

    /// The divider between the sidebar and the content pane:
    /// `[..liquid-glass][..settings] .cap-settings-shell > * + *
    /// { border-color: var(--macos-settings-border) }`. Only the glass path
    /// remaps it; on the others the shell's own `divide-x divide-gray-3`
    /// stands.
    pub fn settings_divider(&self) -> Hsla {
        match self.material {
            Some(material) if material.kind == MaterialKind::LiquidGlass => material.border.into(),
            _ => self.border.into(),
        }
    }

    /// `.cap-settings-page { color: var(--macos-settings-text) }`; the page's
    /// `text-gray-12` bare.
    pub fn settings_text(&self) -> Hsla {
        match self.material {
            Some(material) => material.text.into(),
            None => self.gray_12.into(),
        }
    }

    /// `.cap-settings-page .text-gray-10, .text-gray-11 { color:
    /// var(--macos-settings-muted) }` -- every description line.
    pub fn settings_muted(&self) -> Hsla {
        match self.material {
            Some(material) => material.muted.into(),
            None => self.gray_10.into(),
        }
    }

    // ---- The teleprompter window's shell -------------------------------
    //
    // `applyMacOSWindowMaterial("teleprompter")` sets the same two data
    // attributes as the other two windows, and `theme.css` gives the material
    // exactly two rules of its own:
    //
    // ```text
    // [data-macos-native-material="teleprompter"] .cap-window-shell { border-radius: 16px }
    // [..visual-system="liquid-glass"][..native-material="teleprompter"]
    //   .cap-window-shell { border-radius: 22px }
    // ```
    //
    // Everything else it paints comes from the shared blocks: the base
    // `[data-macos-native-material] .cap-window-shell` wash + border under
    // vibrancy, and `[..="liquid-glass"] .cap-window-shell { background:
    // transparent; border: 0 }` under glass. The panel's `rgba(255,255,255,
    // 0.55)` tint is *not* inherited -- that rule is `[..="panel"]`-gated --
    // so under Liquid Glass this window is bare glass. The body remaps are
    // panel-gated too, so the footer and script keep their Radix grays.

    /// `.cap-window-shell` under material `"teleprompter"`.
    pub fn teleprompter_shell_bg(&self) -> Hsla {
        match self.material {
            // `[data-macos-visual-system="liquid-glass"] .cap-window-shell
            //  { background: transparent }`, with no panel-style tint on top.
            Some(material) if material.remaps_body() => gpui::transparent_black(),
            // `background: var(--macos-settings-window)`, which is what
            // `MaterialTokens::shell` already holds on the vibrancy path.
            Some(material) => material.shell.into(),
            // No material (non-mac): the route's own
            // `bg-gray-1/90 rounded-2xl border border-gray-5` fallback.
            None => Self::with_alpha(self.gray_1, 0.9),
        }
    }

    /// The teleprompter shell's `1px solid var(--macos-settings-border)` --
    /// the same base rule the panel material takes, erased by the glass block.
    /// Without a material the route draws `border border-gray-5`.
    pub fn teleprompter_shell_border(&self) -> Option<Hsla> {
        match self.material {
            Some(material) if material.remaps_body() => None,
            Some(material) => Some(material.border.into()),
            None => Some(self.gray_5.into()),
        }
    }

    /// 22 under Liquid Glass, 16 under vibrancy; `rounded-2xl` (16) is also
    /// what the non-mac fallback draws.
    pub fn teleprompter_window_radius(&self) -> f32 {
        match self.material_kind() {
            Some(MaterialKind::LiquidGlass) => 22.,
            _ => 16.,
        }
    }

    pub fn is_dark(&self) -> bool {
        self.appearance == Appearance::Dark
    }

    pub fn tile_selected_bg(&self) -> Hsla {
        self.selection.into()
    }

    /// A colour at an alpha, for Tailwind's `/N` overlay fills
    /// (`hover:bg-gray-12/6`, `hover:bg-red-500/8`).
    pub fn with_alpha(color: Rgba, alpha: f32) -> Hsla {
        let mut color: Hsla = color.into();
        color.a = alpha;
        color
    }

    /// The TSX has a `#DCDCDC` unfocused state, but it hangs off an
    /// `onFocusChanged` the non-activating main panel never delivers -- the
    /// shipping lights stay colored, so no inactive color exists here.
    pub const TRAFFIC_CLOSE: u32 = 0xff5f57;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_preference_resolves_like_tauri() {
        assert_eq!(
            resolve_appearance(AppTheme::System, Appearance::Dark),
            Appearance::Dark
        );
        assert_eq!(
            resolve_appearance(AppTheme::System, Appearance::Light),
            Appearance::Light
        );
        assert_eq!(
            resolve_appearance(AppTheme::Light, Appearance::Dark),
            Appearance::Light
        );
        assert_eq!(
            resolve_appearance(AppTheme::Dark, Appearance::Light),
            Appearance::Dark
        );
    }

    #[test]
    fn no_material_paints_the_klein_field_tokens() {
        for theme in [Theme::light(), Theme::dark()] {
            assert_eq!(theme.shell_bg(), Hsla::from(theme.gray_1));
            assert_eq!(theme.shell_border(), None);
            assert_eq!(theme.header_bg(), Hsla::from(theme.background));
            assert_eq!(theme.header_border(), Hsla::from(theme.gray_5));
            assert_eq!(theme.body_text(), Hsla::from(theme.text_primary));
            assert_eq!(theme.ring_offset(), Hsla::from(theme.gray_1));
            for step in 1..=12 {
                assert_eq!(theme.body_fill(step), theme.gray(step));
                assert_eq!(theme.body_hover_fill(step), theme.gray(step));
                assert_eq!(theme.body_border(step), theme.gray(step));
            }
        }
    }

    /// The `.cap-window-body` remaps are gated on
    /// `[data-macos-visual-system="liquid-glass"]`, so vibrancy must leave
    /// them alone and change only the shell and header.
    #[test]
    fn vibrancy_changes_the_shell_but_not_the_body() {
        for appearance in [Appearance::Light, Appearance::Dark] {
            let theme = Theme::new(appearance).with_material(Some(MaterialKind::Vibrancy));
            assert_ne!(theme.shell_bg(), Hsla::from(theme.gray_1));
            // `border: 1px solid var(--macos-settings-border)` -- only the
            // vibrancy path draws it.
            assert!(theme.shell_border().is_some());
            for step in 1..=12 {
                assert_eq!(theme.body_fill(step), theme.gray(step));
                assert_eq!(theme.body_hover_fill(step), theme.gray(step));
                assert_eq!(theme.body_border(step), theme.gray(step));
            }
        }
    }

    /// The exact `theme.css` mapping for material "panel" under Liquid Glass.
    #[test]
    fn liquid_glass_maps_body_fills_to_the_panel_material() {
        for appearance in [Appearance::Light, Appearance::Dark] {
            let theme = Theme::new(appearance).with_material(Some(MaterialKind::LiquidGlass));
            let material = theme.material.expect("material was just installed");

            // `.cap-window-shell { border: 0; box-shadow: none }`, then the
            // panel tint.
            assert_eq!(theme.shell_border(), None);
            assert_eq!(theme.shell_bg(), Hsla::from(material.shell));
            // `.cap-window-header { background: transparent;
            //  border-bottom-color: transparent }`
            assert_eq!(theme.header_bg().a, 0.);
            assert_eq!(theme.header_border(), gpui::transparent_black());
            assert_eq!(theme.body_text(), Hsla::from(material.text));
            // `.ring-offset-gray-1 { --tw-ring-offset-color: transparent }`
            assert_eq!(theme.ring_offset(), gpui::transparent_black());

            // `.bg-gray-2, .bg-gray-3 -> control-fill`, `.bg-gray-4 ->
            // control-hover`, `.bg-gray-5 -> control-active`.
            assert_eq!(theme.body_fill(2), Hsla::from(material.control_fill));
            assert_eq!(theme.body_fill(3), Hsla::from(material.control_fill));
            assert_eq!(theme.body_fill(4), Hsla::from(material.control_hover));
            assert_eq!(theme.body_fill(5), Hsla::from(material.control_active));
            // Untouched steps stay Radix.
            assert_eq!(theme.body_fill(6), theme.gray(6));

            // `hover:bg-gray-4|5 -> control-hover`, `hover:bg-gray-6|7 ->
            // selection`.
            assert_eq!(theme.body_hover_fill(4), Hsla::from(material.control_hover));
            assert_eq!(theme.body_hover_fill(5), Hsla::from(material.control_hover));
            assert_eq!(theme.body_hover_fill(6), Hsla::from(material.selection));
            assert_eq!(theme.body_hover_fill(7), Hsla::from(material.selection));

            // `border-gray-4|5|6 -> --macos-settings-border`.
            for step in 4..=6 {
                assert_eq!(theme.body_border(step), Hsla::from(material.border));
            }
            assert_eq!(theme.body_border(8), theme.gray(8));
        }
    }

    /// The settings window's own surfaces, material "settings".
    #[test]
    fn liquid_glass_settings_surfaces() {
        for appearance in [Appearance::Light, Appearance::Dark] {
            let theme = Theme::new(appearance).with_material(Some(MaterialKind::LiquidGlass));
            let material = theme.material.expect("material was just installed");

            // `--macos-settings-window-radius: 26px` under the glass block,
            // and the settings shell takes it unmodified -- 16 is the panel's
            // override, which must not leak over here.
            assert_eq!(theme.settings_window_radius(), 26.);
            assert_eq!(material.sidebar_radius, 18.);

            assert_eq!(theme.settings_sidebar_bg(), Hsla::from(material.sidebar));
            assert_eq!(theme.settings_content_bg(), Hsla::from(material.content));
            assert_eq!(theme.settings_card_bg(), Hsla::from(material.card));
            assert_eq!(theme.settings_fill(), Hsla::from(material.fill));
            assert_eq!(theme.settings_hover(), Hsla::from(material.hover));
            assert_eq!(theme.settings_selection(), Hsla::from(material.selection));
            assert_eq!(theme.settings_border(), Hsla::from(material.border));
            assert_eq!(theme.settings_divider(), Hsla::from(material.border));
            assert_eq!(theme.settings_text(), Hsla::from(material.text));
            assert_eq!(theme.settings_muted(), Hsla::from(material.muted));
        }

        // The sidebar is a wash over the live backdrop; the content pane is
        // opaque in both appearances.
        let light = Theme::light().with_material(Some(MaterialKind::LiquidGlass));
        assert!((light.settings_sidebar_bg().a - 0.78).abs() < 0.01);
        assert_eq!(light.settings_content_bg().a, 1.);
        let dark = Theme::dark().with_material(Some(MaterialKind::LiquidGlass));
        assert!((dark.settings_sidebar_bg().a - 0.88).abs() < 0.01);
        assert_eq!(dark.settings_content_bg().a, 1.);
    }

    #[test]
    fn vibrancy_settings_surfaces() {
        for appearance in [Appearance::Light, Appearance::Dark] {
            let theme = Theme::new(appearance).with_material(Some(MaterialKind::Vibrancy));
            assert_eq!(theme.settings_window_radius(), 16.);
            assert_eq!(theme.settings_divider(), Hsla::from(theme.border));
        }
        // No material at all: the Tailwind classes, unremapped.
        let bare = Theme::light();
        assert_eq!(bare.settings_sidebar_bg(), Hsla::from(bare.sidebar));
        assert_eq!(bare.settings_card_bg(), bare.gray(2));
        assert_eq!(bare.settings_fill(), bare.gray(3));
        assert_eq!(bare.settings_selection(), Hsla::from(bare.selection));
        assert_eq!(bare.settings_text(), bare.gray(12));
        assert_eq!(bare.settings_muted(), bare.gray(10));
        assert_eq!(bare.settings_window_radius(), 16.);
    }

    /// Material `"teleprompter"` differs from `"panel"` in exactly one thing:
    /// the radius. It gets no tint under Liquid Glass (that rule is
    /// `[..native-material="panel"]`-gated) and the shared vibrancy wash
    /// otherwise.
    #[test]
    fn the_teleprompter_material_is_the_shared_one_at_radius_22() {
        for appearance in [Appearance::Light, Appearance::Dark] {
            let glass = Theme::new(appearance).with_material(Some(MaterialKind::LiquidGlass));
            assert_eq!(glass.teleprompter_window_radius(), 22.);
            assert_eq!(glass.teleprompter_shell_bg(), gpui::transparent_black());
            assert_eq!(glass.teleprompter_shell_border(), None);
            // The panel's tint must not leak over: it is a different window.
            assert_ne!(glass.teleprompter_shell_bg(), glass.shell_bg());

            let vibrancy = Theme::new(appearance).with_material(Some(MaterialKind::Vibrancy));
            assert_eq!(vibrancy.teleprompter_window_radius(), 16.);
            // `background: var(--macos-settings-window)` -- the same value the
            // panel and settings shells take on this path.
            assert_eq!(vibrancy.teleprompter_shell_bg(), vibrancy.shell_bg());
            assert!(vibrancy.teleprompter_shell_border().is_some());
        }

        // No material at all: the route's own non-macOS classes.
        let bare = Theme::dark();
        assert_eq!(bare.teleprompter_window_radius(), 16.);
        assert!((bare.teleprompter_shell_bg().a - 0.9).abs() < 0.01);
        assert_eq!(bare.teleprompter_shell_border(), Some(bare.gray(5)));
    }

    /// The light panel tint is `rgba(255, 255, 255, 0.55)` and the dark one
    /// `rgba(17, 17, 17, 0.88)` -- both translucent, or there would be no
    /// glass to see.
    #[test]
    fn panel_glass_tint_is_translucent() {
        let light = Theme::light().with_material(Some(MaterialKind::LiquidGlass));
        let dark = Theme::dark().with_material(Some(MaterialKind::LiquidGlass));
        assert!((light.shell_bg().a - 0.55).abs() < 0.01);
        assert!((dark.shell_bg().a - 0.88).abs() < 0.01);
    }
}
