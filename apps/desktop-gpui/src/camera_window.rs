mod layout;

use std::sync::{
    Arc,
    atomic::{AtomicU32, AtomicU64, Ordering},
};
use std::time::{Duration, Instant};

use gpui::{Animation, AnimationExt as _, StyledImage as _};
use gpui::{
    AppContext as _, Context, Entity, FontWeight, InteractiveElement as _, IntoElement,
    MouseButton, MouseMoveEvent, MouseUpEvent, ParentElement as _, Render,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Subscription, WeakEntity, Window,
    div, prelude::FluentBuilder as _, px, svg,
};
use serde_json::Value;

use crate::{
    camera_loading,
    feeds::Feeds,
    i18n::t,
    platform,
    store::{self, BlurMode, CameraShape, CameraWindowState},
    theme::Theme,
};

#[cfg(target_os = "macos")]
use crate::camera_blur;

pub const CAMERA_MIN_SIZE: f32 = 150.;
pub const CAMERA_MAX_SIZE: f32 = 600.;
pub const CAMERA_DEFAULT_SIZE: f32 = 230.;
pub const CAMERA_PRESET_SMALL: f32 = 230.;
pub const CAMERA_PRESET_LARGE: f32 = 400.;
pub const CAMERA_TOOLBAR_HEIGHT: f32 = 44.;
/// `CAMERA_WIDE_ASPECT_RATIO`.
const WIDE_ASPECT: f32 = 16. / 9.;

pub fn clamp_size(scale: f32) -> f32 {
    scale.clamp(CAMERA_MIN_SIZE, CAMERA_MAX_SIZE)
}

/// `cameraPreviewDimensions`: height is the size; width follows the aspect
/// ratio (1 unless the shape is `full`).
pub fn preview_dimensions(state: &CameraWindowState, _frame_aspect: Option<f32>) -> (f32, f32) {
    let base = clamp_size(state.size);
    (base * if state.shape == CameraShape::Full { WIDE_ASPECT } else { 1. }, base)
}

pub fn window_size(state: &CameraWindowState, frame_aspect: Option<f32>) -> (f32, f32) {
    let (width, height) = preview_dimensions(state, frame_aspect);
    (width, height + CAMERA_TOOLBAR_HEIGHT)
}

pub(crate) fn inline_preview_size(
    state: &CameraWindowState,
    frame_aspect: Option<f32>,
    viewport: (f32, f32),
) -> (f32, f32) {
    let (width, height) = preview_dimensions(state, frame_aspect);
    let scale = ((viewport.0 - 48.).max(160.) / width)
        .min((viewport.1 - 320.).max(160.) / height)
        .min(1.);
    (
        (width * scale).round(),
        (height * scale).round() + CAMERA_TOOLBAR_HEIGHT,
    )
}

#[cfg(test)]
mod camera_redesign_tests {
    use super::*;

    #[test]
    fn camera_window_reserves_only_the_fixed_bar_and_gap() {
        for size in [150., 230., 600.] {
            let state = CameraWindowState { size, ..Default::default() };
            assert_eq!(window_size(&state, None), (size, size + 44.));
        }
    }
}
#[cfg(test)]
mod inline_preview_tests {
    use super::*;

    #[test]
    fn camera_only_preview_preserves_size_until_viewport_requires_scaling() {
        let state = CameraWindowState::default();
        assert_eq!(
            inline_preview_size(&state, Some(16. / 9.), (1920., 1080.)),
            window_size(&state, Some(16. / 9.)),
        );
        assert_eq!(
            inline_preview_size(&state, None, (100., 100.)),
            (160., 204.),
        );
    }

    #[test]
    fn camera_only_wide_preview_preserves_aspect_and_toolbar_space() {
        let state = CameraWindowState {
            size: 600.,
            shape: CameraShape::Full,
            ..Default::default()
        };
        assert_eq!(
            inline_preview_size(&state, Some(16. / 9.), (800., 600.)),
            (498., 324.),
        );
        assert_eq!(state.size, 600.);
    }
}

#[cfg(test)]
mod toolbar_fit_tests {
    use super::*;

    #[test]
    fn camera_toolbar_keeps_fixed_controls_at_every_bubble_size() {
        for size in [150., 230., 600.] {
            let state = CameraWindowState { size, ..Default::default() };
            assert_eq!(toolbar_scale(&state), 1.);
        }
    }

    #[test]
    fn camera_toolbar_drops_blur_then_mirror_without_shrinking() {
        assert_eq!(toolbar_buttons(1., 230.), 5);
        assert_eq!(toolbar_buttons(1., 165.), 5);
        assert_eq!(toolbar_buttons(1., 150.), 4);
        assert_eq!(toolbar_buttons(1., 134.), 3);
        assert_eq!(toolbar_width(1., 5), 165.);
        assert_eq!(toolbar_width(1., 4), 135.);
        assert_eq!(toolbar_width(1., 3), 105.);
    }
}
fn toolbar_scale(_: &CameraWindowState) -> f32 {
    1.
}

fn toolbar_width(_: f32, buttons: usize) -> f32 {
    buttons as f32 * 28. + (buttons - 1) as f32 * 2. + 17.
}

fn toolbar_buttons(scale: f32, width: f32) -> usize {
    [5, 4].into_iter().find(|&buttons| toolbar_width(scale, buttons) <= width).unwrap_or(3)
}
pub(crate) fn preview_radius(state: &CameraWindowState) -> f32 {
    if cfg!(target_os = "macos") && state.background_blur == BlurMode::Remove {
        return 0.;
    }
    match state.shape {
        CameraShape::Round => clamp_size(state.size) / 2.,
        _ => 20.,
    }
}

fn picker_preview_radius(state: &CameraWindowState, picker_size: Option<(f32, f32)>) -> f32 {
    if cfg!(target_os = "macos") && state.background_blur == BlurMode::Remove {
        return 0.;
    }
    match (state.shape, picker_size) {
        (CameraShape::Round, Some((width, height))) => {
            width.max(0.).min((height - CAMERA_TOOLBAR_HEIGHT).max(0.)) / 2.
        }
        _ => preview_radius(state),
    }
}

#[cfg(test)]
mod preview_radius_tests {
    use super::*;

    #[test]
    fn parked_circle_tracks_visible_content_and_restores_saved_radius() {
        let state = CameraWindowState::default();
        let saved = state;
        assert_eq!(picker_preview_radius(&state, Some((136., 192.))), 68.);
        assert_eq!(picker_preview_radius(&state, Some((120., 192.))), 60.);
        assert_eq!(picker_preview_radius(&state, Some((136., 156.))), 56.);
        assert_eq!(picker_preview_radius(&state, None), saved.size / 2.);
        assert_eq!(state, saved);
    }

    #[test]
    fn parked_rectangular_shapes_preserve_their_corner_style() {
        for shape in [CameraShape::Square, CameraShape::Full] {
            let state = CameraWindowState {
                shape,
                ..Default::default()
            };
            assert_eq!(picker_preview_radius(&state, Some((136., 192.))), 20.);
            assert_eq!(picker_preview_radius(&state, None), 20.);
        }
    }
}

#[cfg(target_os = "macos")]
mod frame {
    use cidre::{arc, cf, cv, vt};

    type CreateRotationSession = unsafe extern "C-unwind" fn(
        Option<&cf::Allocator>,
        *mut Option<arc::R<vt::PixelRotationSession>>,
    ) -> cidre::os::Status;
    type RotateImage = unsafe extern "C-unwind" fn(
        &vt::PixelRotationSession,
        &cv::PixelBuf,
        &mut cv::PixelBuf,
    ) -> cidre::os::Status;

    struct FlipSession {
        session: arc::R<vt::PixelRotationSession>,
        rotate_image: RotateImage,
    }

    impl FlipSession {
        fn new() -> Option<Self> {
            // These APIs are macOS 13+. Direct cidre calls create strong imports that
            // make dyld terminate the entire app on macOS 12 before any OS guard runs.
            let create = unsafe {
                libc::dlsym(libc::RTLD_DEFAULT, c"VTPixelRotationSessionCreate".as_ptr())
            };
            let rotate = unsafe {
                libc::dlsym(
                    libc::RTLD_DEFAULT,
                    c"VTPixelRotationSessionRotateImage".as_ptr(),
                )
            };
            let key = unsafe {
                libc::dlsym(
                    libc::RTLD_DEFAULT,
                    c"kVTPixelRotationPropertyKey_FlipHorizontalOrientation".as_ptr(),
                )
            };
            if create.is_null() || rotate.is_null() || key.is_null() {
                return None;
            }
            let create =
                unsafe { std::mem::transmute::<*mut libc::c_void, CreateRotationSession>(create) };
            let rotate_image =
                unsafe { std::mem::transmute::<*mut libc::c_void, RotateImage>(rotate) };
            let key = unsafe { key.cast::<*const cf::String>().read().as_ref()? };
            let mut session = None;
            unsafe { create(None, &mut session).result().ok()? };
            let mut session = session?;
            session
                .set_prop(key, Some(cf::Boolean::value_true().as_ref()))
                .ok()?;
            Some(Self {
                session,
                rotate_image,
            })
        }

        fn rotate(
            &self,
            source: &cv::PixelBuf,
            destination: &mut cv::PixelBuf,
        ) -> cidre::os::Result {
            unsafe { (self.rotate_image)(&self.session, source, destination).result() }
        }
    }

    /// A converted preview frame: the BGRA IOSurface pixel buffer to paint or
    /// blur, its dimensions, and the ring generation (bumped on every ring
    /// rebuild so the blur worker's imported-texture cache can never alias a
    /// recycled allocation).
    pub struct Converted {
        pub buffer: arc::R<cv::PixelBuf>,
        pub dims: (usize, usize),
        pub generation: u64,
        pub mirrored: bool,
    }

    /// Converts camera frames (typically `420v`) into BGRA IOSurface-backed
    /// pixel buffers that gpui paints directly via `paint_surface_fitted`.
    ///
    /// VideoToolbox does the `420v` -> BGRA conversion in hardware either way;
    /// pointing it at an IOSurface destination removes the old per-frame CPU
    /// row-copy into a `RenderImage` and the sprite-atlas re-upload
    /// (`camera-preview-convert-benchmark`: 196us -> 137us per 720p frame on
    /// the conversion alone, byte-identical output, before atlas savings).
    /// The same transfer also downscales when a maximum size is given (blur
    /// caps its input at 720p, [`crate::camera_blur::BLUR_MAX_DIMS`]), so
    /// the blur path's scaling costs nothing extra.
    ///
    /// Mirroring is a second hardware pass: the conversion lands in a single
    /// scratch buffer and a `VTPixelRotationSession` with
    /// `FlipHorizontalOrientation` flips it into the ring. Both passes are
    /// synchronous, so the unflipped scratch is never painted and one buffer
    /// covers it. This is the preview-only mirror the Tauri bubble has --
    /// its shader flips the sampling UV (`camera.wgsl:97-100`), its legacy
    /// page flips the canvas with `scaleX(-1)` (`camera.tsx:869`), and
    /// neither touches the recorded camera track.
    ///
    /// The fixed rings keep a buffer alive while gpui's scene still references
    /// the previous one; four slots at camera rates leaves ~100ms before a
    /// slot is overwritten.
    pub struct FrameConverter {
        session: arc::R<vt::PixelTransferSession>,
        /// `None` when unmirrored, or when the rotation session could not be
        /// created (the preview then degrades to unmirrored, logged once).
        flip_session: Option<FlipSession>,
        ring: Vec<arc::R<cv::PixelBuf>>,
        scratch: Option<arc::R<cv::PixelBuf>>,
        next: usize,
        src_dims: (usize, usize),
        dst_dims: (usize, usize),
        mirrored: bool,
        generation: u64,
    }

    // Only touched from the main thread; the CF objects have atomic refcounts.
    unsafe impl Send for FrameConverter {}

    const RING_SIZE: usize = 4;

    fn bgra_buffers(
        count: usize,
        width: usize,
        height: usize,
    ) -> Option<Vec<arc::R<cv::PixelBuf>>> {
        let io_surface_properties = cf::Dictionary::new();
        let keys: [&cf::Type; 2] = [
            cv::pixel_buffer::keys::io_surf_props().as_ref(),
            cv::pixel_buffer::keys::metal_compatibility().as_ref(),
        ];
        let values: [&cf::Type; 2] = [
            io_surface_properties.as_ref(),
            cf::Boolean::value_true().as_ref(),
        ];
        let attrs = cf::Dictionary::with_keys_values(&keys, &values)?;
        (0..count)
            .map(|_| {
                cv::PixelBuf::new(width, height, cv::PixelFormat::_32_BGRA, Some(&attrs))
                    .ok()
                    .filter(|buf| buf.io_surf().is_some())
            })
            .collect::<Option<Vec<_>>>()
    }

