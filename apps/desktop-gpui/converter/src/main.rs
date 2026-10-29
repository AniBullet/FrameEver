#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::{borrow::Cow, sync::Arc};

use frameever_converter::{Target, find_installs_on_this_machine};
use frameever_converter_app::{
    i18n,
    theme::Palette,
    wizard::{Action, Button, Row, Screen, Wizard},
};
use gpui::{
    App, AppContext, AssetSource, Bounds, Context, FontWeight, Image, ImageFormat,
    InteractiveElement, IntoElement, ParentElement, PathPromptOptions, Render, Rgba, SharedString,
    StatefulInteractiveElement, Styled, TitlebarOptions, Window, WindowBackgroundAppearance,
    WindowBounds, WindowControlArea, WindowOptions, div, img, px, rgb, size, svg,
};

const WINDOW_WIDTH: f32 = 560.;
const WINDOW_HEIGHT: f32 = 560.;
const CAPTION_HEIGHT: f32 = 40.;
const WINDOW_TITLE: &str = "FrameEver Converter";
const ICONS: &[(&str, &[u8])] = &[
    (
        "icons/caption-minimize-windows.svg",
        include_bytes!("../../assets/icons/caption-minimize-windows.svg"),
    ),
    (
        "icons/caption-close-windows.svg",
        include_bytes!("../../assets/icons/caption-close-windows.svg"),
    ),
];
const APP_ICON: &[u8] = include_bytes!("../../assets/dock-icon.png");
const FONTS: &[&[u8]] = &[
    include_bytes!("../../assets/fonts/Inter-Regular.ttf"),
    include_bytes!("../../assets/fonts/Inter-Medium.ttf"),
    include_bytes!("../../assets/fonts/Inter-SemiBold.ttf"),
];

struct ConverterWindow {
    wizard: Wizard,
    icon: Arc<Image>,
}

impl ConverterWindow {
    fn act(&mut self, action: Action, cx: &mut Context<Self>) {
        match action {
            Action::Choose(index) => self.wizard.choose(index),
            Action::Back | Action::StartOver => self.wizard.back(),
            Action::PickFolder => self.pick_folder(cx),
            Action::Convert => self.convert(cx),
            Action::OpenDestination => {
                if let Some(folder) = self.wizard.destination() {
                    cx.open_with_system(folder);
                }
            }
            Action::Close => cx.quit(),
        }
        cx.notify();
    }

