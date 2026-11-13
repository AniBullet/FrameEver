use std::{sync::Arc, time::Duration};

use frameever_recording::sources::screen_capture::ScreenCaptureTarget;
use gpui::{
    Context, IntoElement, ParentElement as _, RenderImage, Styled, StyledImage as _, Window, div,
    img, px,
};
use image::RgbaImage;

use super::{AreaRect, Mode, OverlayWindow, TargetType, repaint};
use crate::i18n::t;

pub(super) enum Capture {
    Empty,
    Loading,
    Ready {
        pixels: RgbaImage,
        background: Arc<RenderImage>,
    },
    Failed,
}

enum SelectedImage {
    Wait,
    Live,
    Frozen(image::DynamicImage),
}

impl Capture {
    fn selected_image(
        &self,
        target: &ScreenCaptureTarget,
        display_id: &scap_targets::DisplayId,
        display: (f32, f32),
        active: bool,
        current: bool,
    ) -> anyhow::Result<SelectedImage> {
        let ScreenCaptureTarget::Area { screen, bounds } = target else {
            return Ok(SelectedImage::Live);
        };
        if !active || screen != display_id {
            return Ok(SelectedImage::Live);
        }
        if !current || matches!(self, Self::Empty | Self::Loading) {
            return Ok(SelectedImage::Wait);
        }
        let Self::Ready { pixels, .. } = self else {
            return Ok(SelectedImage::Live);
        };
        anyhow::ensure!(display.0 > 0. && display.1 > 0., "invalid display size");
        let scale_x = pixels.width() as f32 / display.0;
        let scale_y = pixels.height() as f32 / display.1;
        let x = (bounds.position().x() as f32 * scale_x).floor();
        let y = (bounds.position().y() as f32 * scale_y).floor();
        let width = (bounds.size().width() as f32 * scale_x).round();
        let height = (bounds.size().height() as f32 * scale_y).round();
        anyhow::ensure!(
            x.is_finite()
                && y.is_finite()
                && width.is_finite()
                && height.is_finite()
                && x >= 0.
                && y >= 0.
                && width > 0.
                && height > 0.
                && x < pixels.width() as f32
                && y < pixels.height() as f32,
            "selected area does not fit the frozen display"
        );
        let x = x as u32;
        let y = y as u32;
        let width = (width as u32).min(pixels.width() - x);
        let height = (height as u32).min(pixels.height() - y);
        Ok(SelectedImage::Frozen(image::DynamicImage::ImageRgba8(
            image::imageops::crop_imm(pixels, x, y, width, height).to_image(),
        )))
    }
}

fn capture_active(mode: Mode, target: Option<TargetType>, recording: bool) -> bool {
    mode == Mode::Screenshot && target == Some(TargetType::Area) && !recording
}