    /// Scale-to-fit inside `max`, aspect preserved, never upscaling.
    fn fit_within(src: (usize, usize), max: (usize, usize)) -> (usize, usize) {
        let scale = (max.0 as f64 / src.0.max(1) as f64)
            .min(max.1 as f64 / src.1.max(1) as f64)
            .min(1.0);
        if scale >= 1.0 {
            return src;
        }
        (
            ((src.0 as f64 * scale).round() as usize).max(1),
            ((src.1 as f64 * scale).round() as usize).max(1),
        )
    }

    impl FrameConverter {
        fn new(
            src_dims: (usize, usize),
            dst_dims: (usize, usize),
            mirrored: bool,
            generation: u64,
        ) -> Option<Self> {
            let mut session = vt::PixelTransferSession::new().ok()?;
            session.set_realtime(true).ok()?;

            let flip_session = if mirrored {
                let flip = FlipSession::new();
                if flip.is_none() {
                    tracing::warn!(
                        "VTPixelRotationSession unavailable; camera preview mirroring disabled"
                    );
                }
                flip
            } else {
                None
            };

            let ring = bgra_buffers(RING_SIZE, dst_dims.0, dst_dims.1)?;
            let scratch = if flip_session.is_some() {
                Some(bgra_buffers(1, dst_dims.0, dst_dims.1)?.pop()?)
            } else {
                None
            };

            Some(Self {
                session,
                flip_session,
                ring,
                scratch,
                next: 0,
                src_dims,
                dst_dims,
                mirrored,
                generation,
            })
        }

