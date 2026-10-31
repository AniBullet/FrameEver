//! Embedded assets.
//!
//! There are only a few dozen files, so they are baked in with `include_bytes!`
//! rather than pulling in `rust_embed` for a directory walk we can spell out by
//! hand. The [`AssetSource`] impl is load-bearing, not just tidy: gpui's `svg()`
//! element resolves every icon path through it, and an unregistered path fails
//! silently -- the element lays out at its declared size and draws nothing.

use std::borrow::Cow;
use std::sync::atomic::{AtomicBool, Ordering};

use gpui::{App, AssetSource, Result, SharedString};

use crate::i18n::Language;

/// Expands to `(path, bytes)` pairs rooted at `assets/`.
macro_rules! assets {
    ($dir:literal: $($name:literal),* $(,)?) => {
        &[$((
            concat!($dir, "/", $name),
            include_bytes!(concat!("../assets/", $dir, "/", $name)) as &[u8],
        )),*]
    };
}

const FONTS: &[(&str, &[u8])] = assets!("fonts":
    "Geist.ttf",
    "Geist-Medium.ttf",
    "Geist-Bold.ttf",
    "Inter-Regular.ttf",
    "Inter-Medium.ttf",
    "Inter-SemiBold.ttf",
    "Inter-Bold.ttf",
    // Adobe, SIL Open Font License 1.1 (the licence is embedded in the font).
    "SourceSerif4-Light.ttf",
);

const CHINESE_UI_FONT: &[(&str, &[u8])] = assets!("fonts": "harmonyos-sans/HarmonyOS_Sans_SC.ttf");

static CHINESE_UI_FONT_LOADED: AtomicBool = AtomicBool::new(false);

const ICONS: &[(&str, &[u8])] = assets!("icons":
    "app-window.svg",
    "app-window-mac.svg",
    "arrow-left.svg",
    "arrow-left-right.svg",
    "arrow-left-to-line.svg",
    "arrow-right-to-line.svg",
    "arrow-up-right.svg",
    "ban.svg",
    "bell.svg",
    "blur.svg",
    "box-padding.svg",
    "caption-close-windows.svg",
    "caption-maximize-windows.svg",
    "caption-minimize-windows.svg",
    "caption-restore-windows.svg",
    "captions.svg",
    "check.svg",
    "chevron-down.svg",
    "chevron-left.svg",
    "chevron-right.svg",
    "chevron-up.svg",
    "circle.svg",
    "circle-check.svg",
    "circle-pause.svg",
    "circle-play.svg",
    "circle-plus.svg",
    "circle-question-mark.svg",
    "circle-stop.svg",
    "circle-x.svg",
    "clapperboard.svg",
    "cloud-upload.svg",
    "columns-2.svg",
    "command.svg",
    "copy.svg",
    "crop.svg",
    "download.svg",
    "eighth-notes.svg",
    "ellipsis.svg",
    "ellipsis-vertical.svg",
    "eye-off.svg",
    "film.svg",
    "folder.svg",
    "folder-down.svg",
    "folder-open.svg",
    "gauge.svg",
    "gem.svg",
    "globe.svg",
    "grip-vertical.svg",
    "image.svg",
    "import.svg",
    "info.svg",
    "keyboard.svg",
    "laptop.svg",
    "layers.svg",
    "link.svg",
    "logo-clip-bottom.svg",
    "logo-clip-top.svg",
    "logo-record.svg",
    "logo-record-dot.svg",
    "logo-record-gap.svg",
    "logo-tile.svg",
    "logo-wordmark.svg",
    "maximize.svg",
    "maximize-2.svg",
    "message-square-plus.svg",
    "mic.svg",
    "mic-off.svg",
    "minus.svg",
    "monitor.svg",
    "mouse-pointer-2.svg",
    "mouse-pointer-ban.svg",
    "move.svg",
    "move-left.svg",
    "move-right.svg",
    "palette.svg",
    "panel-right.svg",
    "pause.svg",
    "pencil.svg",
    "person-standing.svg",
    "play.svg",
    "plus.svg",
    "ratio.svg",
    "record-fill.svg",
    "rectangle-horizontal.svg",
    "redo-2.svg",
    "refresh-cw.svg",
    "rotate-3d.svg",
    "rotate-ccw.svg",
    "save.svg",
    "scan.svg",
    "scan-text.svg",
    "scissors.svg",
    "search.svg",
    "settings.svg",
    "settings-2.svg",
    "shadow.svg",
    "shuffle.svg",
    "skip-back.svg",
    "skip-forward.svg",
    "sliders-horizontal.svg",
    "square.svg",
    "square-dashed.svg",
    "square-pen.svg",
    "square-play.svg",
    "square-round-corner.svg",
    "terminal.svg",
    "text-align-center.svg",
    "text-align-end.svg",
    "text-align-start.svg",
    "traffic-close.svg",
    "trash.svg",
    "triangle-alert.svg",
    "triangles-centerline-dashed-horizontal.svg",
    "triangles-centerline-dashed-vertical.svg",
    "type.svg",
    "undo-2.svg",
    "upload.svg",
    "video.svg",
    "volume-2.svg",
    "volume-x.svg",
    "wand-sparkles.svg",
    "webcam.svg",
    "workflow.svg",
    "x.svg",
    "zap.svg",
    "zoom-in.svg",
    "zoom-out.svg",
);