impl OverlayWindow {
    pub(super) fn sync_screenshot_frame(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let select = self.select.read(cx);
        let epoch = select.capture_epoch;
        let active = capture_active(select.recording_mode, select.mode, self.recording_area);
        if self.screenshot_frame_epoch != epoch || !active {
            self.screenshot_frame_task = None;
            self.screenshot_frame = Capture::Empty;
            self.magnifier_cursor = None;
            self.screenshot_frame_epoch = epoch;
        }
        if !active || !matches!(self.screenshot_frame, Capture::Empty) {
            return;
        }
        self.cancel_area_drag(cx);
        self.screenshot_frame = Capture::Loading;
        let id = self.display_id.clone();
        let task = gpui_tokio::Tokio::spawn_result(cx, async move {
            tokio::time::sleep(Duration::from_millis(150)).await;
            let pixels = tokio::time::timeout(
                Duration::from_millis(1500),
                frameever_recording::screenshot::capture_screenshot(ScreenCaptureTarget::Display {
                    id,
                }),
            )
            .await??
            .into_rgba8();
            anyhow::ensure!(
                pixels.width() > 0 && pixels.height() > 0,
                "empty display frame"
            );
            let background = crate::library::rgba_to_render_image(pixels.clone());
            Ok((pixels, background))
        });
        self.screenshot_frame_task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update_in(cx, |this, window, cx| {
                let select = this.select.read(cx);
                if select.capture_epoch != epoch
                    || !capture_active(select.recording_mode, select.mode, this.recording_area)
                {
                    return;
                }
                this.screenshot_frame = match result {
                    Ok((pixels, background)) => Capture::Ready { pixels, background },
                    Err(error) => {
                        tracing::error!(display = %this.display_id, %error, "screenshot magnifier frame capture failed");
                        Capture::Failed
                    }
                };
                repaint(window, cx);
            })
            .ok();
        }));
    }

    pub(super) fn screenshot_frame_loading(&self) -> bool {
        matches!(self.screenshot_frame, Capture::Loading)
    }

    pub(super) fn take_selected_screenshot(
        &mut self,
        target: ScreenCaptureTarget,
        cx: &mut Context<Self>,
    ) {
        let select = self.select.read(cx);
        let image = match self.screenshot_frame.selected_image(
            &target,
            &self.display_id,
            self.display_size,
            capture_active(select.recording_mode, select.mode, self.recording_area),
            self.screenshot_frame_epoch == select.capture_epoch,
        ) {
            Ok(SelectedImage::Wait) => {
                cx.notify();
                return;
            }
            Ok(SelectedImage::Live) => None,
            Ok(SelectedImage::Frozen(image)) => Some(image),
            Err(error) => {
                tracing::error!(display = %self.display_id, %error, "selected screenshot area is outside its frozen frame");
                self.screenshot_frame = Capture::Failed;
                cx.notify();
                return;
            }
        };
        cx.defer(move |cx| match image {
            Some(image) => crate::screenshot::take_screenshot_from_image(target, image, cx),
            None => crate::screenshot::take_screenshot(target, cx),
        });
    }

    pub(super) fn render_screenshot_background(&self) -> Option<gpui::AnyElement> {
        let Capture::Ready { background, .. } = &self.screenshot_frame else {
            return None;
        };
        Some(
            img(background.clone())
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .object_fit(gpui::ObjectFit::Fill)
                .into_any_element(),
        )
    }

    pub(super) fn render_magnifier(&self, window: &Window) -> Option<gpui::AnyElement> {
        if matches!(self.screenshot_frame, Capture::Failed) {
            return Some(
                div()
                    .absolute()
                    .left(px(8.))
                    .top(px(8.))
                    .px(px(12.))
                    .py(px(8.))
                    .rounded(px(6.))
                    .bg(self.theme.popover)
                    .text_color(self.theme.foreground)
                    .text_size(px(12.))
                    .child(t!(
                        "Couldn't load magnifier. You can still take a screenshot."
                    ))
                    .into_any_element(),
            );
        }
        let Capture::Ready { pixels, .. } = &self.screenshot_frame else {
            return None;
        };
        self.drag.as_ref()?;
        let cursor = self.magnifier_cursor?;
        let scale = window.scale_factor();
        let border = 1. / scale;
        let side = 120. + border * 2.;
        let place = |cursor: f32, limit: f32| {
            let origin = if cursor + 20. + side <= limit {
                cursor + 20.
            } else {
                cursor - 20. - side
            };
            (origin.clamp(0., (limit - side).max(0.)) * scale).round() / scale
        };
        let sample = sample(pixels, cursor, self.display_size);
        let mut grid = div().relative().size(px(120.));
        for (x, y, color) in sample.enumerate_pixels() {
            grid = grid.child(
                div()
                    .absolute()
                    .left(px(x as f32 * 8.))
                    .top(px(y as f32 * 8.))
                    .size(px(8.))
                    .bg(gpui::rgba(u32::from_be_bytes(color.0))),
            );
        }
        grid = grid.child(
            div()
                .absolute()
                .left(px(56.))
                .top(px(56.))
                .size(px(8.))
                .border(px(border))
                .border_color(self.theme.background)
                .child(
                    div()
                        .size_full()
                        .border(px(border))
                        .border_color(self.theme.foreground),
                ),
        );
        Some(
            div()
                .absolute()
                .left(px(place(cursor.0, self.display_size.0)))
                .top(px(place(cursor.1, self.display_size.1)))
                .size(px(side))
                .border(px(border))
                .border_color(self.theme.border)
                .bg(self.theme.popover)
                .child(grid)
                .into_any_element(),
        )
    }
}