        /// The frame as a BGRA IOSurface-backed pixel buffer, downscaled to
        /// fit `max_dims` when given and horizontally flipped when
        /// `mirrored`. Recreates the rings when the camera's frame size, the
        /// target size or the mirror toggle changes.
        pub fn convert(
            this: &mut Option<Self>,
            frame: &frameever_recording::NativeCameraFrame,
            max_dims: Option<(usize, usize)>,
            mirrored: bool,
        ) -> Option<Converted> {
            let src = frame.sample_buf.image_buf()?;
            let src_dims = (src.width(), src.height());
            let dst_dims = max_dims.map_or(src_dims, |max| fit_within(src_dims, max));
            if this.as_ref().is_none_or(|converter| {
                converter.src_dims != src_dims
                    || converter.dst_dims != dst_dims
                    || converter.mirrored != mirrored
            }) {
                let generation = this.as_ref().map_or(0, |c| c.generation + 1);
                *this = Self::new(src_dims, dst_dims, mirrored, generation);
            }
            let converter = this.as_mut()?;

            let mut out = converter.ring[converter.next].clone();
            let flipped = match (&converter.flip_session, &converter.scratch) {
                (Some(flip), Some(scratch)) => {
                    converter.session.transfer(src, scratch).ok()?;
                    flip.rotate(scratch, &mut out)
                        .inspect_err(|error| {
                            tracing::warn!(?error, "camera preview mirroring failed");
                        })
                        .is_ok()
                }
                _ => false,
            };
            if !flipped {
                converter.flip_session = None;
                converter.scratch = None;
                converter.session.transfer(src, &out).ok()?;
            }
            converter.next = (converter.next + 1) % RING_SIZE;
            Some(Converted {
                buffer: out,
                dims: converter.dst_dims,
                generation: converter.generation,
                mirrored: converter.flip_session.is_some(),
            })
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResizeCorner {
    NorthWest,
    NorthEast,
    SouthWest,
    SouthEast,
}

impl ResizeCorner {
    const ALL: [Self; 4] = [
        Self::NorthWest,
        Self::NorthEast,
        Self::SouthWest,
        Self::SouthEast,
    ];

    fn east(self) -> bool {
        matches!(self, Self::NorthEast | Self::SouthEast)
    }

    fn south(self) -> bool {
        matches!(self, Self::SouthWest | Self::SouthEast)
    }
}

struct ResizeDrag {
    corner: ResizeCorner,
    start_size: f32,
    start_picker_size: Option<(f32, f32)>,
    start_position: gpui::Point<gpui::Pixels>,
    start_origin: (f32, f32),
    start_dimensions: (f32, f32),
}

struct MoveDrag {
    pointer: gpui::Point<gpui::Pixels>,
    origin: (f32, f32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ResizeTarget {
    Picker((f32, f32)),
    State(f32),
}

fn resize_start(state_size: f32, picker_size: Option<(f32, f32)>) -> (f32, Option<(f32, f32)>) {
    let picker_size = picker_size.filter(|(width, height)| {
        width.is_finite() && *width > 0. && height.is_finite() && *height > CAMERA_TOOLBAR_HEIGHT
    });
    let start_size = picker_size
        .map(|(_, height)| height - CAMERA_TOOLBAR_HEIGHT)
        .unwrap_or(clamp_size(state_size));
    (start_size, picker_size)
}

fn resize_target(
    start_size: f32,
    start_picker_size: Option<(f32, f32)>,
    delta: f32,
) -> ResizeTarget {
    let minimum = start_picker_size.map_or(CAMERA_MIN_SIZE, |_| start_size.min(CAMERA_MIN_SIZE));
    let next = (start_size + delta).clamp(minimum, CAMERA_MAX_SIZE);
    if next < CAMERA_MIN_SIZE
        && let Some((start_width, _)) = start_picker_size
    {
        return ResizeTarget::Picker((
            start_width * next / start_size,
            next + CAMERA_TOOLBAR_HEIGHT,
        ));
    }
    ResizeTarget::State(next)
}

#[cfg(test)]
mod resize_tests {
    use super::*;

    fn close(left: f32, right: f32) {
        assert!((left - right).abs() < 0.001, "{left} != {right}");
    }

    #[test]
    fn parked_square_resizes_from_its_visible_size() {
        let (start, picker) = resize_start(230., Some((100., 144.)));
        assert_eq!(start, 100.);
        assert_eq!(
            resize_target(start, picker, 1.),
            ResizeTarget::Picker((101., 145.))
        );
    }

    #[test]
    fn parked_landscape_preserves_aspect_below_the_saved_size_floor() {
        let (start, picker) = resize_start(180., Some((144., 125.)));
        let ResizeTarget::Picker((width, height)) = resize_target(start, picker, 1.) else {
            panic!("expected temporary picker geometry");
        };
        close(start, 81.);
        close(height, 126.);
        close(width / (height - CAMERA_TOOLBAR_HEIGHT), 16. / 9.);
    }

    #[test]
    fn parked_resize_hands_off_continuously_at_the_saved_size_floor() {
        let (start, picker) = resize_start(180., Some((144., 125.)));
        assert_eq!(
            resize_target(start, picker, CAMERA_MIN_SIZE - start),
            ResizeTarget::State(CAMERA_MIN_SIZE)
        );
        let expected_width = 144. * CAMERA_MIN_SIZE / start;
        let state = CameraWindowState {
            size: CAMERA_MIN_SIZE,
            shape: CameraShape::Full,
            ..Default::default()
        };
        let (width, height) = window_size(&state, Some(16. / 9.));
        close(width, expected_width);
        close(height, CAMERA_MIN_SIZE + CAMERA_TOOLBAR_HEIGHT);
    }

    #[test]
    fn parked_resize_does_not_shrink_below_its_visible_size() {
        let (start, picker) = resize_start(230., Some((100., 144.)));
        assert_eq!(
            resize_target(start, picker, -50.),
            ResizeTarget::Picker((100., 144.))
        );
    }
}

/// `cameraOverlayTextMetrics` (`camera.tsx:891-909`), rem resolved at 16px.
struct OverlayMetrics {
    gap: f32,
    max_width: f32,
    line_height: f32,
    message_size: f32,
    title_size: f32,
}

fn overlay_metrics(size: f32) -> OverlayMetrics {
    let normalized = (clamp_size(size) - CAMERA_MIN_SIZE) / (CAMERA_MAX_SIZE - CAMERA_MIN_SIZE);
    OverlayMetrics {
        gap: 4.,
        max_width: (size * 0.8).clamp(118., 320.),
        line_height: 16. + normalized * 2.,
        message_size: 11. + normalized,
        title_size: 12. + normalized * 2.,
    }
}

/// `camera_preview_error_message` (`lib.rs:755-774`) plus the fixed title
/// from `emit_camera_preview_error` (`lib.rs:776-784`). The error strings
/// come from the same `frameever-recording` camera errors the Tauri app matches on.
/// The camera is waiting on the macOS prompt: asked through the pickers, or
/// a start in flight while macOS has not decided yet.
fn awaiting_camera_access(cx: &gpui::App) -> bool {
    use crate::permissions::{OSPermission, OSPermissionStatus};
    let access = crate::media_access::MediaAccess::global(cx);
    let access = access.read(cx);
    access.requesting(OSPermission::Camera)
        || (access.status(OSPermission::Camera) == OSPermissionStatus::NotDetermined
            && crate::feeds::Feeds::global(cx)
                .read(cx)
                .camera_input_pending())
}

fn camera_issue(error: &str) -> (&'static str, &'static str) {
    if error.contains("access is unavailable") {
        return (
            t!("Camera access needed"),
            if cfg!(target_os = "windows") {
                t!("Allow it in Windows Settings.")
            } else {
                t!("Choose Allow in the macOS prompt.")
            },
        );
    }
    let message = if error.contains("DeviceNotFound") {
        t!("Reconnect your camera.")
    } else if error.contains("CameraTimeout") || error.contains("StartCapturing") {
        t!("It may be in use by another app.")
    } else if error.contains("InvalidFormat") {
        t!("No usable camera format.")
    } else {
        t!("Choose another camera.")
    };
    (t!("Couldn't start camera"), message)
}

fn preview_issue(
    disconnected: bool,
    camera_error: Option<&str>,
    effect_issue: Option<(&'static str, &'static str)>,
    awaiting_access: bool,
) -> Option<(&'static str, &'static str)> {
    if disconnected {
        return Some((
            t!("Camera disconnected"),
            t!("Recording continues without it."),
        ));
    }
    camera_error
        .map(camera_issue)
        .or(effect_issue)
        .or(awaiting_access.then_some((
            t!("Waiting for camera access"),
            t!("Choose Allow in the macOS prompt."),
        )))
}

#[cfg(test)]
mod preview_issue_tests {
    use super::*;

    #[test]
    fn a_camera_lost_during_a_recording_says_the_recording_continues() {
        let lost = (
            t!("Camera disconnected"),
            t!("Recording continues without it."),
        );
        assert_eq!(preview_issue(true, None, None, false), Some(lost));
        assert_eq!(
            preview_issue(true, Some("CameraTimeout"), None, true),
            Some(lost)
        );
    }

    #[test]
    fn without_a_disconnect_the_other_messages_keep_their_order() {
        assert_eq!(preview_issue(false, None, None, false), None);
        assert_eq!(
            preview_issue(false, Some("CameraTimeout"), None, true),
            Some(camera_issue("CameraTimeout"))
        );
        assert_eq!(
            preview_issue(false, None, None, true).map(|(title, _)| title),
            Some(t!("Waiting for camera access"))
        );
    }
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct PreviewEffectFailures {
    mirror: bool,
    blur: bool,
}

#[cfg(target_os = "macos")]
impl PreviewEffectFailures {
    fn issue(self) -> Option<(&'static str, &'static str)> {
        match (self.mirror, self.blur) {
            (false, false) => None,
            (true, false) => Some((
                t!("Mirror unavailable"),
                t!("Your camera preview is not mirrored."),
            )),
            (false, true) => Some((
                t!("Background blur unavailable"),
                t!("Your camera is unblurred."),
            )),
            (true, true) => Some((
                t!("Camera effects unavailable"),
                t!("Your camera is unblurred and not mirrored."),
            )),
        }
    }

    fn reset_changed(&mut self, before: CameraWindowState, after: CameraWindowState) {
        if before.mirrored != after.mirrored {
            self.mirror = false;
        }
        if before.background_blur != after.background_blur {
            self.blur = false;
        }
    }
}

#[cfg(all(test, target_os = "macos"))]
mod preview_effect_failure_tests {
    use super::*;

    #[test]
    fn each_failed_effect_has_explicit_feedback() {
        assert!(PreviewEffectFailures::default().issue().is_none());
        assert!(
            PreviewEffectFailures {
                mirror: true,
                blur: false
            }
            .issue()
            .unwrap()
            .1
            .contains("not mirrored")
        );
        assert!(
            PreviewEffectFailures {
                mirror: false,
                blur: true
            }
            .issue()
            .unwrap()
            .1
            .contains("unblurred")
        );
        assert!(
            PreviewEffectFailures {
                mirror: true,
                blur: true
            }
            .issue()
            .unwrap()
            .1
            .contains("unblurred and not mirrored")
        );
    }

    #[test]
    fn unrelated_changes_preserve_failure_and_requested_recording_blur() {
        let before = CameraWindowState {
            mirrored: true,
            background_blur: BlurMode::Heavy,
            ..Default::default()
        };
        let after = CameraWindowState {
            size: 400.,
            ..before
        };
        let mut failures = PreviewEffectFailures {
            mirror: true,
            blur: true,
        };
        failures.reset_changed(before, after);
        assert!(failures.mirror && failures.blur);
        assert_eq!(after.background_blur, BlurMode::Heavy);
    }

    #[test]
    fn toggle_retry_resets_only_the_changed_effect() {
        let before = CameraWindowState {
            mirrored: true,
            background_blur: BlurMode::Heavy,
            ..Default::default()
        };
        let mut failures = PreviewEffectFailures {
            mirror: true,
            blur: true,
        };
        failures.reset_changed(
            before,
            CameraWindowState {
                mirrored: false,
                ..before
            },
        );
        assert!(!failures.mirror && failures.blur);
        failures.reset_changed(
            before,
            CameraWindowState {
                background_blur: BlurMode::Off,
                ..before
            },
        );
        assert!(!failures.mirror && !failures.blur);
        assert!(failures.issue().is_none());
    }
}

#[derive(Default)]
struct ParkedCameraPreview(Option<RetainedCameraPreview>);

impl gpui::Global for ParkedCameraPreview {}

struct RetainedCameraPreview {
    image: Arc<gpui::RenderImage>,
    camera: crate::feeds::SelectedCamera,
    state: CameraWindowState,
    captured_at: Instant,
    frame_dims: (usize, usize),
}

pub(crate) fn retained_preview_aspect(cx: &gpui::App) -> Option<f32> {
    let retained = cx.try_global::<ParkedCameraPreview>()?.0.as_ref()?;
    let feeds = Feeds::global(cx);
    let feeds = feeds.read(cx);
    if feeds.camera.as_ref() != Some(&retained.camera)
        || feeds.camera_error.is_some()
        || store::load().camera_window.unwrap_or_default() != retained.state
        || retained.captured_at.elapsed() >= Duration::from_secs(60)
    {
        return None;
    }
    Some(retained.frame_dims.0 as f32 / retained.frame_dims.1.max(1) as f32)
}

pub(crate) fn clear_parked_camera_preview(cx: &mut gpui::App) {
    cx.default_global::<ParkedCameraPreview>().0 = None;
}

#[cfg(target_os = "macos")]
pub(crate) fn snapshot_preview(
    buffer: &core_video::pixel_buffer::CVPixelBuffer,
) -> Option<Arc<gpui::RenderImage>> {
    snapshot_bgra(buffer, (960., 540.))
}

/// The background-removal cutout, at the full 720p it was composited at: the
/// alpha edge is the whole point, so it is never nearest-neighbour shrunk.
#[cfg(target_os = "macos")]
pub(crate) fn snapshot_cutout(
    buffer: &core_video::pixel_buffer::CVPixelBuffer,
) -> Option<Arc<gpui::RenderImage>> {
    snapshot_bgra(buffer, (1280., 720.))
}

#[cfg(target_os = "macos")]
fn snapshot_bgra(
    buffer: &core_video::pixel_buffer::CVPixelBuffer,
    (max_width, max_height): (f64, f64),
) -> Option<Arc<gpui::RenderImage>> {
    use core_video::pixel_buffer::{kCVPixelBufferLock_ReadOnly, kCVPixelFormatType_32BGRA};

    let width = buffer.get_width();
    let height = buffer.get_height();
    let stride = buffer.get_bytes_per_row();
    let row_bytes = width.checked_mul(4)?;
    let required_bytes = height
        .checked_sub(1)?
        .checked_mul(stride)?
        .checked_add(row_bytes)?;
    if buffer.get_pixel_format() != kCVPixelFormatType_32BGRA
        || width == 0
        || height == 0
        || stride < row_bytes
        || required_bytes > buffer.get_data_size()
        || buffer.lock_base_address(kCVPixelBufferLock_ReadOnly) != 0
    {
        return None;
    }
    let image = (|| {
        let base = unsafe { buffer.get_base_address() }.cast::<u8>();
        if base.is_null() {
            return None;
        }
        let scale = (max_width / width as f64)
            .min(max_height / height as f64)
            .min(1.);
        let target_width = (width as f64 * scale).round().max(1.) as u32;
        let target_height = (height as f64 * scale).round().max(1.) as u32;
        let data = unsafe { std::slice::from_raw_parts(base, required_bytes) };
        // RenderImage consumes BGRA bytes even though image::Frame wraps RgbaImage.
        let pixels = if (target_width as usize, target_height as usize) == (width, height) {
            if stride == row_bytes {
                data[..row_bytes * height].to_vec()
            } else {
                let mut pixels = Vec::with_capacity(row_bytes * height);
                for row in data.chunks(stride).take(height) {
                    pixels.extend_from_slice(&row[..row_bytes]);
                }
                pixels
            }
        } else {
            frameever_gpui_kernels::frame::downsample_nearest(
                data,
                width,
                height,
                stride,
                target_width,
                target_height,
            )
        };
        let image = image::RgbaImage::from_raw(target_width, target_height, pixels)?;
        Some(Arc::new(gpui::RenderImage::new(smallvec::smallvec![
            image::Frame::new(image)
        ])))
    })();
    if buffer.unlock_base_address(kCVPixelBufferLock_ReadOnly) != 0 {
        return None;
    }
    image
}

/// The per-frame half of the window: owns the latest converted (or blurred)
/// frame and is the only entity notified at camera rate. Chrome invalidation
/// goes through the parent [`CameraWindow`] instead, so a frame draw reuses
/// the cached toolbar subtree.
struct CameraPreviewView {
    #[cfg(target_os = "macos")]
    cutout_frame: Option<Arc<gpui::RenderImage>>,
    #[cfg(target_os = "macos")]
    painted_cutout_frame: Option<Arc<gpui::RenderImage>>,
    theme: Theme,
    radius: f32,
    /// Clamped bubble size, for the issue overlay's scaled text metrics.
    size: f32,
    #[cfg(target_os = "macos")]
    latest_frame: Option<core_video::pixel_buffer::CVPixelBuffer>,
    #[cfg(not(target_os = "macos"))]
    latest_frame: Option<Arc<gpui::RenderImage>>,
    frame_dims: Option<(usize, usize)>,
    retained: Option<Arc<gpui::RenderImage>>,
    retained_dims: Option<(usize, usize)>,
    retained_captured_at: Option<Instant>,
    reveal_started: Option<Instant>,
    retained_invalidated: bool,
    selection_revision: u64,
    frame_revision: Option<u64>,
    camera: Option<crate::feeds::SelectedCamera>,
    /// Bumped by the canvas paint callback; the parent's cadence log reads it
    /// to prove notify-driven repaints actually present.
    paints: Arc<AtomicU32>,
    /// `Feeds::camera_error`, copied on change. The observe must not notify
    /// unconditionally: `Feeds` also carries the mic meter, which notifies at
    /// ~20Hz whenever a microphone is selected, and each of those would
    /// repaint the whole preview for a message that did not change.
    camera_error: Option<String>,
    camera_disconnected: bool,
    effect_issue: Option<(&'static str, &'static str)>,
    awaiting_access: bool,
    _feeds_subscription: Subscription,
    _access_subscription: Subscription,
}

impl CameraPreviewView {
    fn new(
        theme: Theme,
        radius: f32,
        size: f32,
        paints: Arc<AtomicU32>,
        cx: &mut Context<Self>,
    ) -> Self {
        let feeds = Feeds::global(cx);
        let camera_error = feeds.read(cx).camera_error.clone();
        let camera_disconnected = feeds.read(cx).camera_disconnected();
        let camera = feeds.read(cx).camera.clone();
        let state = store::load().camera_window.unwrap_or_default();
        let retained = cx
            .default_global::<ParkedCameraPreview>()
            .0
            .take()
            .filter(|retained| {
                camera.as_ref() == Some(&retained.camera)
                    && state == retained.state
                    && camera_error.is_none()
                    && retained.captured_at.elapsed() < Duration::from_secs(60)
            });
        let retained_dims = retained.as_ref().map(|retained| retained.frame_dims);
        let retained_captured_at = retained.as_ref().map(|retained| retained.captured_at);
        if let Some(retained) = &retained {
            let expiry = cx
                .background_executor()
                .timer(Duration::from_secs(60).saturating_sub(retained.captured_at.elapsed()));
            cx.spawn(async move |this, cx| {
                expiry.await;
                this.update(cx, |this: &mut Self, cx| {
                    if this.retained.is_some() {
                        this.retained_invalidated = true;
                        cx.notify();
                    }
                })
                .ok();
            })
            .detach();
        }
        let retained = retained.map(|retained| retained.image);
        let access = crate::media_access::MediaAccess::global(cx);
        let awaiting_access = awaiting_camera_access(cx);
        let access_subscription = cx.observe(&access, |this: &mut Self, _, cx| {
            let awaiting = awaiting_camera_access(cx);
            if this.awaiting_access != awaiting {
                this.awaiting_access = awaiting;
                cx.notify();
            }
        });
        let feeds_subscription = cx.observe(&feeds, |this: &mut Self, feeds, cx| {
            let awaiting = awaiting_camera_access(cx);
            if this.awaiting_access != awaiting {
                this.awaiting_access = awaiting;
                cx.notify();
            }
            let feeds = feeds.read(cx);
            let error = feeds.camera_error.clone();
            let camera = feeds.camera.clone();
            if this.camera_disconnected != feeds.camera_disconnected() {
                this.camera_disconnected = feeds.camera_disconnected();
                cx.notify();
            }
            if this.camera_error != error || this.camera != camera {
                if error.is_some() || this.camera != camera {
                    this.retained_invalidated = true;
                    this.selection_revision = this.selection_revision.wrapping_add(1);
                }
                this.camera_error = error;
                this.camera = camera;
                cx.notify();
            }
        });
        Self {
            #[cfg(target_os = "macos")]
            cutout_frame: None,
            #[cfg(target_os = "macos")]
            painted_cutout_frame: None,
            theme,
            radius,
            size,
            #[cfg(target_os = "macos")]
            latest_frame: None,
            #[cfg(not(target_os = "macos"))]
            latest_frame: None,
            frame_dims: None,
            retained,
            retained_dims,
            retained_captured_at,
            reveal_started: None,
            retained_invalidated: false,
            selection_revision: 0,
            frame_revision: None,
            camera,
            paints,
            camera_error,
            camera_disconnected,
            effect_issue: None,
            awaiting_access,
            _feeds_subscription: feeds_subscription,
            _access_subscription: access_subscription,
        }
    }

    #[cfg(target_os = "macos")]
    fn set_frame(
        &mut self,
        frame: core_video::pixel_buffer::CVPixelBuffer,
        dims: (usize, usize),
        cx: &mut Context<Self>,
    ) {
        self.cutout_frame = None;
        self.latest_frame = Some(frame);
        self.frame_dims = Some(dims);
        self.frame_revision = Some(self.selection_revision);
        if self.retained.is_some() && self.reveal_started.is_none() {
            self.reveal_started = Some(Instant::now());
        }
        cx.notify();
    }

    #[cfg(not(target_os = "macos"))]
    fn set_frame(
        &mut self,
        frame: Arc<gpui::RenderImage>,
        dims: (usize, usize),
        cx: &mut Context<Self>,
    ) -> Option<Arc<gpui::RenderImage>> {
        let previous = self.latest_frame.replace(frame);
        self.frame_dims = Some(dims);
        self.frame_revision = Some(self.selection_revision);
        if self.retained.is_some() && self.reveal_started.is_none() {
            self.reveal_started = Some(Instant::now());
        }
        cx.notify();
        previous
    }

    fn set_chrome(&mut self, radius: f32, size: f32, cx: &mut Context<Self>) {
        if self.radius != radius || self.size != size {
            self.radius = radius;
            self.size = size;
            cx.notify();
        }
    }
}

impl Render for CameraPreviewView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        #[cfg(target_os = "macos")]
        if self.painted_cutout_frame.as_ref().is_some_and(|painted| {
            self.cutout_frame
                .as_ref()
                .is_none_or(|current| !Arc::ptr_eq(painted, current))
        }) && let Some(image) = self.painted_cutout_frame.take()
        {
            let _ = window.drop_image(image);
        }
        if self
            .frame_revision
            .is_some_and(|revision| revision != self.selection_revision)
        {
            #[cfg(target_os = "macos")]
            {
                if let Some(image) = self.cutout_frame.take() {
                    let _ = window.drop_image(image);
                }
                self.latest_frame = None;
            }
            #[cfg(not(target_os = "macos"))]
            if let Some(image) = self.latest_frame.take() {
                let _ = window.drop_image(image);
            }
            self.frame_dims = None;
            self.frame_revision = None;
        }
        if (self.retained_invalidated
            || self
                .reveal_started
                .is_some_and(|started| started.elapsed() >= crate::theme::motion::MID))
            && let Some(image) = self.retained.take()
        {
            let _ = window.drop_image(image);
        }
        self.retained_invalidated = false;
        let theme = self.theme;
        let radius = self.radius;

        let showing_frame = self.latest_frame.is_some() || self.retained.is_some();
        let awaiting_access = self.awaiting_access && !showing_frame && self.camera_error.is_none();

        let mut container = div()
            .relative()
            .size_full()
            .overflow_hidden()
            .rounded(px(radius))
            .when(radius > 0., |this| this.bg(theme.preview))
            .text_color(gpui::white());

        #[cfg(target_os = "macos")]
        if let Some(buffer) = self
            .latest_frame
            .clone()
            .filter(|_| self.cutout_frame.is_none())
        {
            let frame_dims = self.frame_dims;
            let paints = self.paints.clone();
            // Cover-fit painted straight from the IOSurface: `ObjectFit::Cover`
            // computes the same fitted box the old `img()` element used, and
            // `paint_surface_fitted` UV-crops it to the element bounds so the
            // corner radii round the visible rect (circle shape included).
            container = container.child(
                gpui::canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        let Some((width, height)) = frame_dims else {
                            return;
                        };
                        let frame_size = gpui::size(
                            gpui::DevicePixels(width as i32),
                            gpui::DevicePixels(height as i32),
                        );
                        let fitted = gpui::ObjectFit::Cover.get_bounds(bounds, frame_size);
                        window.paint_surface_fitted(
                            bounds,
                            fitted,
                            gpui::Corners::all(px(radius)).clamp_radii_for_quad_size(bounds.size),
                            buffer.clone(),
                        );
                        paints.fetch_add(1, Ordering::Relaxed);
                    },
                )
                .size_full(),
            );
        }

        #[cfg(target_os = "macos")]
        if let Some(image) = self.cutout_frame.clone() {
            self.painted_cutout_frame = Some(image.clone());
            self.paints.fetch_add(1, Ordering::Relaxed);
            // The cutout window is transparent, so the person fades in while
            // the loading disc fades out; a raw surface cannot take opacity,
            // but the bubble's opaque glass covers it during its own fade.
            let reveal = 1.;
            container = container.child(
                gpui::img(image)
                    .size_full()
                    .object_fit(gpui::ObjectFit::Cover)
                    .opacity(reveal),
            );
        }

        #[cfg(not(target_os = "macos"))]
        if let Some(image) = self.latest_frame.clone() {
            self.paints.fetch_add(1, Ordering::Relaxed);
            container = container.child(
                gpui::img(image)
                    .size_full()
                    .rounded(px(radius))
                    .object_fit(gpui::ObjectFit::Cover),
            );
        }

        if let Some(image) = self.retained.clone() {
            let overlay = div().absolute().inset_0().child(
                gpui::img(image)
                    .size_full()
                    .rounded(px(radius))
                    .object_fit(gpui::ObjectFit::Cover),
            );
            container = if self.reveal_started.is_some() {
                container.child(
                    overlay.with_animation(
                        "camera-preview-reveal",
                        Animation::new(crate::theme::motion::MID)
                            .with_easing(crate::theme::motion::EASE_EFFECTS.curve()),
                        |element, progress| element.opacity(1. - progress),
                    ),
                )
            } else {
                container.child(overlay)
            };
        }

        if !showing_frame {
            container = container.child(camera_loading::camera_loading(&theme, radius));
        }
        // `CameraIssueOverlay` (`camera.tsx:911-955`): a black/75 wash over
        // the preview with a centred, size-scaled title + message. The
        // `backdrop-blur-xs` behind it has no per-element hook in this gpui
        // rev (the recording overlay documents the same gap).
        let access_issue = self
            .camera_error
            .as_deref()
            .is_some_and(|error| error.contains("access is unavailable"));
        if let Some((title, message)) = preview_issue(
            self.camera_disconnected,
            self.camera_error.as_deref(),
            self.effect_issue,
            awaiting_access,
        ) {
            let metrics = overlay_metrics(self.size);
            container = container.child(
                div()
                    .id("camera-issue")
                    .when(access_issue, |overlay| {
                        overlay.cursor_pointer().on_click(|_, _, cx| {
                            let access = crate::media_access::MediaAccess::global(cx);
                            access.update(cx, |access, cx| {
                                access.request(crate::permissions::OSPermission::Camera, cx)
                            });
                        })
                    })
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .overflow_hidden()
                    .rounded(px(radius))
                    .bg(theme.preview)
                    .px(px(16.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .text_center()
                            .gap(px(metrics.gap))
                            .max_w(px(metrics.max_width))
                            .child(
                                div()
                                    .text_size(px(metrics.title_size))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(gpui::white())
                                    .child(title),
                            )
                            .child(
                                div()
                                    .text_size(px(metrics.message_size))
                                    .line_height(px(metrics.line_height))
                                    .text_color(gpui::hsla(0., 0., 1., 0.75))
                                    .child(message),
                            ),
                    ),
            );
        }

        container
    }
}

#[cfg(not(target_os = "macos"))]
pub struct CameraPreviewFrame {
    pub timestamp: frameever_timestamp::Timestamp,
    pub image: Arc<gpui::RenderImage>,
    pub dims: (usize, usize),
}

/// The chrome half: renders the parent's toolbar, re-rendered only when the
/// parent notifies (same shape as the editor's `EditorSectionView`).
struct CameraToolbarView {
    camera: WeakEntity<CameraWindow>,
    _camera_subscription: Subscription,
}

impl CameraToolbarView {
    fn new(camera: &Entity<CameraWindow>, cx: &mut Context<Self>) -> Self {
        let camera_subscription = cx.observe(camera, |_, _, cx| cx.notify());
        Self {
            camera: camera.downgrade(),
            _camera_subscription: camera_subscription,
        }
    }
}

impl Render for CameraToolbarView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(camera) = self.camera.upgrade() else {
            return div().into_any_element();
        };
        camera.update(cx, |camera, cx| {
            camera.render_toolbar(cx).into_any_element()
        })
    }
}

/// Channels to and from the `camera-blur` worker thread, plus the foreground
/// pump that paints its outputs. Dropping this closes the job channel, which
/// ends the thread and releases the ONNX/GPU resources -- the
/// `release_blur_resources` behaviour (`camera.rs:1477-1484`).
#[cfg(target_os = "macos")]
struct BlurBridge {
    epoch: u64,
    tx: flume::Sender<camera_blur::BlurJob>,
    /// The first blurred output may land while the window is inactive, where
    /// a notify alone may not present (the unit-2 first-frame finding); the
    /// pump answers it with one explicit `refresh`.
    first_output_pending: bool,
    _pump: gpui::Task<()>,
}

pub struct CameraWindow {
    theme: Theme,
    state: CameraWindowState,
    inline: bool,
    picker_size: Option<(f32, f32)>,
    size_generation: Arc<AtomicU64>,
    chrome_visible: bool,
    focus: gpui::FocusHandle,
    toolbar_below: bool,
    moving: Option<MoveDrag>,
    resizing: Option<ResizeDrag>,
    hovered_handle: Option<ResizeCorner>,
    #[cfg(target_os = "macos")]
    converter: Option<frame::FrameConverter>,
    #[cfg(target_os = "macos")]
    blur: Option<BlurBridge>,
    #[cfg(target_os = "macos")]
    effect_failures: PreviewEffectFailures,
    preview: Entity<CameraPreviewView>,
    toolbar: Entity<CameraToolbarView>,
    frame_dims: Option<(usize, usize)>,
    retained_aspect: Option<f32>,
    // Cadence instrumentation: proves the preview stays live (delivered) and
    // actually presents (painted) while the window is inactive.
    frames_in_window: u32,
    cadence_window_start: Instant,
    paints: Arc<AtomicU32>,
    paints_at_window_start: u32,
    /// Debounce for the position save, the `queueCameraPositionSave` 200ms
    /// (`camera.tsx:93-112`): only the newest generation writes.
    position_save_generation: Arc<AtomicU64>,
    last_saved_position: Option<(f32, f32)>,
}

fn camera_size_percent(short_side: f64, reference: (f64, f64)) -> Option<f32> {
    let (width, height) = reference;
    if ![short_side, width, height].into_iter().all(|value| value.is_finite() && value > 0.) { return None; }
    Some((short_side / width.min(height) * 100.) as f32)
}

fn camera_capture_size(target: &frameever_recording::screen_capture::ScreenCaptureTarget) -> Option<(f64, f64)> {
    use frameever_recording::screen_capture::ScreenCaptureTarget;
    match target {
        ScreenCaptureTarget::CameraOnly => None,
        ScreenCaptureTarget::Area { bounds, .. } => {
            #[cfg(target_os = "windows")]
            let scale = {
                let display = target.display()?;
                let physical = display.raw_handle().physical_bounds()?;
                let logical = display.logical_size()?;
                (physical.size().width() / logical.width(), physical.size().height() / logical.height())
            };
            #[cfg(target_os = "macos")]
            let scale = (1., 1.);
            Some((bounds.size().width() * scale.0, bounds.size().height() * scale.1))
        }
        ScreenCaptureTarget::Display { .. } => {
            let display = target.display()?;
            #[cfg(target_os = "windows")]
            let bounds = display.raw_handle().physical_bounds()?;
            #[cfg(target_os = "macos")]
            let bounds = display.raw_handle().logical_bounds()?;
            Some((bounds.size().width(), bounds.size().height()))
        }
        ScreenCaptureTarget::Window { id } => {
            let window = scap_targets::Window::from_id(id)?;
            #[cfg(target_os = "windows")]
            let bounds = window.raw_handle().physical_bounds()?;
            #[cfg(target_os = "macos")]
            let bounds = window.raw_handle().logical_bounds()?;
            Some((bounds.size().width(), bounds.size().height()))
        }
    }
}

#[cfg(test)]
mod snapshot_size_tests {
    use super::*;