/// Full-colour art, which `svg()` cannot draw -- it keeps only the alpha. The
/// three theme previews in the settings window's Appearance section are the
/// app's own `~/assets/theme-previews/*.jpg`, drawn with `img()`.
const IMAGES: &[(&str, &[u8])] = assets!("images":
    "auto.jpg",
    "light.jpg",
    "dark.jpg",
    "monitor-illustration.svg",
);

const ONBOARDING: &[(&str, &[u8])] = &[
    (
        "onboarding/music.mp3",
        include_bytes!("../assets/onboarding/tears-and-fireflies-adi-goldstein.mp3"),
    ),
    (
        "onboarding/cursor-macos.svg",
        include_bytes!("../assets/onboarding/cursor-macos.svg"),
    ),
    (
        "onboarding/cursor-windows.svg",
        include_bytes!("../assets/onboarding/cursor-windows.svg"),
    ),
    (
        "onboarding/volume-2.svg",
        include_bytes!("../assets/icons/volume-2.svg"),
    ),
    (
        "onboarding/volume-x.svg",
        include_bytes!("../assets/icons/volume-x.svg"),
    ),
];

pub struct Assets;

impl Assets {
    fn all() -> impl Iterator<Item = &'static (&'static str, &'static [u8])> {
        FONTS
            .iter()
            .chain(ICONS.iter())
            .chain(IMAGES.iter())
            .chain(ONBOARDING.iter())
    }
}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(Self::all()
            .find(|(name, _)| *name == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(Self::all()
            .filter(|(name, _)| name.starts_with(path))
            .map(|(name, _)| SharedString::from(*name))
            .collect())
    }
}

impl Assets {
    pub fn load_fonts(&self, cx: &App) -> Result<()> {
        cx.text_system().add_fonts(borrowed(FONTS))?;
        self.load_chinese_font_if_needed(cx)
    }

    pub fn load_chinese_font_if_needed(&self, cx: &App) -> Result<()> {
        if !needs_chinese_font(
            crate::i18n::current(),
            CHINESE_UI_FONT_LOADED.load(Ordering::Relaxed),
        ) {
            return Ok(());
        }
        cx.text_system().add_fonts(borrowed(CHINESE_UI_FONT))?;
        CHINESE_UI_FONT_LOADED.store(true, Ordering::Relaxed);
        Ok(())
    }
}

fn needs_chinese_font(language: Language, already_loaded: bool) -> bool {
    language == Language::SimplifiedChinese && !already_loaded
}