pub(super) fn sample(frame: &RgbaImage, cursor: (f32, f32), display: (f32, f32)) -> RgbaImage {
    let center_x = (cursor.0 * frame.width() as f32 / display.0).floor() as i64;
    let center_y = (cursor.1 * frame.height() as f32 / display.1).floor() as i64;
    RgbaImage::from_fn(15, 15, |x, y| {
        *frame.get_pixel(
            (center_x + i64::from(x) - 7).clamp(0, i64::from(frame.width()) - 1) as u32,
            (center_y + i64::from(y) - 7).clamp(0, i64::from(frame.height()) - 1) as u32,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magnifier_samples_fifteen_original_pixels_at_each_display_scale() {
        let frame = RgbaImage::from_fn(120, 90, |x, y| image::Rgba([x as u8, y as u8, 40, 255]));
        for scale in [1., 1.5, 2.] {
            let sample = sample(
                &frame,
                (40. / scale, 30. / scale),
                (120. / scale, 90. / scale),
            );
            assert_eq!(sample.dimensions(), (15, 15));
            assert_eq!(sample.get_pixel(0, 0).0, [33, 23, 40, 255]);
            assert_eq!(sample.get_pixel(7, 7).0, [40, 30, 40, 255]);
            assert_eq!(sample.get_pixel(14, 14).0, [47, 37, 40, 255]);
        }
        let edge = sample(&frame, (0., 0.), (120., 90.));
        assert_eq!(edge.get_pixel(0, 0).0, [0, 0, 40, 255]);
        assert_eq!(edge.get_pixel(7, 7).0, [0, 0, 40, 255]);
        assert_eq!(edge.get_pixel(14, 14).0, [7, 7, 40, 255]);
    }

    #[test]
    fn magnifier_and_saved_crop_use_the_same_frozen_pixels() {
        let frame = RgbaImage::from_fn(120, 90, |x, y| image::Rgba([x as u8, y as u8, 40, 255]));
        let display_id = "1".parse::<scap_targets::DisplayId>().unwrap();
        let other_id = "2".parse::<scap_targets::DisplayId>().unwrap();
        let capture = Capture::Ready {
            background: crate::library::rgba_to_render_image(frame.clone()),
            pixels: frame.clone(),
        };
        for scale in [1., 1.5, 2.] {
            let display = (120. / scale, 90. / scale);
            let lens = sample(&frame, (40. / scale, 30. / scale), display);
            let target = ScreenCaptureTarget::Area {
                screen: display_id.clone(),
                bounds: AreaRect {
                    x: 38. / scale,
                    y: 28. / scale,
                    width: 5. / scale,
                    height: 5. / scale,
                }
                .to_bounds(),
            };
            let SelectedImage::Frozen(output) = capture
                .selected_image(&target, &display_id, display, true, true)
                .expect("the selected area is inside the frozen display")
            else {
                panic!("the current area must use the frozen display frame");
            };
            let output = output.to_rgba8();
            assert_eq!(output.dimensions(), (5, 5));
            assert_eq!(output.get_pixel(0, 0).0, [38, 28, 40, 255]);
            assert_eq!(output.get_pixel(2, 2), lens.get_pixel(7, 7));
            assert_eq!(output.get_pixel(4, 4).0, [42, 32, 40, 255]);
            assert!(matches!(
                capture.selected_image(&target, &display_id, display, true, false),
                Ok(SelectedImage::Wait)
            ));
            assert!(matches!(
                capture.selected_image(&target, &other_id, display, true, true),
                Ok(SelectedImage::Live)
            ));
            assert!(matches!(
                capture.selected_image(&target, &display_id, display, false, true),
                Ok(SelectedImage::Live)
            ));
            assert!(matches!(
                Capture::Failed.selected_image(&target, &display_id, display, true, true),
                Ok(SelectedImage::Live)
            ));
            assert!(matches!(
                capture.selected_image(
                    &ScreenCaptureTarget::Display {
                        id: display_id.clone()
                    },
                    &display_id,
                    display,
                    false,
                    false,
                ),
                Ok(SelectedImage::Live)
            ));
        }
    }

    #[gpui::test]
    fn magnifier_rejects_a_frame_after_recording_mode_changes(cx: &mut gpui::TestAppContext) {
        struct ResetStore;
        impl Drop for ResetStore {
            fn drop(&mut self) {
                crate::store::set_test_tauri_store_path(None);
            }
        }
        let directory = tempfile::tempdir().unwrap();
        crate::store::set_test_tauri_store_path(Some(directory.path().join("store")));
        let _reset = ResetStore;
        cx.update(|cx| {
            let select = super::super::TargetSelect::init(cx);
            select.update(cx, |select, cx| {
                select.set_recording_mode(Mode::Screenshot, cx)
            });
            let epoch = select.read(cx).capture_epoch;
            let frame = RgbaImage::from_pixel(20, 20, image::Rgba([40, 50, 60, 255]));
            let capture = Capture::Ready {
                background: crate::library::rgba_to_render_image(frame.clone()),
                pixels: frame,
            };
            select.update(cx, |select, cx| {
                select.set_recording_mode(Mode::Instant, cx)
            });
            select.update(cx, |select, cx| {
                select.set_recording_mode(Mode::Screenshot, cx)
            });
            let display_id = "1".parse::<scap_targets::DisplayId>().unwrap();
            let target = ScreenCaptureTarget::Area {
                screen: display_id.clone(),
                bounds: AreaRect {
                    x: 1.,
                    y: 1.,
                    width: 5.,
                    height: 5.,
                }
                .to_bounds(),
            };
            assert!(matches!(
                capture.selected_image(
                    &target,
                    &display_id,
                    (20., 20.),
                    true,
                    select.read(cx).capture_epoch == epoch
                ),
                Ok(SelectedImage::Wait)
            ));
        });
    }
}