    #[test]
    fn camera_size_inherits_the_visible_short_side_without_a_minimum_padding() {
        for (short, reference, expected) in [
            (230., (1920., 1080.), 21.296296),
            (460., (3840., 2160.), 21.296296),
            (230., (1080., 1920.), 21.296296),
            (150., (720., 1280.), 20.833334),
        ] {
            assert!((camera_size_percent(short, reference).unwrap() - expected).abs() < 0.001);
        }
        assert_eq!(camera_size_percent(230., (0., 1080.)), None);
        assert_eq!(camera_size_percent(f64::NAN, (1920., 1080.)), None);
    }

    #[test]
    fn camera_wide_preview_keeps_the_designed_ratio_for_portrait_and_four_three_inputs() {
        let state = CameraWindowState { shape: CameraShape::Full, size: 230., ..Default::default() };
        for aspect in [Some(0.75), Some(4. / 3.), Some(16. / 9.), None] {
            let dimensions = preview_dimensions(&state, aspect);
            assert!((dimensions.0 / dimensions.1 - 16. / 9.).abs() < 0.001);
        }
    }
}

impl CameraWindow {
    pub(crate) fn studio_snapshot(
        &self,
        window: &Window,
        target: &frameever_recording::screen_capture::ScreenCaptureTarget,
    ) -> crate::recording::StudioCameraSnapshot {
        let bounds = if self.inline {
            None
        } else {
            #[cfg(target_os = "macos")]
            let bounds = {
                let bounds = window.bounds();
                Some([
                    f64::from(f32::from(bounds.origin.x)),
                    f64::from(f32::from(bounds.origin.y) + self.preview_offset()),
                    f64::from(f32::from(bounds.size.width)),
                    f64::from(f32::from(bounds.size.height) - CAMERA_TOOLBAR_HEIGHT),
                ])
            };
            #[cfg(target_os = "windows")]
            let bounds = platform::native_window(window).map(|native| {
                let (x, y, width, height) = platform::window_frame(&native);
                let scale = f64::from(window.scale_factor());
                [x, y + f64::from(self.preview_offset()) * scale, width, height - f64::from(CAMERA_TOOLBAR_HEIGHT) * scale]
            });
            bounds
        };
        let placement = bounds.and_then(|bounds| frameever_recording::camera_placement::recording_camera_placement(target, bounds));
        let size_percent = bounds.and_then(|bounds| camera_capture_size(target).and_then(|reference| camera_size_percent(bounds[2].min(bounds[3]), reference)));
        crate::recording::StudioCameraSnapshot {
            blur: self.state.background_blur,
            placement,
            preview: if self.inline { self.state } else { CameraWindowState { size: f32::from(window.bounds().size.width).min(f32::from(window.bounds().size.height) - CAMERA_TOOLBAR_HEIGHT), ..self.state } },
            size_percent,
        }
    }