fn borrowed(fonts: &'static [(&'static str, &'static [u8])]) -> Vec<Cow<'static, [u8]>> {
    fonts
        .iter()
        .map(|(_, bytes)| Cow::Borrowed(*bytes))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every source file that draws icons; both directions of the asset checks
    /// scan the same list.
    const ICON_SOURCES: &[&str] = &[
        include_str!("main_window.rs"),
        include_str!("logo.rs"),
        // Not a window: the Recents card's per-kind pill and fallback glyphs
        // are named on `MediaKind`, next to the `Recents.tsx` lines they come
        // from, so the table has to scan here too.
        include_str!("library.rs"),
        include_str!("controls_window.rs"),
        include_str!("camera_window.rs"),
        include_str!("target_overlay.rs"),
        include_str!("settings_window.rs"),
        include_str!("settings_pages.rs"),
        include_str!("mode_select_window.rs"),
        include_str!("teleprompter_window.rs"),
        include_str!("editor_window.rs"),
        include_str!("editor_window/frame.rs"),
        include_str!("editor_window/scenes.rs"),
        // The timeline's nine track glyphs and its scene-mode icons are named
        // in the strip's own module, not in the window that hosts it.
        include_str!("editor_timeline.rs"),
        // The config sidebar's field glyphs and the background section's
        // source tiles, same split.
        include_str!("editor_sidebar.rs"),
        // The five later tabs, the colour-grade section and the eight segment
        // panels each name their own glyphs.
        include_str!("editor_tabs.rs"),
        // The cursor tab's style picker and ripple section live one directory
        // down, and the ripple field names its own glyph.
        include_str!("editor_sidebar/cursor.rs"),
        include_str!("editor_color.rs"),
        include_str!("editor_panels.rs"),
        // The crop dialog's ratio / Full / Reset glyphs and the chevron
        // between its two panes.
        include_str!("editor_crop.rs"),
        // The clips sidebar's cards, import menu and record modal.
        include_str!("editor_clips.rs"),
        // The export page's destination cards and share-status glyphs.
        include_str!("editor_export.rs"),
        // The screenshot editor's toolbar, popovers and zoom HUD; the
        // annotation module names the tool and layer glyphs.
        include_str!("screenshot_editor.rs"),
        include_str!("screenshot_annotations.rs"),
        // The crop overlay's own ratio glyph.
        include_str!("screenshot_crop.rs"),
        // `ui::SelectionHeader` names the check and the trash itself.
        include_str!("ui/selection_header.rs"),
        include_str!("ui/radio_cards.rs"),
        include_str!("ui/windows_caption.rs"),
        // The onboarding window's welcome cards and permissions surface; the
        // per-permission row glyphs are named on `OSPermission::icon`.
        include_str!("onboarding_window.rs"),
        include_str!("permissions.rs"),
    ];

    /// Every icon a window asks for must be in the table. gpui draws
    /// nothing at all for a path it cannot resolve, and does not log, so a typo
    /// here is otherwise only visible by looking at the window.
    #[test]
    fn every_referenced_icon_is_embedded() {
        let source = ICON_SOURCES.concat();
        let source = source.as_str();

        let referenced: Vec<&str> = source
            .match_indices("\"icons/")
            .filter_map(|(start, _)| {
                let rest = &source[start + 1..];
                rest.split('"').next()
            })
            .filter(|path| path.ends_with(".svg"))
            .collect();

        assert!(
            !referenced.is_empty(),
            "found no icon references to check -- has main_window.rs stopped \
             spelling paths as string literals?"
        );

        let missing: Vec<&str> = referenced
            .into_iter()
            .filter(|path| Assets.load(path).unwrap().is_none())
            .collect();

        assert!(
            missing.is_empty(),
            "icons referenced but not embedded: {missing:?}"
        );
    }

    /// And the other direction: an icon that stopped being drawn during a
    /// refactor is dead weight in the binary, and nothing else would notice.
    #[test]
    fn every_embedded_icon_is_referenced() {
        let source = ICON_SOURCES.concat();
        let source = source.as_str();

        let unused: Vec<&str> = ICONS
            .iter()
            .map(|(path, _)| *path)
            .filter(|path| !source.contains(path))
            .collect();

        assert!(
            unused.is_empty(),
            "icons embedded but never drawn: {unused:?}"
        );
    }

    #[test]
    fn every_icon_file_is_in_the_table_or_included_by_source() {
        let assets_source = include_str!("assets.rs");
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/icons");

        let mut orphans: Vec<String> = std::fs::read_dir(&directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".svg"))
            .filter(|name| {
                Assets.load(&format!("icons/{name}")).unwrap().is_none()
                    && !assets_source.contains(&format!("\"../assets/icons/{name}\""))
            })
            .collect();
        orphans.sort();

        assert!(
            orphans.is_empty(),
            "icon files neither embedded nor included: {orphans:?}"
        );
    }

    #[test]
    fn icon_licence_notices_ship_beside_the_icons_and_in_the_installer() {
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let notices: &[(&str, &[&str])] = &[
            (
                "LICENSE-lucide.txt",
                &[
                    "ISC License",
                    "Copyright (c) 2026 Lucide Icons and Contributors",
                    "The MIT License (MIT) (for the icons listed above)",
                    "Copyright (c) 2013-present Cole Bemis",
                ],
            ),
            (
                "LICENSE-tabler.txt",
                &["MIT License", "Copyright (c) 2020-2026 Paweł Kuna"],
            ),
        ];
        for (file, required) in notices {
            let text = std::fs::read_to_string(manifest.join("assets/icons").join(file))
                .unwrap_or_else(|error| panic!("{file}: {error}"));
            for line in *required {
                assert!(text.contains(line), "{file} lacks {line:?}");
            }
        }

        let config: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(manifest.join("tauri.conf.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            config["bundle"]["resources"]["assets/icons/LICENSE-*.txt"],
            "licenses/icons/"
        );
        assert_eq!(
            config["bundle"]["resources"]["assets/fonts/harmonyos-sans/LICENSE-*.txt"],
            "licenses/fonts/"
        );
    }

    #[test]
    fn the_phosphor_notice_is_gone_and_about_credits_both_icon_families() {
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        assert!(!manifest.join("assets/icons/LICENSE-phosphor.txt").exists());

        let about = include_str!("settings_window.rs");
        assert!(about.contains("Icons: Lucide (ISC)"));
        assert!(about.contains("Icons: Tabler (MIT)"));
        assert!(!about.contains("Phosphor"));
    }

    fn icon_sources() -> serde_json::Value {
        serde_json::from_str(include_str!("../assets/icons/sources.json")).unwrap()
    }

    fn names(value: &serde_json::Value) -> Vec<String> {
        value
            .as_array()
            .unwrap()
            .iter()
            .map(|name| name.as_str().unwrap().to_owned())
            .collect()
    }

    fn vendored_icons() -> Vec<(String, &'static str)> {
        let sources = icon_sources();
        let lucide = names(&sources["lucide"])
            .into_iter()
            .chain(sources["lucideFilled"].as_object().unwrap().keys().cloned());
        lucide
            .map(|name| (name, "lucide"))
            .chain(
                names(&sources["tabler"])
                    .into_iter()
                    .map(|name| (name, "tabler")),
            )
            .collect()
    }

    #[test]
    fn every_icon_file_is_listed_in_the_sources() {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/icons");
        let mut on_disk: Vec<String> = std::fs::read_dir(&directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter_map(|name| name.strip_suffix(".svg").map(str::to_owned))
            .collect();
        on_disk.sort();

        let mut listed: Vec<String> = vendored_icons()
            .into_iter()
            .map(|(name, _)| name)
            .chain(names(&icon_sources()["custom"]))
            .collect();
        listed.sort();

        assert_eq!(on_disk, listed);
    }

    #[test]
    fn every_vendored_icon_comes_from_its_pinned_release_at_the_design_stroke() {
        let sources = icon_sources();
        for (name, family) in vendored_icons() {
            let release = &sources["releases"][family];
            let stamp = format!(
                "@license {} v{} - {}",
                release["package"].as_str().unwrap(),
                release["version"].as_str().unwrap(),
                release["license"].as_str().unwrap(),
            );
            let path = format!("icons/{name}.svg");
            let bytes = Assets.load(&path).unwrap();
            let bytes = bytes.unwrap_or_else(|| panic!("{path} is not embedded"));
            let svg = std::str::from_utf8(&bytes).unwrap();
            assert!(svg.starts_with(&format!("<!-- {stamp} -->")), "{path}");
            assert!(svg.contains(r#"viewBox="0 0 24 24""#), "{path}");
            assert!(svg.contains(r#"stroke-width="1.75""#), "{path}");
        }
    }

    #[test]
    fn every_icon_rasterises_to_visible_pixels() {
        let options = resvg::usvg::Options::default();
        for (path, bytes) in ICONS {
            let tree = resvg::usvg::Tree::from_data(bytes, &options)
                .unwrap_or_else(|error| panic!("{path}: {error}"));
            let mut pixmap = resvg::tiny_skia::Pixmap::new(48, 48).unwrap();
            let scale = 48. / tree.size().width().max(tree.size().height());
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::from_scale(scale, scale),
                &mut pixmap.as_mut(),
            );
            assert!(
                pixmap.pixels().iter().any(|pixel| pixel.alpha() > 0),
                "{path} draws nothing"
            );
        }
    }

    #[test]
    fn the_custom_icons_are_the_brand_and_window_chrome_only() {
        let custom = names(&icon_sources()["custom"]);
        assert_eq!(custom.len(), 12);
        for name in custom {
            let path = format!("icons/{name}.svg");
            let bytes = Assets.load(&path).unwrap().unwrap();
            let svg = std::str::from_utf8(&bytes).unwrap();
            assert!(!svg.contains("@license"), "{path}");
        }
        assert!(
            Assets
                .load("images/monitor-illustration.svg")
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn the_modes_and_automations_use_their_decided_icons() {
        assert!(include_str!("main_window.rs").contains(r#"Self::Instant => "icons/zap.svg""#));
        assert!(
            include_str!("main_window.rs").contains(r#"Self::Studio => "icons/clapperboard.svg""#)
        );
        assert!(
            include_str!("main_window.rs").contains(r#"Self::Screenshot => "icons/image.svg""#)
        );
        assert!(
            include_str!("target_overlay.rs").contains(r#"Mode::Screenshot => "icons/image.svg""#)
        );
        assert!(
            include_str!("settings_window.rs")
                .contains(r#"Self::Automations => "icons/workflow.svg""#)
        );
    }

    /// Same contract for the bitmaps, which `img()` resolves through the
    /// identical `AssetSource` path and fails just as silently.
    #[test]
    fn every_referenced_image_is_embedded_and_vice_versa() {
        let source = ICON_SOURCES.concat();
        let source = source.as_str();

        let referenced: Vec<&str> = source
            .match_indices("\"images/")
            .filter_map(|(start, _)| source[start + 1..].split('"').next())
            .collect();
        assert!(!referenced.is_empty(), "found no image references to check");

        let missing: Vec<&str> = referenced
            .into_iter()
            .filter(|path| Assets.load(path).unwrap().is_none())
            .collect();
        assert!(
            missing.is_empty(),
            "images referenced but not embedded: {missing:?}"
        );

        let unused: Vec<&str> = IMAGES
            .iter()
            .map(|(path, _)| *path)
            .filter(|path| !source.contains(path))
            .collect();
        assert!(
            unused.is_empty(),
            "images embedded but never drawn: {unused:?}"
        );
    }

    #[test]
    fn only_the_chinese_ui_loads_the_chinese_font_and_only_once() {
        assert!(!needs_chinese_font(Language::English, false));
        assert!(needs_chinese_font(Language::SimplifiedChinese, false));
        assert!(!needs_chinese_font(Language::SimplifiedChinese, true));
        assert!(!FONTS.iter().any(|(name, _)| name.contains("HarmonyOS")));
        assert!(Assets.load("fonts/Inter-Regular.ttf").unwrap().is_some());
    }

    #[test]
    fn fonts_and_icons_resolve() {
        assert!(Assets.load("fonts/Geist.ttf").unwrap().is_some());
        assert!(Assets.load("icons/webcam.svg").unwrap().is_some());
        assert!(Assets.load("icons/nope.svg").unwrap().is_none());
        assert!(Assets.load("images/auto.jpg").unwrap().is_some());
        assert_eq!(Assets.list("fonts").unwrap().len(), FONTS.len());
        assert_eq!(Assets.list("icons").unwrap().len(), ICONS.len());
        assert_eq!(Assets.list("images").unwrap().len(), IMAGES.len());
    }
}