    fn pick_folder(&mut self, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(folder) = paths.into_iter().next() else {
                return;
            };
            this.update(cx, |this, cx| {
                this.wizard.choose_folder(&folder);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn convert(&mut self, cx: &mut Context<Self>) {
        let Some(job) = self.wizard.convert() else {
            return;
        };
        let running = cx.background_executor().spawn(async move { job.run() });
        cx.spawn(async move |this, cx| {
            let report = running.await;
            this.update(cx, |this, cx| {
                this.wizard.finish(report);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn row(
        &self,
        index: usize,
        row: Row,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let card = div()
            .id(("row", index))
            .flex()
            .flex_row()
            .items_center()
            .gap_4()
            .px_4()
            .py(px(10.))
            .rounded(px(10.))
            .border_1()
            .border_color(palette.border)
            .bg(palette.card)
            .child(
                div()
                    .flex_shrink_0()
                    .font_weight(FontWeight::MEDIUM)
                    .child(row.title),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_right()
                    .text_sm()
                    .text_color(if row.warning {
                        palette.warning
                    } else {
                        palette.muted
                    })
                    .child(row.detail),
            );
        match row.action {
            Some(action) => card
                .cursor_pointer()
                .hover(move |style| style.bg(palette.card_hover))
                .child(div().text_color(palette.muted).child("›"))
                .on_click(cx.listener(move |this, _, _, cx| this.act(action, cx))),
            None => card,
        }
    }

    fn button(
        &self,
        button: Button,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let action = button.action;
        let base = div()
            .id(("button", action_id(action)))
            .flex()
            .items_center()
            .h(px(32.))
            .px(px(14.))
            .rounded(px(8.))
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .cursor_pointer()
            .child(button.label)
            .on_click(cx.listener(move |this, _, _, cx| this.act(action, cx)));
        if button.primary {
            base.bg(palette.primary)
                .text_color(palette.on_primary)
                .hover(move |style| style.bg(palette.primary_hover))
        } else {
            base.border_1()
                .border_color(palette.border)
                .bg(palette.card)
                .hover(move |style| style.bg(palette.card_hover))
        }
    }
}

fn action_id(action: Action) -> usize {
    match action {
        Action::Choose(index) => 100 + index,
        Action::PickFolder => 0,
        Action::Back => 1,
        Action::Convert => 2,
        Action::OpenDestination => 3,
        Action::StartOver => 4,
        Action::Close => 5,
    }
}

impl Render for ConverterWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = Palette::for_appearance(window.appearance());
        let Screen {
            title,
            subtitle,
            rows,
            notes,
            buttons,
        } = self.wizard.screen();
        let rows: Vec<_> = rows
            .into_iter()
            .enumerate()
            .map(|(index, row)| self.row(index, row, palette, cx))
            .collect();
        let buttons: Vec<_> = buttons
            .into_iter()
            .map(|button| self.button(button, palette, cx))
            .collect();
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(palette.background)
            .text_color(palette.text)
            .font_family(i18n::ui_font())
            .child(caption(
                self.icon.clone(),
                palette,
                window.is_window_active(),
                !self.wizard.is_converting(),
            ))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .px(px(28.))
                    .pt(px(12.))
                    .pb(px(24.))
                    .child(page(title, subtitle, palette))
                    .child(
                        div()
                            .id("rows")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .children(rows),
                    )
                    .children(
                        notes
                            .into_iter()
                            .map(|note| div().text_xs().text_color(palette.muted).child(note)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_end()
                            .gap_2()
                            .children(buttons),
                    ),
            )
    }
}

fn page(title: String, subtitle: String, palette: Palette) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_xl()
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(div().text_sm().text_color(palette.muted).child(subtitle))
}

fn caption(icon: Arc<Image>, palette: Palette, active: bool, closable: bool) -> impl IntoElement {
    let glyph = if active { palette.text } else { palette.muted };
    let control = move |id: &'static str, path: &'static str, height: f32, hover: Rgba| {
        div()
            .id(id)
            .w(px(46.))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .occlude()
            .hover(move |style| style.bg(hover))
            .child(svg().path(path).w(px(10.)).h(px(height)).text_color(glyph))
    };
    div()
        .h(px(CAPTION_HEIGHT))
        .flex_shrink_0()
        .flex()
        .flex_row()
        .child(
            div()
                .flex_1()
                .h_full()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .pl(px(28.))
                .window_control_area(WindowControlArea::Drag)
                .child(img(icon).size(px(20.)).rounded(px(5.)))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(WINDOW_TITLE),
                ),
        )
        .child(
            control(
                "caption-minimize",
                "icons/caption-minimize-windows.svg",
                1.,
                palette.card_hover,
            )
            .on_click(|_, window, _| window.minimize_window()),
        )
        .child(
            control(
                "caption-close",
                "icons/caption-close-windows.svg",
                10.,
                rgb(0xc42b1c),
            )
            .on_click(move |_, window, _| {
                if closable {
                    window.remove_window();
                }
            }),
        )
}

struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        Ok(ICONS
            .iter()
            .find(|(name, _)| *name == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, _: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(ICONS.iter().map(|(name, _)| (*name).into()).collect())
    }
}

fn main() {
    let target = Target::from_environment();
    i18n::set(i18n::resolve(
        i18n::language_setting(&target.settings_file).as_deref(),
    ));
    let app = gpui_platform::application().with_assets(Assets);
    app.run(move |cx: &mut App| {
        if let Err(error) = cx
            .text_system()
            .add_fonts(FONTS.iter().map(|font| Cow::Borrowed(*font)).collect())
        {
            eprintln!("failed to load fonts: {error:#}");
        }
        let bounds = Bounds::centered(None, size(px(WINDOW_WIDTH), px(WINDOW_HEIGHT)), cx);
        let opened = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some(WINDOW_TITLE.into()),
                    appears_transparent: true,
                    ..Default::default()
                }),
                window_background: WindowBackgroundAppearance::Opaque,
                is_resizable: false,
                ..Default::default()
            },
            move |window, cx| {
                let view = cx.new(|cx| {
                    cx.observe_window_appearance(window, |_, _, cx| cx.notify())
                        .detach();
                    ConverterWindow {
                        wizard: Wizard::new(target, Box::new(find_installs_on_this_machine)),
                        icon: Arc::new(Image::from_bytes(ImageFormat::Png, APP_ICON.to_vec())),
                    }
                });
                let watched = view.downgrade();
                window.on_window_should_close(cx, move |_, cx| {
                    watched
                        .upgrade()
                        .is_none_or(|view| !view.read(cx).wizard.is_converting())
                });
                view
            },
        );
        if let Err(error) = opened {
            eprintln!("failed to open the converter window: {error:#}");
            cx.quit();
        }
        cx.on_window_closed(|cx, _| cx.quit()).detach();
    });
}