    pub(crate) fn retain_preview(&self, cx: &mut Context<Self>) {
        let feeds = Feeds::global(cx);
        let Some(camera) = feeds.read(cx).camera.clone() else {
            return;
        };
        if feeds.read(cx).camera_error.is_some() {
            return;
        }
        let preview = self.preview.read(cx);
        if preview.retained_invalidated || preview.camera.as_ref() != Some(&camera) {
            return;
        }
        #[cfg(target_os = "macos")]
        let image = (preview.frame_revision == Some(preview.selection_revision))
            .then(|| preview.latest_frame.as_ref().and_then(snapshot_preview))
            .flatten();
        #[cfg(not(target_os = "macos"))]
        let image = (preview.frame_revision == Some(preview.selection_revision))
            .then(|| preview.latest_frame.clone())
            .flatten();
        let Some((image, captured_at)) = image
            .map(|image| (image, Instant::now()))
            .or_else(|| preview.retained.clone().zip(preview.retained_captured_at))
        else {
            return;
        };
        let remaining = Duration::from_secs(60).saturating_sub(captured_at.elapsed());
        if remaining.is_zero() {
            return;
        }
        let Some(frame_dims) = preview.frame_dims.or(preview.retained_dims) else {
            return;
        };
        cx.default_global::<ParkedCameraPreview>().0 = Some(RetainedCameraPreview {
            image,
            camera,
            state: self.state,
            captured_at,
            frame_dims,
        });
        let expiry = cx.background_executor().timer(remaining);
        cx.spawn(async move |_, cx| {
            expiry.await;
            cx.update(|cx| {
                if cx
                    .try_global::<ParkedCameraPreview>()
                    .and_then(|cache| cache.0.as_ref())
                    .is_some_and(|cached| cached.captured_at == captured_at)
                {
                    clear_parked_camera_preview(cx);
                }
            });
        })
        .detach();
    }

    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        crate::theme::bind_window(window, cx);
        #[cfg(target_os = "macos")]
        platform::apply_window_theme_deferred(
            window,
            crate::theme::forced_appearance(crate::theme::current_preference(cx)),
            cx.foreground_executor(),
        );
        let theme = Theme::for_window(window, cx, false);
        let state = store::load().camera_window.unwrap_or_default();
        #[cfg(not(target_os = "macos"))]
        Feeds::global(cx).update(cx, |feeds, _| {
            feeds.set_camera_preview_state(state.mirrored, state.background_blur)
        });
        let retained_aspect = retained_preview_aspect(cx);
        let paints = Arc::new(AtomicU32::new(0));
        let preview = cx.new({
            let paints = paints.clone();
            let radius = preview_radius(&state);
            let size = clamp_size(state.size);
            move |cx| CameraPreviewView::new(theme, radius, size, paints, cx)
        });
        let camera = cx.entity();
        let toolbar = cx.new(|cx| CameraToolbarView::new(&camera, cx));

        // `currentWindow.onMoved` + the 400ms position sync
        // (`camera.tsx:167-216`): every move lands in the shared store,
        // debounced, so the next open -- from either app -- restores it.
        cx.observe_window_bounds(window, |this, window, cx| {
            this.queue_position_save(window, cx);
        })
        .detach();

        let session = crate::session::RecordingSession::global(cx);
        cx.observe_in(&session, window, |this, _, window, cx| {
            if !this.chrome_allowed(cx) {
                this.chrome_visible = false;
                this.resizing = None;
            }
            cx.notify();
            window.refresh();
        }).detach();
        cx.defer_in(window, |this, window, cx| this.apply_window_size(window, cx));
        let toolbar_below = window.display(cx).is_some_and(|display| window.bounds().origin.y <= display.visible_bounds().origin.y);
        Self {
            theme,
            state,
            inline: false,
            picker_size: None,
            size_generation: Arc::new(AtomicU64::new(0)),
            chrome_visible: false,
            focus: cx.focus_handle(),
            toolbar_below,
            moving: None,
            resizing: None,
            hovered_handle: None,
            #[cfg(target_os = "macos")]
            converter: None,
            #[cfg(target_os = "macos")]
            blur: None,
            #[cfg(target_os = "macos")]
            effect_failures: PreviewEffectFailures::default(),
            preview,
            toolbar,
            frame_dims: None,
            retained_aspect,
            frames_in_window: 0,
            cadence_window_start: Instant::now(),
            paints,
            paints_at_window_start: 0,
            position_save_generation: Arc::new(AtomicU64::new(0)),
            last_saved_position: None,
        }
    }

    pub fn set_inline(&mut self, inline: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.inline == inline {
            return;
        }
        self.inline = inline;
        self.invalidate_pending_resize();
        self.resizing = None;
        self.moving = None;
        self.chrome_visible = false;
        self.hovered_handle = None;
        if !inline {
            self.apply_window_size(window, cx);
        }
        cx.notify();
    }

    pub fn inline_size(&self, viewport: (f32, f32)) -> (f32, f32) {
        inline_preview_size(&self.state, self.frame_aspect(), viewport)
    }

    pub fn is_inline(&self) -> bool {
        self.inline
    }

    pub fn picker_size(&self) -> Option<(f32, f32)> {
        self.picker_size
    }

    pub fn invalidate_pending_resize(&self) {
        self.size_generation.fetch_add(1, Ordering::AcqRel);
    }

    pub fn set_picker_size(
        &mut self,
        size: Option<(f32, f32)>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.picker_size != size {
            self.invalidate_pending_resize();
            self.picker_size = size;
            self.sync_preview_chrome(cx);
            cx.notify();
        }
    }

    #[cfg(not(target_os = "macos"))]
    pub fn preview_image(&self, cx: &gpui::App) -> Option<Arc<gpui::RenderImage>> {
        self.preview.read(cx).latest_frame.clone()
    }

    /// Called by the feed pump for every camera frame. Only the preview child
    /// is notified, so the cached toolbar chrome is reused; `refresh` is
    /// reserved for the first frame, where an inactive window may need the
    /// explicit ask (unit-2 finding).
    pub fn frame_arrived(
        &mut self,
        #[cfg(target_os = "macos")] frame: frameever_recording::NativeCameraFrame,
        #[cfg(not(target_os = "macos"))] frame: CameraPreviewFrame,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        #[cfg(target_os = "macos")]
        {
            let Some(epoch) = Feeds::global(cx).read(cx).camera_preview_epoch() else {
                return;
            };
            use core_foundation::base::TCFType as _;
            use core_video::pixel_buffer::{CVPixelBuffer, CVPixelBufferRef};

            let failures_before = self.effect_failures;
            if self.state.background_blur != BlurMode::Off && camera_blur::is_low_spec_preview() {
                self.effect_failures.blur = true;
            }
            let blur_mode = self.active_blur_mode();
            let max_dims = blur_mode.is_some().then_some(camera_blur::BLUR_MAX_DIMS);
            if let Some(converted) = frame::FrameConverter::convert(
                &mut self.converter,
                &frame,
                max_dims,
                self.state.mirrored,
            ) {
                self.effect_failures.mirror = self.state.mirrored && !converted.mirrored;
                let first_frame = self.frame_dims.is_none();
                let dims = converted.dims;
                let dims_changed = self.frame_dims != Some(dims);
                self.frame_dims = Some(dims);

                let mut paint_raw = blur_mode.is_none();
                if let Some(mode) = blur_mode {
                    let job = camera_blur::BlurJob {
                        buffer: camera_blur::SendPixelBuf(converted.buffer.clone()),
                        width: dims.0 as u32,
                        height: dims.1 as u32,
                        ring_generation: converted.generation,
                        mode,
                    };
                    match self.ensure_blur_bridge(epoch, window, cx).tx.try_send(job) {
                        Ok(()) => {}
                        // Worker busy: drop this frame and keep the last
                        // painted one -- the bounded(1) latest-wins shape of
                        // the Tauri preview's `camera_tx` (`camera.rs:424`).
                        Err(flume::TrySendError::Full(_)) => {}
                        Err(flume::TrySendError::Disconnected(_)) => {
                            // The worker could not come up (no device, no
                            // ONNX runtime, low-level failure). Same degrade
                            // as `ensure_blur_processor` returning false:
                            // raw frames, latched until the mode changes.
                            tracing::warn!(
                                "camera blur worker unavailable; preview continues unblurred"
                            );
                            self.blur = None;
                            self.effect_failures.blur = true;
                            paint_raw = true;
                        }
                    }
                }

                if paint_raw {
                    let raw =
                        converted.buffer.as_ref() as *const cidre::cv::PixelBuf as CVPixelBufferRef;
                    let buffer = unsafe { CVPixelBuffer::wrap_under_get_rule(raw) };
                    self.preview
                        .update(cx, |preview, cx| preview.set_frame(buffer, dims, cx));
                }
                if dims_changed {
                    self.apply_window_size(window, cx);
                    cx.notify();
                }
                if first_frame && !window.is_window_active() {
                    window.refresh();
                }
            } else if self.frame_dims.is_none() && self.frames_in_window == 0 {
                tracing::warn!("camera frame could not be converted for preview");
            }
            if failures_before != self.effect_failures {
                self.sync_effect_feedback(cx);
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let first_frame = self.frame_dims.is_none();
            let dims_changed = self.frame_dims != Some(frame.dims);
            self.frame_dims = Some(frame.dims);
            let previous = self.preview.update(cx, |preview, cx| {
                preview.set_frame(frame.image, frame.dims, cx)
            });
            if let Some(previous) = previous {
                let _ = window.drop_image(previous);
            }
            if dims_changed {
                self.apply_window_size(window, cx);
                cx.notify();
            }
            if first_frame && !window.is_window_active() {
                window.refresh();
            }
        }

        self.frames_in_window += 1;
        let elapsed = self.cadence_window_start.elapsed();
        if elapsed >= Duration::from_secs(5) {
            let painted = self
                .paints
                .load(Ordering::Relaxed)
                .wrapping_sub(self.paints_at_window_start);
            tracing::info!(
                fps = format!(
                    "{:.1}",
                    self.frames_in_window as f64 / elapsed.as_secs_f64()
                ),
                painted_fps = format!("{:.1}", painted as f64 / elapsed.as_secs_f64()),
                "camera preview cadence"
            );
            self.frames_in_window = 0;
            self.cadence_window_start = Instant::now();
            self.paints_at_window_start = self.paints.load(Ordering::Relaxed);
        }
    }

    #[cfg(target_os = "macos")]
    fn sync_effect_feedback(&self, cx: &mut Context<Self>) {
        let issue = self.effect_failures.issue();
        self.preview.update(cx, |preview, cx| {
            if preview.effect_issue != issue {
                preview.effect_issue = issue;
                cx.notify();
            }
        });
        cx.notify();
    }

    #[cfg(target_os = "macos")]
    fn active_blur_mode(&self) -> Option<frameever_camera_effects::BlurMode> {
        if self.effect_failures.blur || camera_blur::is_low_spec_preview() {
            return None;
        }
        match self.state.background_blur {
            BlurMode::Off => None,
            BlurMode::Light => Some(frameever_camera_effects::BlurMode::Light),
            BlurMode::Heavy => Some(frameever_camera_effects::BlurMode::Heavy),
            BlurMode::Remove => Some(frameever_camera_effects::BlurMode::Remove),
        }
    }

    #[cfg(target_os = "macos")]
    fn ensure_blur_bridge(
        &mut self,
        epoch: u64,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> &BlurBridge {
        if self
            .blur
            .as_ref()
            .is_some_and(|bridge| bridge.epoch != epoch)
        {
            self.blur = None;
        }
        if self.blur.is_none() {
            let (job_tx, job_rx) = flume::bounded::<camera_blur::BlurJob>(1);
            let (out_tx, out_rx) = flume::bounded::<camera_blur::BlurOutput>(2);
            if let Err(error) = std::thread::Builder::new()
                .name("camera-blur".into())
                .stack_size(crate::MEDIA_THREAD_STACK_SIZE)
                .spawn(move || camera_blur::run(job_rx, out_tx))
            {
                // job_rx is gone; the caller's try_send sees Disconnected and
                // latches the raw-frame fallback.
                tracing::warn!("camera blur thread failed to spawn: {error}");
            }
            let handle = window.window_handle();
            let pump = cx.spawn(async move |this, cx| {
                while let Ok(output) = out_rx.recv_async().await {
                    let first = match this.update(cx, |this: &mut CameraWindow, cx| {
                        this.blurred_frame_arrived(output, epoch, cx)
                    }) {
                        Ok(first) => first,
                        Err(_) => break,
                    };
                    if first {
                        handle
                            .update(cx, |_, window, _| {
                                if !window.is_window_active() {
                                    window.refresh();
                                }
                            })
                            .ok();
                    }
                }
            });
            self.blur = Some(BlurBridge {
                epoch,
                tx: job_tx,
                first_output_pending: true,
                _pump: pump,
            });
        }
        self.blur.as_ref().expect("just ensured")
    }

    /// A blurred surface came back from the worker; paint it. Returns whether
    /// it was the bridge's first output (the pump may owe an explicit
    /// refresh).
    #[cfg(target_os = "macos")]
    fn blurred_frame_arrived(
        &mut self,
        output: camera_blur::BlurOutput,
        epoch: u64,
        cx: &mut Context<Self>,
    ) -> bool {
        // A stale output can land after the mode flips back to Off; the raw
        // path is already painting again, so drop it.
        if self.active_blur_mode() != Some(output.mode)
            || Feeds::global(cx).read(cx).camera_preview_epoch() != Some(epoch)
        {
            return false;
        }
        let first = self
            .blur
            .as_mut()
            .is_some_and(|bridge| std::mem::take(&mut bridge.first_output_pending));

        use core_foundation::base::TCFType as _;
        use core_video::pixel_buffer::{CVPixelBuffer, CVPixelBufferRef};
        let dims = (output.width as usize, output.height as usize);
        let raw = &*output.buffer.0 as *const cidre::cv::PixelBuf as CVPixelBufferRef;
        let buffer = unsafe { CVPixelBuffer::wrap_under_get_rule(raw) };
        self.preview.update(cx, |preview, cx| {
            preview.set_frame(buffer, dims, cx);
            preview.cutout_frame = output.cutout;
        });
        first
    }

    fn frame_aspect(&self) -> Option<f32> {
        self.frame_dims
            .map(|(width, height)| width as f32 / height.max(1) as f32)
            .or(self.retained_aspect)
    }

    pub fn focus_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        self.chrome_visible = self.chrome_allowed(cx);
        cx.notify();
    }

    fn chrome_allowed(&self, cx: &gpui::App) -> bool {
        let session = crate::session::RecordingSession::global(cx);
        let session = session.read(cx);
        layout::controls_allowed(self.inline, session.phase, session.mode())
    }

    fn preview_offset(&self) -> f32 {
        if self.toolbar_below { 0. } else { CAMERA_TOOLBAR_HEIGHT }
    }

    fn bubble_origin(&self, window: &Window) -> (f32, f32) {
        let bounds = window.bounds();
        (bounds.origin.x.into(), f32::from(bounds.origin.y) + self.preview_offset())
    }

    fn pointer_global(window: &Window, point: gpui::Point<gpui::Pixels>) -> gpui::Point<gpui::Pixels> {
        window.bounds().origin + point
    }

    fn place_bubble(&mut self, position: (f32, f32), snap: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.inline { return; }
        let (width, window_height) = self.picker_size.unwrap_or_else(|| window_size(&self.state, self.frame_aspect()));
        let height = window_height - CAMERA_TOOLBAR_HEIGHT;
        let display = display_for_point(f64::from(position.0 + width / 2.), f64::from(position.1 + height / 2.));
        let Some(work) = crate::app_windows::display_work_area(display.as_ref(), cx) else {
            tracing::warn!("camera move skipped because its display work area is unavailable");
            return;
        };
        let layout = layout::camera_layout([position.0, position.1, width, height], [work.origin.x.into(), work.origin.y.into(), work.size.width.into(), work.size.height.into()], snap);
        let [x, y, width, height] = layout.frame;
        if self.toolbar_below != layout.toolbar_below {
            self.toolbar_below = layout.toolbar_below;
            cx.notify();
        }
        let generation = self.size_generation.fetch_add(1, Ordering::AcqRel) + 1;
        let generations = self.size_generation.clone();
        if let Some(native) = platform::native_window(window) {
            #[cfg(target_os = "windows")]
            cx.spawn(async move |_, _| {
                if generations.load(Ordering::Acquire) == generation {
                    platform::set_window_logical_frame(&native, x.into(), y.into(), width.into(), height.into());
                }
            }).detach();
            #[cfg(not(target_os = "windows"))]
            {
                let old = window.bounds();
                let (frame_x, frame_y, _, frame_height) = platform::window_frame(&native);
                let native_x = frame_x + f64::from(x - f32::from(old.origin.x));
                let native_y = frame_y + frame_height - f64::from(y - f32::from(old.origin.y)) - f64::from(height);
                cx.spawn(async move |_, _| {
                    if generations.load(Ordering::Acquire) == generation {
                        platform::set_window_frame(&native, native_x, native_y, width.into(), height.into());
                    }
                }).detach();
            }
        } else {
            tracing::warn!("camera move skipped because its native window is unavailable");
        }
    }

    fn handle_move(&mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(drag) = &self.moving else { return; };
        let pointer = Self::pointer_global(window, event.position);
        let position = (drag.origin.0 + f32::from(pointer.x - drag.pointer.x), drag.origin.1 + f32::from(pointer.y - drag.pointer.y));
        self.place_bubble(position, !event.modifiers.control, window, cx);
    }

    fn handle_key(&mut self, event: &gpui::KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.inline || event.keystroke.modifiers.control || event.keystroke.modifiers.alt || event.keystroke.modifiers.platform { return; }
        let step = if event.keystroke.modifiers.shift { 10. } else { 1. };
        let (x, y) = self.bubble_origin(window);
        match event.keystroke.key.as_str() {
            "left" => self.place_bubble((x - step, y), false, window, cx),
            "right" => self.place_bubble((x + step, y), false, window, cx),
            "up" => self.place_bubble((x, y - step), false, window, cx),
            "down" => self.place_bubble((x, y + step), false, window, cx),
            "+" | "=" => self.mutate_state(window, cx, |state| state.size += 10.),
            "-" => self.mutate_state(window, cx, |state| state.size -= 10.),
            "s" => self.mutate_state(window, cx, |state| state.shape = match state.shape { CameraShape::Round => CameraShape::Square, CameraShape::Square => CameraShape::Full, CameraShape::Full => CameraShape::Round }),
            "m" => self.mutate_state(window, cx, |state| state.mirrored = !state.mirrored),
            "b" => self.mutate_state(window, cx, |state| state.background_blur = state.background_blur.cycle()),
            "delete" | "backspace" => self.close(cx),
            "escape" => { window.blur(cx); self.chrome_visible = false; cx.notify(); },
            _ => return,
        }
        cx.stop_propagation();
    }

    fn toolbar_scale(&self) -> f32 {
        toolbar_scale(&self.state)
    }

    fn toolbar_buttons(&self) -> usize {
        let width = self.picker_size.map_or_else(
            || window_size(&self.state, self.frame_aspect()).0,
            |(width, _)| width,
        );
        toolbar_buttons(self.toolbar_scale(), width)
    }

    /// Pushes the chrome inputs the preview renders with (its own notify is
    /// the only thing that busts its cache).
    fn sync_preview_chrome(&mut self, cx: &mut Context<Self>) {
        let radius = picker_preview_radius(&self.state, self.picker_size);
        let size = clamp_size(self.state.size);
        self.preview
            .update(cx, |preview, cx| preview.set_chrome(radius, size, cx));
    }

    fn mutate_state(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        mutate: impl FnOnce(&mut CameraWindowState),
    ) {
        #[cfg(target_os = "macos")]
        let before = self.state;
        self.picker_size = None;
        mutate(&mut self.state);
        self.state.size = clamp_size(self.state.size);
        #[cfg(not(target_os = "macos"))]
        Feeds::global(cx).update(cx, |feeds, _| {
            feeds.set_camera_preview_state(self.state.mirrored, self.state.background_blur)
        });
        #[cfg(target_os = "macos")]
        {
            self.effect_failures.reset_changed(before, self.state);
            if before.mirrored != self.state.mirrored {
                self.converter = None;
            }
            self.sync_effect_feedback(cx);
            if self.state.background_blur == BlurMode::Off {
                // Ends the worker thread, dropping the ONNX session and every
                // GPU texture -- `release_blur_resources`
                // (`camera.rs:1477-1484`).
                self.blur = None;
            }
        }
        self.apply_window_size(window, cx);
        self.sync_preview_chrome(cx);
        self.persist(cx);
        cx.notify();
    }

    fn apply_window_size(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.place_bubble(self.bubble_origin(window), false, window, cx);
    }

    fn persist(&self, cx: &mut Context<Self>) {
        let state = self.state;
        cx.background_executor()
            .spawn(async move {
                store::update(|persisted| persisted.camera_window = Some(state));
            })
            .detach();
    }

    /// `queueCameraPositionSave` (`camera.tsx:93-112`): a 200ms debounce over
    /// the window's logical top-left, skipped when it has not moved a whole
    /// pixel.
    fn queue_position_save(&mut self, window: &Window, cx: &mut Context<Self>) {
        let origin = window.bounds().origin;
        let (x, y) = (f32::from(origin.x), f32::from(origin.y));
        if self
            .last_saved_position
            .is_some_and(|(sx, sy)| (sx - x).abs() < 1. && (sy - y).abs() < 1.)
        {
            return;
        }
        self.last_saved_position = Some((x, y));
        let generation = self.position_save_generation.fetch_add(1, Ordering::AcqRel) + 1;
        let generations = self.position_save_generation.clone();
        let executor = cx.background_executor().clone();
        cx.background_executor()
            .spawn(async move {
                executor.timer(Duration::from_millis(200)).await;
                if generations.load(Ordering::Acquire) == generation {
                    persist_camera_position(x as f64, y as f64);
                }
            })
            .detach();
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        // Deselecting the camera is what closes the window (`Feeds` observes
        // the selection); mirrors the Tauri close button closing the webview
        // and the main window's selection following it.
        Feeds::global(cx).update(cx, |feeds, cx| feeds.set_camera(None, cx));
    }

    fn toolbar_button(
        &self,
        id: &'static str,
        icon: &'static str,
        pressed: bool,
        failed: bool,
        cx: &mut Context<Self>,
        on_click: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> impl IntoElement {
        let theme = self.theme;
        let label = match id {
            "shape" => t!("Shape"),
            "size" => t!("Size"),
            "mirror" => t!("Mirror"),
            "blur" => t!("Background blur"),
            _ => t!("Turn camera off"),
        };
        let dots = if id == "blur" && !failed {
            match self.state.background_blur {
                BlurMode::Light => 1,
                BlurMode::Heavy => 2,
                _ => 0,
            }
        } else { 0 };
        div()
            .id(id)
            .relative()
            .size(px(28.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(6.))
            .text_color(theme.muted_foreground)
            .hover(|style| style.bg(theme.accent).text_color(theme.foreground))
            .when(pressed, |this| this.bg(theme.selection).text_color(theme.selection_foreground))
            .tooltip(move |_, cx| crate::ui::Tooltip::new(&theme, label).view(cx))
            .on_mouse_down(MouseButton::Left, |_, window, cx| {
                window.prevent_default();
                cx.stop_propagation();
            })
            .on_click(cx.listener(move |this, _, window, cx| {
                cx.stop_propagation();
                on_click(this, window, cx);
            }))
            .child(svg().path(icon).size(px(16.)))
            .when(failed, |this| this.child(div().absolute().bottom_0().right_0().text_size(px(10.)).child("!")))
            .when(dots > 0, |this| this.child(
                div().absolute().bottom(px(2.)).left_0().right_0().flex().justify_center().gap(px(2.))
                    .children((0..dots).map(|_| div().size(px(3.)).rounded_full().bg(theme.foreground)))
            ))
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let buttons = self.toolbar_buttons();
        #[cfg(target_os = "macos")]
        let (mirror_failed, blur_failed) = (self.effect_failures.mirror, self.effect_failures.blur);
        #[cfg(not(target_os = "macos"))]
        let (mirror_failed, blur_failed) = (false, false);
        let shape_icon = match self.state.shape {
            CameraShape::Round => "icons/circle.svg",
            CameraShape::Square => "icons/square.svg",
            CameraShape::Full => "icons/rectangle-horizontal.svg",
        };
        div().h(px(36.)).w_full().flex_none().flex().justify_center().items_center().child(
            div().flex().items_center().gap(px(2.)).h(px(36.)).px(px(4.)).rounded(px(20.))
                .bg(theme.popover).border_1().border_color(theme.border).text_color(theme.popover_foreground)
                .when(!self.chrome_allowed(cx) || (!self.chrome_visible && self.resizing.is_none() && self.moving.is_none()), |this| this.invisible())
                .child(self.toolbar_button("shape", shape_icon, false, false, cx, |this, window, cx| {
                    this.mutate_state(window, cx, |state| {
                        state.shape = match state.shape {
                            CameraShape::Round => CameraShape::Square,
                            CameraShape::Square => CameraShape::Full,
                            CameraShape::Full => CameraShape::Round,
                        };
                    });
                }))
                .child(self.toolbar_button("size", "icons/maximize-2.svg", self.state.size >= CAMERA_PRESET_LARGE, false, cx, |this, window, cx| {
                    this.mutate_state(window, cx, |state| {
                        state.size = if state.size < CAMERA_PRESET_LARGE { CAMERA_PRESET_LARGE } else { CAMERA_PRESET_SMALL };
                    });
                }))
                .when(buttons >= 4, |bar| bar.child(self.toolbar_button("mirror", "icons/triangles-centerline-dashed-horizontal.svg", self.state.mirrored && !mirror_failed, mirror_failed, cx, |this, window, cx| {
                    this.mutate_state(window, cx, |state| state.mirrored = !state.mirrored);
                })))
                .when(buttons >= 5, |bar| bar.child(self.toolbar_button("blur", "icons/person-standing.svg", self.state.background_blur != BlurMode::Off && !blur_failed, blur_failed, cx, |this, window, cx| {
                    this.mutate_state(window, cx, |state| state.background_blur = state.background_blur.cycle());
                })))
                .child(div().flex_none().w(px(1.)).h(px(20.)).mx(px(2.)).bg(theme.border))
                .child(self.toolbar_button("close", "icons/x.svg", false, false, cx, |this, _, cx| this.close(cx))),
        )
    }

    /// `CameraResizeHandles` + `ResizeCornerHandle`: a 28px hit area in each
    /// corner holding a 14px white 2px L-bracket, 85% while the chrome is
    /// visible and 100% while hovered or resizing. A 50% black bracket sits
    /// underneath, 1px proud on every edge, so the white L keeps a contour
    /// over a light desktop (cutout mode has no backdrop behind it).
    fn render_resize_handles(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        if !self.chrome_allowed(cx) { return div().into_any_element(); }
        let visible = self.chrome_visible || self.resizing.is_some() || self.moving.is_some();
        let mut layer = div()
            .absolute()
            .top(px(self.preview_offset()))
            .bottom(px(CAMERA_TOOLBAR_HEIGHT - self.preview_offset()))
            .left_0()
            .right_0();

        for corner in ResizeCorner::ALL {
            let id: &'static str = match corner {
                ResizeCorner::NorthWest => "resize-nw",
                ResizeCorner::NorthEast => "resize-ne",
                ResizeCorner::SouthWest => "resize-sw",
                ResizeCorner::SouthEast => "resize-se",
            };
            let active = self
                .resizing
                .as_ref()
                .is_some_and(|drag| drag.corner == corner)
                || self.hovered_handle == Some(corner);

            // 14px white L-bracket 6px in from the corner, with a thicker
            // 50% black bracket underneath that pokes out 1px on every edge
            // so the L reads as a contour over a light desktop too (cutout
            // mode has no backdrop). Mirrors `ResizeCornerHandle` in
            // `CameraPreviewChrome.tsx`.
            let outline = div()
                .absolute()
                .size(px(16.))
                .border_color(gpui::hsla(0., 0., 0., 0.5));
            let bracket = div().absolute().size(px(14.)).border_color(gpui::white());
            let (outline, bracket) = match corner {
                ResizeCorner::NorthWest => (
                    outline
                        .top(px(5.))
                        .left(px(5.))
                        .border_t_4()
                        .border_l_4()
                        .rounded_tl(px(7.)),
                    bracket
                        .top(px(6.))
                        .left(px(6.))
                        .border_t_2()
                        .border_l_2()
                        .rounded_tl(px(6.)),
                ),
                ResizeCorner::NorthEast => (
                    outline
                        .top(px(5.))
                        .right(px(5.))
                        .border_t_4()
                        .border_r_4()
                        .rounded_tr(px(7.)),
                    bracket
                        .top(px(6.))
                        .right(px(6.))
                        .border_t_2()
                        .border_r_2()
                        .rounded_tr(px(6.)),
                ),
                ResizeCorner::SouthWest => (
                    outline
                        .bottom(px(5.))
                        .left(px(5.))
                        .border_b_4()
                        .border_l_4()
                        .rounded_bl(px(7.)),
                    bracket
                        .bottom(px(6.))
                        .left(px(6.))
                        .border_b_2()
                        .border_l_2()
                        .rounded_bl(px(6.)),
                ),
                ResizeCorner::SouthEast => (
                    outline
                        .bottom(px(5.))
                        .right(px(5.))
                        .border_b_4()
                        .border_r_4()
                        .rounded_br(px(7.)),
                    bracket
                        .bottom(px(6.))
                        .right(px(6.))
                        .border_b_2()
                        .border_r_2()
                        .rounded_br(px(6.)),
                ),
            };
            let glyph = div()
                .absolute()
                .inset_0()
                .opacity(if active {
                    1.0
                } else if visible {
                    0.85
                } else {
                    0.0
                })
                .child(outline)
                .child(bracket);

            // Like the toolbar buttons, no `.occlude()`: it would knock the
            // root's hover flag false over the 28px corner hit areas (even
            // while the brackets are invisible) and hide the chrome mid-
            // travel. The handle's own on_mouse_down stops propagation, which
            // is what keeps a resize press from starting a window move.
            let mut handle = div().id(id).absolute().size(px(28.)).child(glyph);
            // `cursor-nw-resize` and friends (`CameraPreviewChrome.tsx`).
            handle = match corner {
                ResizeCorner::NorthWest => handle
                    .top_0()
                    .left_0()
                    .cursor(gpui::CursorStyle::ResizeUpLeftDownRight),
                ResizeCorner::NorthEast => handle
                    .top_0()
                    .right_0()
                    .cursor(gpui::CursorStyle::ResizeUpRightDownLeft),
                ResizeCorner::SouthWest => handle
                    .bottom_0()
                    .left_0()
                    .cursor(gpui::CursorStyle::ResizeUpRightDownLeft),
                ResizeCorner::SouthEast => handle
                    .bottom_0()
                    .right_0()
                    .cursor(gpui::CursorStyle::ResizeUpLeftDownRight),
            };
            layer = layer.child(
                handle
                    .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                        let next = if *hovered { Some(corner) } else { None };
                        let relevant = *hovered || this.hovered_handle == Some(corner);
                        if relevant && this.hovered_handle != next {
                            this.hovered_handle = next;
                            cx.notify();
                        }
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &gpui::MouseDownEvent, window, cx| {
                            window.prevent_default();
                            cx.stop_propagation();
                            let (start_size, start_picker_size) =
                                resize_start(this.state.size, this.picker_size);
                            this.resizing = Some(ResizeDrag {
                                corner,
                                start_size,
                                start_picker_size,
                                start_position: Self::pointer_global(window, event.position),
                                start_origin: this.bubble_origin(window),
                                start_dimensions: { let bounds = window.bounds(); (bounds.size.width.into(), f32::from(bounds.size.height) - CAMERA_TOOLBAR_HEIGHT) },
                            });
                            cx.notify();
                        }),
                    ),
            );
        }
        layer.into_any_element()
    }

    fn handle_resize_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(drag) = &self.resizing else {
            return;
        };
        // `CameraResizeHandles.handleResizeMove`: outward drag on either axis
        // grows the square size, diagonal takes the larger delta.
        let pointer = Self::pointer_global(window, event.position);
        let delta_x = f32::from(pointer.x - drag.start_position.x);
        let delta_y = f32::from(pointer.y - drag.start_position.y);
        let corner = drag.corner;
        let origin = drag.start_origin;
        let dimensions = drag.start_dimensions;
        let dx = if drag.corner.east() {
            delta_x
        } else {
            -delta_x
        };
        let dy = if drag.corner.south() {
            delta_y
        } else {
            -delta_y
        };
        let delta = dx.max(dy);
        match resize_target(drag.start_size, drag.start_picker_size, delta) {
            ResizeTarget::Picker(next) => {
                if self.picker_size.is_some_and(|current| {
                    (current.0 - next.0).abs() <= 0.5 && (current.1 - next.1).abs() <= 0.5
                }) {
                    return;
                }
                self.picker_size = Some(next);

                self.sync_preview_chrome(cx);
                cx.notify();
            }
            ResizeTarget::State(next) => {
                if self.picker_size.is_none() && (next - self.state.size).abs() <= 0.5 {
                    return;
                }
                self.picker_size = None;
                self.state.size = next;

                self.sync_preview_chrome(cx);
                cx.notify();
            }
        }
        let next = self.picker_size.unwrap_or_else(|| window_size(&self.state, self.frame_aspect()));
        self.place_bubble((
            origin.0 + if corner.east() { 0. } else { dimensions.0 - next.0 },
            origin.1 + if corner.south() { 0. } else { dimensions.1 - (next.1 - CAMERA_TOOLBAR_HEIGHT) },
        ), false, window, cx);
    }

    fn end_resize(&mut self, cx: &mut Context<Self>) {
        if self.resizing.take().is_some() {
            self.persist(cx);
            cx.notify();
        }
    }
}

impl Render for CameraWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.theme.refresh(window, cx, false) {
            let theme = self.theme;
            self.preview.update(cx, |preview, cx| { preview.theme = theme; cx.notify(); });
            self.toolbar.update(cx, |_, cx| cx.notify());
        }
        let interacting = self.resizing.is_some() || self.moving.is_some();
        let preview_offset = self.preview_offset();
        let toolbar_top = if self.toolbar_below { f32::from(window.viewport_size().height) - 36. } else { 0. };
        div()
            .id("camera-root")
            .relative()
            .track_focus(&self.focus)
            .size_full()
            .font_family(crate::theme::ui_font())
            .font_weight(FontWeight::MEDIUM)
            .on_key_down(cx.listener(Self::handle_key))
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                let visible = *hovered && this.chrome_allowed(cx);
                if this.chrome_visible != visible {
                    this.chrome_visible = visible;
                    cx.notify();
                }
            }))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                if this.resizing.is_none() && !this.inline {
                    this.moving = Some(MoveDrag { pointer: Self::pointer_global(window, event.position), origin: this.bubble_origin(window) });
                    window.focus(&this.focus, cx);
                    cx.notify();
                }
            }))
            .when(interacting, |this| {
                let move_camera = cx.entity().downgrade();
                let up_camera = cx.entity().downgrade();
                this.child(gpui::canvas(
                    |_, _, _| (),
                    move |_, (), window, _| {
                        let camera = move_camera.clone();
                        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                            if phase != gpui::DispatchPhase::Bubble { return; }
                            camera.update(cx, |this, cx| {
                                if event.dragging() {
                                    if this.resizing.is_some() { this.handle_resize_move(event, window, cx); }
                                    else { this.handle_move(event, window, cx); }
                                } else {
                                    this.moving = None;
                                    this.end_resize(cx);
                                    cx.notify();
                                }
                            }).ok();
                        });
                        let camera = up_camera.clone();
                        window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                            if phase != gpui::DispatchPhase::Bubble || event.button != MouseButton::Left { return; }
                            camera.update(cx, |this, cx| { this.moving = None; this.end_resize(cx); cx.notify(); }).ok();
                        });
                    },
                ))
            })
            .child(self.preview.clone().cached(StyleRefinement::default().absolute().top(px(preview_offset)).bottom(px(CAMERA_TOOLBAR_HEIGHT - preview_offset)).w_full()))
            .child(self.toolbar.clone().cached(StyleRefinement::default().absolute().top(px(toolbar_top)).w_full().h(px(36.))))
            .child(self.render_resize_handles(cx))
    }
}

// ---------------------------------------------------------------------------
// Position persistence -- the shared store's `cameraWindowPosition`
// ---------------------------------------------------------------------------

/// `WindowPosition` (`general_settings.rs:145-150`), camelCase in the store
/// like the rest of `general_settings`. `DisplayId` is the same
/// `scap_targets` type the Tauri app serializes, so the bytes interop.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredWindowPosition {
    x: f64,
    y: f64,
    #[serde(default)]
    display_id: Option<scap_targets::DisplayId>,
}

fn display_contains(display: &scap_targets::Display, x: f64, y: f64) -> bool {
    display.raw_handle().logical_bounds().is_some_and(|bounds| {
        x >= bounds.position().x()
            && x < bounds.position().x() + bounds.size().width()
            && y >= bounds.position().y()
            && y < bounds.position().y() + bounds.size().height()
    })
}

/// `display_for_position` (`lib.rs:1612-1625`).
pub fn display_for_point(x: f64, y: f64) -> Option<scap_targets::Display> {
    scap_targets::Display::list()
        .into_iter()
        .find(|display| display_contains(display, x, y))
}

/// `is_position_on_monitor_name` (`windows.rs:857-878`).
fn position_on_monitor_name(name: &str, x: f64, y: f64) -> bool {
    scap_targets::Display::list()
        .into_iter()
        .any(|display| display.name().as_deref() == Some(name) && display_contains(&display, x, y))
}

/// `is_position_on_any_screen` (`windows.rs:880-896`).
fn any_display_contains(x: f64, y: f64) -> bool {
    scap_targets::Display::list()
        .into_iter()
        .any(|display| display_contains(&display, x, y))
}

/// The saved position that is still valid for `preferred` -- the Tauri
/// restore decision (`windows.rs:2289-2321`): the per-monitor-name entry for
/// the preferred monitor when the point is still on a monitor of that name,
/// else the global `cameraWindowPosition` on the preferred monitor; with no
/// monitor name, the global position validated against its recorded display
/// (or any screen).
fn saved_position(preferred: &scap_targets::Display) -> Option<(f64, f64)> {
    let settings = store::store_section(store::GENERAL_SETTINGS);
    let parse = |value: &Value| serde_json::from_value::<StoredWindowPosition>(value.clone()).ok();
    let global = settings.get("cameraWindowPosition").and_then(parse);

    let position = match preferred.name().filter(|name| !name.trim().is_empty()) {
        Some(name) => settings
            .get("cameraWindowPositionsByMonitorName")
            .and_then(|value| value.as_object())
            .and_then(|map| map.get(&name))
            .and_then(parse)
            .filter(|pos| position_on_monitor_name(&name, pos.x, pos.y))
            .or_else(|| global.filter(|pos| position_on_monitor_name(&name, pos.x, pos.y))),
        None => global.filter(|pos| match &pos.display_id {
            Some(id) => scap_targets::Display::from_id(id)
                .is_some_and(|display| display_contains(&display, pos.x, pos.y)),
            None => any_display_contains(pos.x, pos.y),
        }),
    };
    position.map(|pos| (pos.x, pos.y))
}

/// Where a fresh bubble opens: the saved position when still valid, else the
/// Tauri default slot -- bottom-right of the preferred monitor measured with
/// `DEFAULT_WINDOW_SIZE = 230.0 * 2.0` (`windows.rs:2160, 2340-2344`; the
/// constant, not the actual window size, is what the shipping app subtracts).
pub fn opening_position(preferred: &scap_targets::Display, window_size: (f32, f32)) -> (f32, f32) {
    let saved = saved_position(preferred);
    if let Some((x, y)) = saved {
        tracing::info!(x, y, "camera window restored to its saved position");
    }
    let bounds = saved
        .and_then(|(x, y)| display_for_point(x, y))
        .and_then(|display| display.raw_handle().logical_bounds())
        .or_else(|| preferred.raw_handle().logical_bounds());
    opening_position_in_bounds(saved, bounds, window_size)
}

fn opening_position_in_bounds(
    saved: Option<(f64, f64)>,
    bounds: Option<scap_targets::bounds::LogicalBounds>,
    window_size: (f32, f32),
) -> (f32, f32) {
    let Some(bounds) = bounds else {
        let (x, y) = saved.unwrap_or((100., 100.));
        return (x as f32, y as f32);
    };
    const TAURI_DEFAULT_WINDOW_SIZE: f64 = 230.0 * 2.0;
    let (dx, dy) = (bounds.position().x(), bounds.position().y());
    let (dw, dh) = (bounds.size().width(), bounds.size().height());
    let (x, y) = saved.unwrap_or((
        dx + dw - TAURI_DEFAULT_WINDOW_SIZE - 100.,
        dy + dh - TAURI_DEFAULT_WINDOW_SIZE - 100.,
    ));
    (
        x.clamp(dx, dx + (dw - f64::from(window_size.0)).max(0.)) as f32,
        y.clamp(dy, dy + (dh - f64::from(window_size.1)).max(0.)) as f32,
    )
}

#[cfg(test)]
mod opening_position_tests {
    use super::*;
    use scap_targets::bounds::{LogicalBounds, LogicalPosition, LogicalSize};

    fn display(x: f64, y: f64, width: f64, height: f64) -> Option<LogicalBounds> {
        Some(LogicalBounds::new(
            LogicalPosition::new(x, y),
            LogicalSize::new(width, height),
        ))
    }

    #[test]
    fn restored_wide_preview_fits_after_display_resolution_changes() {
        let state = CameraWindowState {
            size: 600.,
            shape: CameraShape::Full,
            ..Default::default()
        };
        let dimensions = window_size(&state, Some(16. / 9.));
        let (x, y) = opening_position_in_bounds(
            Some((1800., 1000.)),
            display(0., 0., 1920., 1080.),
            dimensions,
        );
        assert!((x + dimensions.0 - 1920.).abs() < 0.001);
        assert_eq!(y + dimensions.1, 1080.);
    }

    #[test]
    fn a_saved_position_off_the_display_opens_on_screen() {
        assert_eq!(
            opening_position_in_bounds(
                Some((5000., -900.)),
                display(0., 0., 1920., 1080.),
                (230., 286.)
            ),
            (1690., 0.),
        );
    }

    #[test]
    fn fresh_large_preview_fits_on_a_display_with_negative_origin() {
        assert_eq!(
            opening_position_in_bounds(None, display(-1280., -800., 1280., 800.), (600., 656.)),
            (-600., -656.),
        );
    }

    #[test]
    fn oversized_preview_is_anchored_to_the_display_origin() {
        assert_eq!(
            opening_position_in_bounds(None, display(-640., 200., 640., 480.), (1066., 656.)),
            (-640., 200.),
        );
    }

    #[test]
    fn valid_saved_position_and_default_slot_are_preserved() {
        let bounds = display(1920., 0., 1920., 1080.);
        assert_eq!(
            opening_position_in_bounds(Some((2000., 400.)), bounds, (230., 286.)),
            (2000., 400.),
        );
        assert_eq!(
            opening_position_in_bounds(None, bounds, (230., 286.)),
            (3280., 520.),
        );
    }

    #[test]
    fn missing_display_geometry_preserves_the_existing_fallback() {
        assert_eq!(
            opening_position_in_bounds(None, None, (600., 656.)),
            (100., 100.),
        );
        assert_eq!(
            opening_position_in_bounds(Some((300., 400.)), None, (600., 656.)),
            (300., 400.),
        );
    }
}

/// `update_camera_window_position_settings` (`lib.rs:1645-1666`): the global
/// position plus the per-monitor-name map entry, both carrying the display
/// id. Runs on the background executor -- two store read-modify-writes.
fn persist_camera_position(x: f64, y: f64) {
    let display = display_for_point(x, y);
    let position = StoredWindowPosition {
        x,
        y,
        display_id: display.as_ref().map(|display| display.id()),
    };
    let Ok(value) = serde_json::to_value(&position) else {
        return;
    };
    store::set_store_setting(
        store::GENERAL_SETTINGS,
        "cameraWindowPosition",
        value.clone(),
    );

    let monitor_name = display
        .as_ref()
        .and_then(|display| display.name())
        .filter(|name| !name.trim().is_empty());
    if let Some(name) = monitor_name {
        let mut map = store::store_section(store::GENERAL_SETTINGS)
            .remove("cameraWindowPositionsByMonitorName")
            .and_then(|value| match value {
                Value::Object(map) => Some(map),
                _ => None,
            })
            .unwrap_or_default();
        map.insert(name, value);
        store::set_store_setting(
            store::GENERAL_SETTINGS,
            "cameraWindowPositionsByMonitorName",
            Value::Object(map),
        );
    }
}

#[cfg(all(test, target_os = "macos"))]
mod retained_preview_tests {
    use super::*;
    use core_video::pixel_buffer::{
        CVPixelBuffer, kCVPixelFormatType_32ARGB, kCVPixelFormatType_32BGRA,
    };

    fn buffer(width: usize, height: usize) -> CVPixelBuffer {
        let buffer = CVPixelBuffer::new(kCVPixelFormatType_32BGRA, width, height, None).unwrap();
        assert_eq!(buffer.lock_base_address(0), 0);
        let stride = buffer.get_bytes_per_row();
        let pixels = unsafe {
            std::slice::from_raw_parts_mut(buffer.get_base_address().cast::<u8>(), stride * height)
        };
        for row in pixels.chunks_exact_mut(stride) {
            for pixel in row[..width * 4].as_chunks_mut::<4>().0 {
                pixel.copy_from_slice(&[19, 71, 143, 255]);
            }
        }
        assert_eq!(buffer.unlock_base_address(0), 0);
        buffer
    }

    #[test]
    fn retained_preview_owns_pixels_after_native_surface_changes() {
        let buffer = buffer(17, 9);
        let image = snapshot_preview(&buffer).unwrap();
        assert_eq!(
            image.as_bytes(0).unwrap(),
            [19, 71, 143, 255].repeat(17 * 9)
        );
        assert_eq!(buffer.lock_base_address(0), 0);
        unsafe { buffer.get_base_address().cast::<u8>().write(200) };
        assert_eq!(buffer.unlock_base_address(0), 0);
        drop(buffer);
        assert_eq!(image.as_bytes(0).unwrap()[0], 19);
    }

    #[test]
    fn retained_preview_bounds_memory_for_large_camera_frames() {
        let buffer = buffer(3840, 2160);
        let image = snapshot_preview(&buffer).unwrap();
        assert_eq!(image.as_bytes(0).unwrap().len(), 960 * 540 * 4);
        assert_eq!(&image.as_bytes(0).unwrap()[..4], &[19, 71, 143, 255]);
    }

    #[test]
    fn cutout_preview_preserves_transparent_and_feathered_pixels() {
        let buffer = buffer(3, 1);
        assert_eq!(buffer.lock_base_address(0), 0);
        let base = unsafe { buffer.get_base_address().cast::<u8>() };
        unsafe {
            base.add(3).write(0);
            base.add(7).write(128);
        }
        assert_eq!(buffer.unlock_base_address(0), 0);
        let image = snapshot_preview(&buffer).unwrap();
        let bytes = image.as_bytes(0).unwrap();
        assert_eq!([bytes[3], bytes[7], bytes[11]], [0, 128, 255]);
        let state = CameraWindowState {
            background_blur: BlurMode::Remove,
            ..Default::default()
        };
        assert_eq!(preview_radius(&state), 0.0);
        assert_eq!(picker_preview_radius(&state, Some((230.0, 286.0))), 0.0);
        assert_eq!(state.shape, CameraShape::Round);
    }

    #[test]
    fn retained_preview_rejects_non_bgra_surfaces() {
        let buffer = CVPixelBuffer::new(kCVPixelFormatType_32ARGB, 32, 16, None).unwrap();
        assert!(snapshot_preview(&buffer).is_none());
    }
}
