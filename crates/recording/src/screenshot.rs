use crate::sources::screen_capture::ScreenCaptureTarget;
#[cfg(target_os = "macos")]
use anyhow::Context;
use anyhow::anyhow;
use image::{DynamicImage, RgbImage, RgbaImage};
#[cfg(target_os = "macos")]
use scap_ffmpeg::AsFFmpeg;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::oneshot;
use tracing::debug;
#[cfg(target_os = "macos")]
use tracing::error;

#[cfg(target_os = "macos")]
use core_graphics::geometry::{CGPoint, CGRect, CGSize};
#[cfg(target_os = "windows")]
use scap_direct3d::{Capturer, Frame, NewCapturerError, PixelFormat, Settings};
#[cfg(target_os = "macos")]
use scap_screencapturekit::{Capturer, StreamCfgBuilder};
#[cfg(target_os = "windows")]
use std::sync::OnceLock;
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{HMODULE, HWND};
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BOX, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device,
};
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Gdi::{
    BITMAPINFO, BITMAPINFOHEADER, BitBlt, CAPTUREBLT, CreateCompatibleDC, CreateDIBSection,
    DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, HDC, ReleaseDC, SRCCOPY, SelectObject,
};
#[cfg(target_os = "windows")]
use windows::Win32::Storage::Xps::{PRINT_WINDOW_FLAGS, PrintWindow};

#[cfg(target_os = "windows")]
const WINDOWS_CAPTURE_UNSUPPORTED: &str =
    "Screen capture not supported on this device/driver. Update graphics drivers or OS.";

#[cfg(target_os = "windows")]
fn unsupported_error() -> anyhow::Error {
    anyhow!(WINDOWS_CAPTURE_UNSUPPORTED)
}

#[derive(Clone, Copy, Debug)]
enum ChannelOrder {
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    Rgba,
    Bgra,
}

fn rgb_from_rgba(
    data: &[u8],
    width: usize,
    height: usize,
    bytes_per_row: usize,
    order: ChannelOrder,
) -> Option<RgbImage> {
    let row_bytes = width.checked_mul(4)?;
    if bytes_per_row < row_bytes {
        return None;
    }

    let required_len = height.checked_mul(bytes_per_row)?;
    if data.len() < required_len {
        return None;
    }

    let width_stride = width.checked_mul(3)?;
    let rgb_len = height.checked_mul(width_stride)?;
    let mut rgb = vec![0u8; rgb_len];

    #[cfg(target_arch = "x86_64")]
    {
        if std::is_x86_feature_detected!("ssse3") {
            for y in 0..height {
                let (src_row, dst_row) =
                    rgb_row_pair(data, &mut rgb, y, bytes_per_row, row_bytes, width_stride)?;
                unsafe {
                    reorder_rgb_row_ssse3(src_row, dst_row, order);
                }
            }
            return RgbImage::from_raw(width as u32, height as u32, rgb);
        }
    }

    for y in 0..height {
        let (src_row, dst_row) =
            rgb_row_pair(data, &mut rgb, y, bytes_per_row, row_bytes, width_stride)?;
        reorder_rgb_row_scalar(src_row, dst_row, order);
    }

    RgbImage::from_raw(width as u32, height as u32, rgb)
}

fn rgb_row_pair<'a>(
    data: &'a [u8],
    rgb: &'a mut [u8],
    y: usize,
    bytes_per_row: usize,
    row_bytes: usize,
    width_stride: usize,
) -> Option<(&'a [u8], &'a mut [u8])> {
    let src_start = y.checked_mul(bytes_per_row)?;
    let src_end = src_start.checked_add(row_bytes)?;
    let dst_start = y.checked_mul(width_stride)?;
    let dst_end = dst_start.checked_add(width_stride)?;
    let src_row = data.get(src_start..src_end)?;
    let dst_row = rgb.get_mut(dst_start..dst_end)?;
    Some((src_row, dst_row))
}

fn reorder_rgb_row_scalar(src: &[u8], dst: &mut [u8], order: ChannelOrder) {
    match order {
        ChannelOrder::Rgba => rgba_to_rgb_scalar(src, dst),
        ChannelOrder::Bgra => bgra_to_rgb_scalar(src, dst),
    }
}

fn rgba_to_rgb_scalar(src: &[u8], dst: &mut [u8]) {
    for (src, dst) in src.chunks_exact(4).zip(dst.chunks_exact_mut(3)) {
        dst[0] = src[0];
        dst[1] = src[1];
        dst[2] = src[2];
    }
}

fn bgra_to_rgb_scalar(src: &[u8], dst: &mut [u8]) {
    for (src, dst) in src.chunks_exact(4).zip(dst.chunks_exact_mut(3)) {
        dst[0] = src[2];
        dst[1] = src[1];
        dst[2] = src[0];
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn reorder_rgb_row_ssse3(src: &[u8], dst: &mut [u8], order: ChannelOrder) {
    use std::arch::x86_64::*;

    unsafe {
        let mask = match order {
            ChannelOrder::Rgba => {
                _mm_setr_epi8(0, 1, 2, 4, 5, 6, 8, 9, 10, 12, 13, 14, -1, -1, -1, -1)
            }
            ChannelOrder::Bgra => {
                _mm_setr_epi8(2, 1, 0, 6, 5, 4, 10, 9, 8, 14, 13, 12, -1, -1, -1, -1)
            }
        };

        let pixels = (src.len() / 4).min(dst.len() / 3);
        let simd_pixels = pixels & !3;
        for i in (0..simd_pixels).step_by(4) {
            let v = _mm_loadu_si128(src.as_ptr().add(i * 4) as *const __m128i);
            let shuffled = _mm_shuffle_epi8(v, mask);
            let dst_ptr = dst.as_mut_ptr().add(i * 3);
            let lo = _mm_cvtsi128_si64(shuffled);
            let hi = _mm_cvtsi128_si32(_mm_srli_si128(shuffled, 8));
            std::ptr::write_unaligned(dst_ptr.cast::<i64>(), lo);
            std::ptr::write_unaligned(dst_ptr.add(8).cast::<i32>(), hi);
        }

        reorder_rgb_row_scalar(&src[simd_pixels * 4..], &mut dst[simd_pixels * 3..], order);
    }
}

fn rgba_from_raw(
    data: &[u8],
    width: usize,
    height: usize,
    bytes_per_row: usize,
    order: ChannelOrder,
) -> Option<RgbaImage> {
    let row_bytes = width.checked_mul(4)?;
    if bytes_per_row < row_bytes {
        return None;
    }

    let required_len = height.checked_mul(bytes_per_row)?;
    if data.len() < required_len {
        return None;
    }

    let width_stride = width.checked_mul(4)?;
    let rgba_len = height.checked_mul(width_stride)?;
    let mut rgba = vec![0u8; rgba_len];

    for y in 0..height {
        let src_start = y.checked_mul(bytes_per_row)?;
        let src_end = src_start.checked_add(row_bytes)?;
        let dst_start = y.checked_mul(width_stride)?;
        let dst_end = dst_start.checked_add(width_stride)?;

        let src_row = data.get(src_start..src_end)?;
        let dst_row = rgba.get_mut(dst_start..dst_end)?;

        for (src, dst) in src_row.chunks_exact(4).zip(dst_row.chunks_exact_mut(4)) {
            let (r, b) = match order {
                ChannelOrder::Rgba => (src[0], src[2]),
                ChannelOrder::Bgra => (src[2], src[0]),
            };

            dst[0] = r;
            dst[1] = src[1];
            dst[2] = b;
            dst[3] = src[3];
        }
    }

    RgbaImage::from_raw(width as u32, height as u32, rgba)
}

fn apply_window_rounded_corners(image: RgbImage, target: &ScreenCaptureTarget) -> RgbaImage {
    round_rgb_corners(image, window_corner_radius_px(target))
}

fn round_rgb_corners(image: RgbImage, corner_radius: f32) -> RgbaImage {
    let width = image.width();
    let height = image.height();

    if corner_radius <= 0.0 {
        return image::buffer::ConvertBuffer::convert(&image);
    }

    let geom = CornerGeom {
        fw: width as f32,
        fh: height as f32,
        radius: corner_radius,
    };
    let corner_x = corner_band_px(corner_radius, width) as usize;
    let corner_y = corner_band_px(corner_radius, height) as usize;
    let width_us = width as usize;
    let height_us = height as usize;
    let src = image.into_raw();
    let mut rgba = RgbaImage::new(width, height);
    let dst = rgba.as_mut();
    let src_stride = width_us * 3;
    let dst_stride = width_us * 4;
    let right = width_us.saturating_sub(corner_x);
    let x_covers_row = corner_x >= right;

    for y in 0..height_us {
        let src_off = y * src_stride;
        let dst_off = y * dst_stride;
        let src_row = &src[src_off..src_off + src_stride];
        let dst_row = &mut dst[dst_off..dst_off + dst_stride];
        let y_in_band = y < corner_y || y.saturating_add(corner_y) >= height_us;

        if !y_in_band || corner_x == 0 {
            copy_rgb_opaque(src_row, dst_row);
            continue;
        }

        if x_covers_row {
            fill_corner_span(src_row, dst_row, 0, width_us, y, geom);
            continue;
        }

        fill_corner_span(src_row, dst_row, 0, corner_x, y, geom);
        copy_rgb_opaque(
            &src_row[corner_x * 3..right * 3],
            &mut dst_row[corner_x * 4..right * 4],
        );
        fill_corner_span(src_row, dst_row, right, width_us, y, geom);
    }

    rgba
}

#[derive(Clone, Copy)]
struct CornerGeom {
    fw: f32,
    fh: f32,
    radius: f32,
}

fn corner_band_px(radius: f32, extent: u32) -> u32 {
    if extent == 0 {
        return 0;
    }

    let rounded = radius.ceil();
    if !rounded.is_finite() || rounded >= extent as f32 {
        extent
    } else if rounded <= 0.0 {
        0
    } else {
        rounded as u32
    }
}

fn copy_rgb_opaque(src: &[u8], dst: &mut [u8]) {
    for (src_px, dst_px) in src.chunks_exact(3).zip(dst.chunks_exact_mut(4)) {
        dst_px[0] = src_px[0];
        dst_px[1] = src_px[1];
        dst_px[2] = src_px[2];
        dst_px[3] = 255;
    }
}

fn fill_corner_span(
    src_row: &[u8],
    dst_row: &mut [u8],
    x_start: usize,
    x_end: usize,
    y: usize,
    geom: CornerGeom,
) {
    let y_center = y as f32 + 0.5;
    for x in x_start..x_end {
        let src_i = x * 3;
        let dst_i = x * 4;
        let alpha = rounded_corner_alpha(x as f32 + 0.5, y_center, geom.fw, geom.fh, geom.radius);
        dst_row[dst_i] = src_row[src_i];
        dst_row[dst_i + 1] = src_row[src_i + 1];
        dst_row[dst_i + 2] = src_row[src_i + 2];
        dst_row[dst_i + 3] = alpha;
    }
}

fn window_corner_radius_px(target: &ScreenCaptureTarget) -> f32 {
    match target {
        ScreenCaptureTarget::Window { id } => {
            let scale = scap_targets::Window::from_id(id)
                .and_then(|w| w.display())
                .and_then(|d| {
                    let physical = d.physical_size()?.width();
                    let logical = d.logical_size()?.width();
                    if logical > 0.0 {
                        Some(physical / logical)
                    } else {
                        None
                    }
                })
                .unwrap_or(1.0);

            #[cfg(target_os = "macos")]
            {
                (10.0 * scale) as f32
            }
            #[cfg(target_os = "windows")]
            {
                (8.0 * scale) as f32
            }
        }
        _ => 0.0,
    }
}

fn rounded_corner_alpha(fx: f32, fy: f32, fw: f32, fh: f32, radius: f32) -> u8 {
    let dx = if fx < radius {
        radius - fx
    } else if fx > fw - radius {
        fx - (fw - radius)
    } else {
        return 255;
    };

    let dy = if fy < radius {
        radius - fy
    } else if fy > fh - radius {
        fy - (fh - radius)
    } else {
        return 255;
    };

    let dist = (dx * dx + dy * dy).sqrt();

    if dist <= radius - 0.5 {
        255
    } else if dist >= radius + 0.5 {
        0
    } else {
        ((radius + 0.5 - dist).clamp(0.0, 1.0) * 255.0) as u8
    }
}

#[cfg(target_os = "macos")]
fn try_fast_capture(target: &ScreenCaptureTarget) -> Option<DynamicImage> {
    use core_graphics::display::{
        CGDisplayCreateImage, CGDisplayCreateImageForRect, kCGWindowImageBoundsIgnoreFraming,
    };
    use core_graphics::window::CGWindowID;
    use foreign_types_shared::ForeignType;

    let start = std::time::Instant::now();

    let cg_image = match target {
        ScreenCaptureTarget::Display { id } => {
            let display = scap_targets::Display::from_id(id)?;
            let display_id = display.raw_handle().inner().id;
            let image = unsafe { CGDisplayCreateImage(display_id) };
            if image.is_null() {
                return None;
            }
            unsafe { core_graphics::image::CGImage::from_ptr(image) }
        }
        ScreenCaptureTarget::Window { id } => {
            use core_graphics::display::CGRectNull;

            let window = scap_targets::Window::from_id(id)?;
            let window_id: CGWindowID = window.id().to_string().parse().ok()?;

            unsafe extern "C" {
                fn CGWindowListCreateImage(
                    screenBounds: core_graphics::display::CGRect,
                    windowOption: u32,
                    windowID: CGWindowID,
                    imageOption: u32,
                ) -> core_graphics::sys::CGImageRef;
            }

            let image = unsafe {
                CGWindowListCreateImage(
                    CGRectNull,
                    0x00000008,
                    window_id,
                    kCGWindowImageBoundsIgnoreFraming,
                )
            };

            if image.is_null() {
                return None;
            }
            unsafe { core_graphics::image::CGImage::from_ptr(image) }
        }
        ScreenCaptureTarget::Area { screen, bounds } => {
            let display = scap_targets::Display::from_id(screen)?;
            let display_id = display.raw_handle().inner().id;
            let scale = display.raw_handle().scale().unwrap_or(1.0);
            let display_bounds = display.raw_handle().logical_bounds();
            let display_physical = display.physical_size();

            tracing::info!(
                "Area screenshot debug: display_id={}, display_logical_bounds={:?}, display_physical={:?}",
                display_id,
                display_bounds,
                display_physical,
            );
            tracing::info!(
                "Area screenshot: input logical bounds=({}, {}, {}x{}), scale={}",
                bounds.position().x(),
                bounds.position().y(),
                bounds.size().width(),
                bounds.size().height(),
                scale,
            );

            let rect = CGRect::new(
                &CGPoint::new(bounds.position().x(), bounds.position().y()),
                &CGSize::new(bounds.size().width(), bounds.size().height()),
            );

            tracing::info!(
                "Area screenshot: CGRect for capture (logical/points) = origin({}, {}), size({}x{})",
                rect.origin.x,
                rect.origin.y,
                rect.size.width,
                rect.size.height,
            );

            let image = unsafe { CGDisplayCreateImageForRect(display_id, rect) };
            if image.is_null() {
                return None;
            }
            unsafe { core_graphics::image::CGImage::from_ptr(image) }
        }
        ScreenCaptureTarget::CameraOnly => {
            return None;
        }
    };

    let width = cg_image.width();
    let height = cg_image.height();
    let bytes_per_row = cg_image.bytes_per_row();

    tracing::info!(
        "Fast capture result: image dimensions = {}x{}",
        width,
        height,
    );

    use core_foundation::data::CFData;
    let cf_data: CFData = cg_image.data();
    let data = cf_data.bytes();

    let is_window = matches!(target, ScreenCaptureTarget::Window { .. });

    let result = if is_window {
        let rgba = rgba_from_raw(data, width, height, bytes_per_row, ChannelOrder::Bgra)?;
        DynamicImage::ImageRgba8(rgba)
    } else {
        let rgb = rgb_from_rgba(data, width, height, bytes_per_row, ChannelOrder::Bgra)?;
        DynamicImage::ImageRgb8(rgb)
    };

    debug!("Fast capture completed in {:?}", start.elapsed());
    Some(result)
}

#[cfg(target_os = "windows")]
fn shared_d3d_device() -> anyhow::Result<&'static ID3D11Device> {
    use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_UNKNOWN;

    static DEVICE: OnceLock<Option<ID3D11Device>> = OnceLock::new();

    let device = DEVICE.get_or_init(|| {
        let selected = match frameever_d3d_adapter::select_capture_adapter(None) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(error = %e, "screenshot: no physical hardware adapter, fast path disabled");
                return None;
            }
        };

        let mut device = None;
        let result = unsafe {
            D3D11CreateDevice(
                Some(&selected.adapter),
                D3D_DRIVER_TYPE_UNKNOWN,
                HMODULE::default(),
                Default::default(),
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                None,
            )
        };

        if let Err(e) = result {
            tracing::warn!(
                adapter = %selected.description,
                error = ?e,
                "screenshot: D3D11CreateDevice failed on pinned adapter, fast path disabled"
            );
            return None;
        }

        device
    });

    device
        .as_ref()
        .ok_or_else(|| anyhow!("D3D11 device unavailable"))
}

#[cfg(target_os = "windows")]
fn windows_fast_path_available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();

    *AVAILABLE.get_or_init(|| match scap_direct3d::is_supported() {
        Ok(true) => shared_d3d_device().is_ok(),
        _ => false,
    })
}

#[cfg(target_os = "windows")]
fn frame_to_rgb(frame: &Frame) -> anyhow::Result<RgbImage> {
    let buffer = frame
        .as_buffer()
        .map_err(|e| anyhow!("Failed to get buffer: {e:?}"))?;

    let order = match buffer.pixel_format() {
        PixelFormat::R8G8B8A8Unorm => ChannelOrder::Rgba,
        PixelFormat::B8G8R8A8Unorm => ChannelOrder::Bgra,
    };

    rgb_from_rgba(
        buffer.data(),
        buffer.width() as usize,
        buffer.height() as usize,
        buffer.stride() as usize,
        order,
    )
    .ok_or_else(|| anyhow!("Failed to create RgbImage"))
}

#[cfg(target_os = "windows")]
fn frame_to_rgba(frame: &Frame) -> anyhow::Result<RgbaImage> {
    let buffer = frame
        .as_buffer()
        .map_err(|e| anyhow!("Failed to get buffer: {e:?}"))?;

    let order = match buffer.pixel_format() {
        PixelFormat::R8G8B8A8Unorm => ChannelOrder::Rgba,
        PixelFormat::B8G8R8A8Unorm => ChannelOrder::Bgra,
    };

    rgba_from_raw(
        buffer.data(),
        buffer.width() as usize,
        buffer.height() as usize,
        buffer.stride() as usize,
        order,
    )
    .ok_or_else(|| anyhow!("Failed to create RgbaImage"))
}

#[cfg(target_os = "windows")]
fn windows_capture_settings(target: &ScreenCaptureTarget) -> anyhow::Result<(Settings, bool)> {
    if matches!(target, ScreenCaptureTarget::CameraOnly) {
        return Err(anyhow!("Camera-only not supported for screenshots"));
    }

    let mut settings = Settings {
        is_cursor_capture_enabled: Some(false),
        pixel_format: PixelFormat::B8G8R8A8Unorm,
        ..Default::default()
    };

    if let Ok(true) = Settings::can_is_border_required() {
        settings.is_border_required = Some(false);
    }

    if let Ok(true) = Settings::can_is_cursor_capture_enabled() {
        settings.is_cursor_capture_enabled = Some(false);
    }

    let mut cropped = false;

    if let ScreenCaptureTarget::Area { bounds, screen } = target {
        let display =
            scap_targets::Display::from_id(screen).ok_or_else(|| anyhow!("Display not found"))?;
        let physical = display
            .physical_size()
            .ok_or_else(|| anyhow!("Physical size not found"))?;
        let logical = display
            .logical_size()
            .ok_or_else(|| anyhow!("Logical size not found"))?;

        if logical.width() <= 0.0 || logical.height() <= 0.0 {
            return Err(anyhow!("Display logical size invalid"));
        }

        let scale = physical.width() / logical.width();
        let left = (bounds.position().x() * scale).floor();
        let top = (bounds.position().y() * scale).floor();
        let right = (left + bounds.size().width() * scale).ceil();
        let bottom = (top + bounds.size().height() * scale).ceil();

        let clamped_right = right.min(physical.width()).min(u32::MAX as f64).max(left);
        let clamped_bottom = bottom.min(physical.height()).min(u32::MAX as f64).max(top);

        if clamped_right > left && clamped_bottom > top {
            settings.crop = Some(D3D11_BOX {
                left: left.max(0.0) as u32,
                top: top.max(0.0) as u32,
                right: clamped_right as u32,
                bottom: clamped_bottom as u32,
                front: 0,
                back: 1,
            });
            cropped = true;
        }
    }

    Ok((settings, cropped))
}

#[cfg(target_os = "windows")]
fn capture_bitmap_with(
    base_dc: HDC,
    width: i32,
    height: i32,
    mut fill: impl FnMut(HDC) -> anyhow::Result<()>,
) -> anyhow::Result<Vec<u8>> {
    if width <= 0 || height <= 0 {
        return Err(unsupported_error());
    }

    if base_dc.0.is_null() {
        return Err(unsupported_error());
    }

    let mem_dc = unsafe { CreateCompatibleDC(Some(base_dc)) };
    if mem_dc.0.is_null() {
        return Err(unsupported_error());
    }

    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: 0,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        },
        bmiColors: [Default::default(); 1],
    };

    let mut data = std::ptr::null_mut();
    let bitmap =
        unsafe { CreateDIBSection(Some(mem_dc), &info, DIB_RGB_COLORS, &mut data, None, 0) };
    let bitmap = match bitmap {
        Ok(b) if !b.0.is_null() && !data.is_null() => b,
        _ => {
            unsafe {
                let _ = DeleteDC(mem_dc);
            }
            return Err(unsupported_error());
        }
    };

    let old_obj = unsafe { SelectObject(mem_dc, bitmap.into()) };

    let result = (|| {
        fill(mem_dc)?;

        let width = usize::try_from(width).map_err(|_| unsupported_error())?;
        let height = usize::try_from(height).map_err(|_| unsupported_error())?;
        let row_bytes = width.checked_mul(4).ok_or_else(unsupported_error)?;
        let len = height
            .checked_mul(row_bytes)
            .ok_or_else(unsupported_error)?;
        let slice = unsafe { std::slice::from_raw_parts(data as *const u8, len) };

        let mut buffer = vec![0u8; len];
        buffer.copy_from_slice(slice);
        Ok(buffer)
    })();

    unsafe {
        SelectObject(mem_dc, old_obj);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(mem_dc);
    }

    result
}

#[cfg(target_os = "windows")]
fn bgra_to_rgb(buffer: Vec<u8>, width: usize, height: usize) -> anyhow::Result<RgbImage> {
    let stride = width.checked_mul(4).ok_or_else(unsupported_error)?;
    rgb_from_rgba(&buffer, width, height, stride, ChannelOrder::Bgra).ok_or_else(unsupported_error)
}

#[cfg(target_os = "windows")]
fn capture_display_bounds(
    bounds: scap_targets::bounds::PhysicalBounds,
) -> anyhow::Result<RgbImage> {
    let width = bounds.size().width().round() as i32;
    let height = bounds.size().height().round() as i32;
    if width <= 0 || height <= 0 {
        return Err(unsupported_error());
    }
    let src_x = bounds.position().x().round() as i32;
    let src_y = bounds.position().y().round() as i32;

    let screen_dc = unsafe { GetDC(None) };
    let result = capture_bitmap_with(screen_dc, width, height, |mem_dc| {
        unsafe {
            BitBlt(
                mem_dc,
                0,
                0,
                width,
                height,
                Some(screen_dc),
                src_x,
                src_y,
                SRCCOPY | CAPTUREBLT,
            )
        }
        .map_err(|_| unsupported_error())
    });
    unsafe {
        ReleaseDC(None, screen_dc);
    }

    let buffer = result?;
    let width = usize::try_from(width).map_err(|_| unsupported_error())?;
    let height = usize::try_from(height).map_err(|_| unsupported_error())?;
    bgra_to_rgb(buffer, width, height)
}

#[cfg(target_os = "windows")]
fn capture_window_bitmap(hwnd: HWND, width: i32, height: i32) -> anyhow::Result<Vec<u8>> {
    let window_dc = unsafe { GetDC(Some(hwnd)) };
    let result = capture_bitmap_with(window_dc, width, height, |mem_dc| {
        unsafe {
            BitBlt(
                mem_dc,
                0,
                0,
                width,
                height,
                Some(window_dc),
                0,
                0,
                SRCCOPY | CAPTUREBLT,
            )
        }
        .map_err(|_| unsupported_error())
    });
    unsafe {
        ReleaseDC(Some(hwnd), window_dc);
    }
    result
}

#[cfg(target_os = "windows")]
fn capture_window_print(hwnd: HWND, width: i32, height: i32) -> anyhow::Result<Vec<u8>> {
    let window_dc = unsafe { GetDC(Some(hwnd)) };
    let result = capture_bitmap_with(window_dc, width, height, |mem_dc| {
        let res = unsafe { PrintWindow(hwnd, mem_dc, PRINT_WINDOW_FLAGS(2)) };

        if res.as_bool() {
            Ok(())
        } else {
            Err(unsupported_error())
        }
    });
    unsafe {
        ReleaseDC(Some(hwnd), window_dc);
    }
    result
}

#[cfg(target_os = "windows")]
fn capture_screenshot_fallback(target: ScreenCaptureTarget) -> anyhow::Result<RgbImage> {
    match target {
        ScreenCaptureTarget::Display { id } => {
            let display = scap_targets::Display::from_id(&id).ok_or_else(unsupported_error)?;
            let bounds = display
                .raw_handle()
                .physical_bounds()
                .ok_or_else(unsupported_error)?;

            let image = capture_display_bounds(bounds)?;
            debug!("Windows GDI display capture");
            Ok(image)
        }
        ScreenCaptureTarget::Window { id } => {
            let window = scap_targets::Window::from_id(&id).ok_or_else(unsupported_error)?;
            let bounds = window
                .raw_handle()
                .physical_bounds()
                .ok_or_else(unsupported_error)?;

            let width = bounds.size().width().round() as i32;
            let height = bounds.size().height().round() as i32;
            let hwnd = window.raw_handle().inner();

            let mut buffer = capture_window_bitmap(hwnd, width, height)?;
            let has_data = buffer.iter().any(|b| *b != 0);
            if !has_data {
                buffer = capture_window_print(hwnd, width, height)?;
            }

            let width = usize::try_from(width).map_err(|_| unsupported_error())?;
            let height = usize::try_from(height).map_err(|_| unsupported_error())?;
            let image = bgra_to_rgb(buffer, width, height)?;
            debug!("Windows GDI window capture");
            Ok(image)
        }
        ScreenCaptureTarget::Area { screen, .. } => {
            let display = scap_targets::Display::from_id(&screen).ok_or_else(unsupported_error)?;
            let bounds = display
                .raw_handle()
                .physical_bounds()
                .ok_or_else(unsupported_error)?;

            let image = capture_display_bounds(bounds)?;
            debug!("Windows GDI area capture");
            Ok(image)
        }
        ScreenCaptureTarget::CameraOnly => Err(unsupported_error()),
    }
}

#[cfg(target_os = "windows")]
fn gdi_or_error(
    target: &ScreenCaptureTarget,
    base_error: anyhow::Error,
) -> anyhow::Result<RgbImage> {
    match capture_screenshot_fallback(target.clone()) {
        Ok(image) => Ok(image),
        Err(fallback_err) => Err(base_error
            .context(fallback_err)
            .context(WINDOWS_CAPTURE_UNSUPPORTED)),
    }
}

#[cfg(target_os = "windows")]
fn try_fast_capture(target: &ScreenCaptureTarget) -> Option<DynamicImage> {
    use std::sync::mpsc::sync_channel;

    if !windows_fast_path_available() {
        return None;
    }

    let start = std::time::Instant::now();
    let is_window = matches!(target, ScreenCaptureTarget::Window { .. });

    let item = match target.clone() {
        ScreenCaptureTarget::Display { id } => {
            let display = scap_targets::Display::from_id(&id)?;
            display.raw_handle().try_as_capture_item().ok()?
        }
        ScreenCaptureTarget::Window { id } => {
            let window = scap_targets::Window::from_id(&id)?;
            window.raw_handle().try_as_capture_item().ok()?
        }
        ScreenCaptureTarget::Area { screen, .. } => {
            let display = scap_targets::Display::from_id(&screen)?;
            display.raw_handle().try_as_capture_item().ok()?
        }
        ScreenCaptureTarget::CameraOnly => {
            return None;
        }
    };

    let (settings, _) = windows_capture_settings(target).ok()?;
    let device = shared_d3d_device().ok().cloned();

    let (tx, rx) = sync_channel(1);

    let mut capturer = Capturer::new(
        item,
        settings,
        {
            move |frame| {
                if is_window {
                    let res = frame_to_rgba(&frame);
                    let _ = tx.try_send(res.map(DynamicImage::ImageRgba8));
                } else {
                    let res = frame_to_rgb(&frame);
                    let _ = tx.try_send(res.map(DynamicImage::ImageRgb8));
                }
                Ok(())
            }
        },
        || Ok(()),
        device,
    )
    .ok()?;

    capturer.start().ok()?;

    let res = rx.recv_timeout(Duration::from_millis(500));
    let _ = capturer.stop();

    let image = res.ok()?.ok()?;
    debug!("Windows fast capture completed in {:?}", start.elapsed());
    Some(image)
}

pub async fn capture_screenshot(target: ScreenCaptureTarget) -> anyhow::Result<DynamicImage> {
    #[cfg(target_os = "macos")]
    {
        if let Some(image) = try_fast_capture(&target) {
            return Ok(image);
        }
        debug!("Fast capture failed, falling back to SCStream");
    }

    #[cfg(target_os = "windows")]
    {
        if !windows_fast_path_available() {
            let fallback_image = gdi_or_error(
                &target,
                anyhow!("Windows.Graphics.Capture not supported on this system"),
            )?;
            return crop_area_if_needed(fallback_image, &target, false)
                .map(|img| finalize_screenshot(img, &target));
        }

        if let Some(image) = try_fast_capture(&target) {
            return Ok(image);
        }
        debug!("Fast capture failed, falling back to Windows.Graphics.Capture");
    }

    #[cfg(target_os = "windows")]
    let cropped_in_capture;

    let (tx, rx) = oneshot::channel::<anyhow::Result<RgbImage>>();
    let tx = Arc::new(Mutex::new(Some(tx)));

    #[cfg(target_os = "macos")]
    let capturer = {
        use cidre::sc;

        let content_filter = match target.clone() {
            ScreenCaptureTarget::Display { id } => {
                let display = scap_targets::Display::from_id(&id)
                    .ok_or_else(|| anyhow!("Display not found"))?;

                display
                    .raw_handle()
                    .as_content_filter(
                        sc::ShareableContent::current()
                            .await
                            .map_err(|e| anyhow!("Failed to get shareable content: {e}"))?,
                    )
                    .ok_or_else(|| anyhow!("Failed to get content filter"))?
            }
            ScreenCaptureTarget::Window { id } => {
                let window = scap_targets::Window::from_id(&id)
                    .ok_or_else(|| anyhow!("Window not found"))?;

                let sc_content = sc::ShareableContent::current()
                    .await
                    .map_err(|e| anyhow!("Failed to get shareable content: {e}"))?;
                let sc_window = window
                    .raw_handle()
                    .as_sc(sc_content)
                    .ok_or_else(|| anyhow!("Failed to get SCWindow"))?;

                sc::ContentFilter::with_desktop_independent_window(sc_window.as_ref())
            }
            ScreenCaptureTarget::Area { screen, .. } => {
                let display = scap_targets::Display::from_id(&screen)
                    .ok_or_else(|| anyhow!("Display not found"))?;

                display
                    .raw_handle()
                    .as_content_filter(
                        sc::ShareableContent::current()
                            .await
                            .map_err(|e| anyhow!("Failed to get shareable content: {e}"))?,
                    )
                    .ok_or_else(|| anyhow!("Failed to get content filter"))?
            }
            ScreenCaptureTarget::CameraOnly => {
                return Err(anyhow!("Camera-only not supported for screenshots"));
            }
        };

        let width = match target.clone() {
            ScreenCaptureTarget::Display { id } => scap_targets::Display::from_id(&id)
                .and_then(|d| d.physical_size())
                .map(|s| s.width())
                .unwrap_or(1920.0),
            ScreenCaptureTarget::Window { id } => scap_targets::Window::from_id(&id)
                .and_then(|w| w.physical_size())
                .map(|s| s.width())
                .unwrap_or(1920.0),
            ScreenCaptureTarget::Area { screen, .. } => scap_targets::Display::from_id(&screen)
                .and_then(|d| d.physical_size())
                .map(|s| s.width())
                .unwrap_or(1920.0),
            ScreenCaptureTarget::CameraOnly => {
                return Err(anyhow!("Camera-only not supported for screenshots"));
            }
        } as usize;

        let height = match target.clone() {
            ScreenCaptureTarget::Display { id } => scap_targets::Display::from_id(&id)
                .and_then(|d| d.physical_size())
                .map(|s| s.height())
                .unwrap_or(1080.0),
            ScreenCaptureTarget::Window { id } => scap_targets::Window::from_id(&id)
                .and_then(|w| w.physical_size())
                .map(|s| s.height())
                .unwrap_or(1080.0),
            ScreenCaptureTarget::Area { screen, .. } => scap_targets::Display::from_id(&screen)
                .and_then(|d| d.physical_size())
                .map(|s| s.height())
                .unwrap_or(1080.0),
            ScreenCaptureTarget::CameraOnly => {
                return Err(anyhow!("Camera-only not supported for screenshots"));
            }
        } as usize;

        let config = StreamCfgBuilder::default()
            .with_fps(60.0) // High FPS to get the first frame quickly
            .with_width(width)
            .with_height(height)
            .with_shows_cursor(false)
            .build();

        Capturer::builder(content_filter, config)
            .with_output_sample_buf_cb({
                let tx = tx.clone();
                move |frame| {
                    if let Some(tx) = tx.lock().unwrap().take() {
                        let res = (|| {
                            use scap_screencapturekit::Frame;
                            let Frame::Screen(video_frame) = frame else {
                                return Err(anyhow!("Not a screen frame"));
                            };
                            // Note: This requires VideoFrame to implement AsFFmpeg trait or similar logic.
                            // Since scap_screencapturekit::VideoFrame doesn't directly implement it in all contexts,
                            // we might need to ensure we are using the correct types or traits.
                            // Assuming scap_ffmpeg::AsFFmpeg is implemented for it if imported.
                            let ff_frame = video_frame
                                .as_ffmpeg()
                                .map_err(|e| anyhow!("Failed to convert to ffmpeg: {e:?}"))?;
                            convert_ffmpeg_frame_to_image(&ff_frame)
                        })();
                        let _ = tx.send(res);
                    }
                }
            })
            .with_stop_with_err_cb(|_, err| {
                error!("Screenshot capture error: {err:?}");
            })
            .build()
            .map_err(|e| anyhow!("Failed to build capturer: {e:?}"))?
    };

    #[cfg(target_os = "windows")]
    let mut capturer = {
        let item = match target.clone() {
            ScreenCaptureTarget::Display { id } => {
                let display = scap_targets::Display::from_id(&id)
                    .ok_or_else(|| anyhow!("Display not found"))?;
                display
                    .raw_handle()
                    .try_as_capture_item()
                    .map_err(|e| anyhow!("Failed to get capture item: {e:?}"))
            }
            ScreenCaptureTarget::Window { id } => {
                let window = scap_targets::Window::from_id(&id)
                    .ok_or_else(|| anyhow!("Window not found"))?;
                window
                    .raw_handle()
                    .try_as_capture_item()
                    .map_err(|e| anyhow!("Failed to get capture item: {e:?}"))
            }
            ScreenCaptureTarget::Area { screen, .. } => {
                let display = scap_targets::Display::from_id(&screen)
                    .ok_or_else(|| anyhow!("Display not found"))?;
                display
                    .raw_handle()
                    .try_as_capture_item()
                    .map_err(|e| anyhow!("Failed to get capture item: {e:?}"))
            }
            ScreenCaptureTarget::CameraOnly => {
                return Err(anyhow!("Camera-only not supported for screenshots"));
            }
        };

        let item = match item {
            Ok(item) => item,
            Err(error) => {
                let fallback_image = gdi_or_error(&target, error)?;
                return crop_area_if_needed(fallback_image, &target, false)
                    .map(|image| finalize_screenshot(image, &target));
            }
        };

        let (settings, cropped) = windows_capture_settings(&target)?;
        cropped_in_capture = cropped;

        match Capturer::new(
            item,
            settings,
            {
                let tx = tx.clone();
                move |frame| {
                    if let Some(tx) = tx.lock().unwrap().take() {
                        let res = frame_to_rgb(&frame);
                        let _ = tx.send(res);
                    }
                    Ok(())
                }
            },
            || Ok(()),
            shared_d3d_device().ok().cloned(),
        ) {
            Ok(capturer) => capturer,
            Err(
                e @ NewCapturerError::NotSupported
                | e @ NewCapturerError::CreateDevice(_)
                | e @ NewCapturerError::Direct3DDevice(_)
                | e @ NewCapturerError::Context(_),
            ) => {
                let fallback_image =
                    gdi_or_error(&target, anyhow!("Failed to create capturer: {e:?}"))?;
                return crop_area_if_needed(fallback_image, &target, false)
                    .map(|img| finalize_screenshot(img, &target));
            }
            Err(e) => return Err(anyhow!("Failed to create capturer: {e:?}")),
        }
    };

    #[cfg(target_os = "macos")]
    capturer
        .start()
        .await
        .map_err(|e| anyhow!("Failed to start capturer: {e:?}"))?;

    #[cfg(target_os = "windows")]
    if let Err(e) = capturer.start() {
        let fallback_image = gdi_or_error(&target, anyhow!("Failed to start capturer: {e:?}"))?;
        return crop_area_if_needed(fallback_image, &target, false)
            .map(|img| finalize_screenshot(img, &target));
    }

    let result = match tokio::time::timeout(Duration::from_secs(2), rx).await {
        Ok(Ok(res)) => res,
        Ok(Err(_)) => Err(anyhow!("Channel closed")),
        Err(_) => Err(anyhow!("Timeout waiting for screenshot")),
    };

    #[cfg(target_os = "macos")]
    capturer
        .stop()
        .await
        .map_err(|e| anyhow!("Failed to stop capturer: {e:?}"))?;

    #[cfg(target_os = "windows")]
    capturer
        .stop()
        .map_err(|e| anyhow!("Failed to stop capturer: {e:?}"))?;

    #[cfg(target_os = "windows")]
    let image = match result {
        Ok(img) => img,
        Err(err) => {
            let fallback_image = gdi_or_error(&target, err)?;
            return crop_area_if_needed(fallback_image, &target, false)
                .map(|img| finalize_screenshot(img, &target));
        }
    };

    #[cfg(not(target_os = "windows"))]
    let image = result?;

    #[cfg(target_os = "windows")]
    let skip_crop = cropped_in_capture;

    #[cfg(not(target_os = "windows"))]
    let skip_crop = false;

    let final_image = crop_area_if_needed(image, &target, skip_crop)?;

    Ok(finalize_screenshot(final_image, &target))
}

fn finalize_screenshot(image: RgbImage, target: &ScreenCaptureTarget) -> DynamicImage {
    if matches!(target, ScreenCaptureTarget::Window { .. }) {
        DynamicImage::ImageRgba8(apply_window_rounded_corners(image, target))
    } else {
        DynamicImage::ImageRgb8(image)
    }
}

fn crop_area_if_needed(
    image: RgbImage,
    target: &ScreenCaptureTarget,
    skip_crop: bool,
) -> anyhow::Result<RgbImage> {
    if let ScreenCaptureTarget::Area { bounds, screen } = target {
        if skip_crop {
            return Ok(image);
        }

        #[cfg(target_os = "macos")]
        let scale = {
            let display = scap_targets::Display::from_id(screen)
                .ok_or_else(|| anyhow!("Display not found"))?;
            display.raw_handle().scale().unwrap_or(1.0)
        };

        #[cfg(target_os = "windows")]
        let scale = {
            let display = scap_targets::Display::from_id(screen)
                .ok_or_else(|| anyhow!("Display not found"))?;
            let physical_width = display.physical_size().map(|s| s.width()).unwrap_or(1.0);
            let logical_width = display.logical_size().map(|s| s.width()).unwrap_or(1.0);
            physical_width / logical_width
        };

        let x = (bounds.position().x() * scale) as u32;
        let y = (bounds.position().y() * scale) as u32;
        let width = (bounds.size().width() * scale) as u32;
        let height = (bounds.size().height() * scale) as u32;

        let img_width = image.width();
        let img_height = image.height();

        let x = x.min(img_width);
        let y = y.min(img_height);
        let width = width.min(img_width - x);
        let height = height.min(img_height - y);

        if width == 0 || height == 0 {
            return Ok(image);
        }

        let cropped = image::imageops::crop_imm(&image, x, y, width, height).to_image();
        return Ok(cropped);
    }

    Ok(image)
}

#[cfg(target_os = "macos")]
fn convert_ffmpeg_frame_to_image(frame: &ffmpeg::frame::Video) -> anyhow::Result<RgbImage> {
    let mut scaler = ffmpeg::software::scaling::context::Context::get(
        frame.format(),
        frame.width(),
        frame.height(),
        ffmpeg::format::Pixel::RGB24,
        frame.width(),
        frame.height(),
        ffmpeg::software::scaling::flag::Flags::BILINEAR,
    )
    .context("Failed to create scaler")?;

    let mut rgb_frame = ffmpeg::frame::Video::empty();
    scaler
        .run(frame, &mut rgb_frame)
        .context("Failed to scale frame")?;

    let width = rgb_frame.width() as usize;
    let height = rgb_frame.height() as usize;
    let dst_stride = width * 3;
    let src_stride = rgb_frame.stride(0);

    if src_stride < dst_stride {
        return Err(anyhow!(
            "Source stride ({}) is less than destination stride ({}); width={}, height={}",
            src_stride,
            dst_stride,
            width,
            height
        ));
    }

    let mut img_buffer = vec![0u8; height * dst_stride];

    for y in 0..height {
        let src_slice = &rgb_frame.data(0)[y * src_stride..y * src_stride + dst_stride];
        let dst_slice = &mut img_buffer[y * dst_stride..(y + 1) * dst_stride];
        dst_slice.copy_from_slice(src_slice);
    }

    RgbImage::from_raw(width as u32, height as u32, img_buffer)
        .ok_or_else(|| anyhow!("Failed to create image buffer"))
}

#[cfg(test)]
mod screenshot_pixel_tests {
    use super::*;

    fn paint(len: usize, seed: u32) -> Vec<u8> {
        let mut state = seed;
        let mut out = vec![0u8; len];
        for byte in &mut out {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            *byte = (state >> 16) as u8;
        }
        out
    }

    fn assert_bytes_eq(got: &[u8], expected: &[u8], label: &str) {
        assert_eq!(got.len(), expected.len(), "{label} length");
        let mismatch = got
            .iter()
            .zip(expected.iter())
            .position(|(got_byte, expected_byte)| got_byte != expected_byte);
        if let Some(index) = mismatch {
            panic!(
                "{label} mismatch at {index}: got {:02x} expected {:02x}",
                got[index], expected[index]
            );
        }
    }

    fn reference_rgb(
        data: &[u8],
        width: usize,
        height: usize,
        bytes_per_row: usize,
        order: ChannelOrder,
    ) -> Option<Vec<u8>> {
        let row_bytes = width.checked_mul(4)?;
        if bytes_per_row < row_bytes {
            return None;
        }

        let required_len = height.checked_mul(bytes_per_row)?;
        if data.len() < required_len {
            return None;
        }

        let width_stride = width.checked_mul(3)?;
        let rgb_len = height.checked_mul(width_stride)?;
        let mut rgb = vec![0u8; rgb_len];

        for y in 0..height {
            let src_start = y.checked_mul(bytes_per_row)?;
            let src_end = src_start.checked_add(row_bytes)?;
            let dst_start = y.checked_mul(width_stride)?;
            let dst_end = dst_start.checked_add(width_stride)?;
            let src_row = data.get(src_start..src_end)?;
            let dst_row = rgb.get_mut(dst_start..dst_end)?;

            for (src, dst) in src_row.chunks_exact(4).zip(dst_row.chunks_exact_mut(3)) {
                let (r, b) = match order {
                    ChannelOrder::Rgba => (src[0], src[2]),
                    ChannelOrder::Bgra => (src[2], src[0]),
                };
                dst[0] = r;
                dst[1] = src[1];
                dst[2] = b;
            }
        }

        Some(rgb)
    }

    fn scalar_rgb(
        data: &[u8],
        width: usize,
        height: usize,
        bytes_per_row: usize,
        order: ChannelOrder,
    ) -> Option<Vec<u8>> {
        let row_bytes = width.checked_mul(4)?;
        if bytes_per_row < row_bytes {
            return None;
        }
        let required_len = height.checked_mul(bytes_per_row)?;
        if data.len() < required_len {
            return None;
        }
        let width_stride = width.checked_mul(3)?;
        let rgb_len = height.checked_mul(width_stride)?;
        let mut rgb = vec![0u8; rgb_len];
        for y in 0..height {
            let (src_row, dst_row) =
                rgb_row_pair(data, &mut rgb, y, bytes_per_row, row_bytes, width_stride)?;
            reorder_rgb_row_scalar(src_row, dst_row, order);
        }
        Some(rgb)
    }

    fn rgb_cases() -> Vec<(usize, usize, usize)> {
        let mut cases = Vec::new();
        let widths = [
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 15, 16, 17, 31, 63, 64, 65, 191, 1920, 1921,
        ];
        let heights = [0, 1, 2, 3, 5, 7];
        let pads = [0usize, 1, 7, 32];
        for width in widths {
            for height in heights {
                for pad in pads {
                    cases.push((width, height, pad));
                }
            }
        }
        cases.push((1920, 1080, 0));
        cases.push((1920, 1080, 64));
        cases.push((1919, 17, 3));
        cases.push((1, 1080, 0));
        cases
    }

    fn assert_rgb_path_matches_original(
        path: &str,
        packed: Option<Vec<u8>>,
        expected: &Option<Vec<u8>>,
        label: &str,
    ) {
        match (packed.as_deref(), expected.as_deref()) {
            (Some(got), Some(expected)) => {
                assert_bytes_eq(got, expected, &format!("{path} {label}"));
            }
            (None, None) => {}
            _ => panic!(
                "{path} {label} presence got {} expected {}",
                packed.is_some(),
                expected.is_some()
            ),
        }
    }

    #[test]
    fn rgb_dispatch_matches_original_for_both_channel_orders() {
        for (width, height, pad) in rgb_cases() {
            let stride = width * 4 + pad;
            let data = paint(
                height * stride + 8,
                (width as u32).wrapping_mul(19).wrapping_add(height as u32),
            );
            for order in [ChannelOrder::Rgba, ChannelOrder::Bgra] {
                let label = format!("{width}x{height} stride {stride} {order:?}");
                let expected = reference_rgb(&data, width, height, stride, order);
                let got = rgb_from_rgba(&data, width, height, stride, order)
                    .map(|image| image.into_raw());
                assert_rgb_path_matches_original("dispatch", got, &expected, &label);
            }
        }
    }

    #[test]
    fn rgb_scalar_fallback_matches_original_for_both_channel_orders() {
        for (width, height, pad) in rgb_cases() {
            let stride = width * 4 + pad;
            let data = paint(
                height * stride + 8,
                (width as u32).wrapping_mul(23).wrapping_add(pad as u32),
            );
            for order in [ChannelOrder::Rgba, ChannelOrder::Bgra] {
                let label = format!("{width}x{height} stride {stride} {order:?}");
                let expected = reference_rgb(&data, width, height, stride, order);
                let got = scalar_rgb(&data, width, height, stride, order);
                assert_rgb_path_matches_original("scalar", got, &expected, &label);
            }
        }
    }

    #[cfg(target_arch = "x86_64")]
    fn ssse3_rgb(
        data: &[u8],
        width: usize,
        height: usize,
        bytes_per_row: usize,
        order: ChannelOrder,
    ) -> Option<Vec<u8>> {
        let row_bytes = width.checked_mul(4)?;
        if bytes_per_row < row_bytes {
            return None;
        }
        let required_len = height.checked_mul(bytes_per_row)?;
        if data.len() < required_len {
            return None;
        }
        let width_stride = width.checked_mul(3)?;
        let rgb_len = height.checked_mul(width_stride)?;
        let mut rgb = vec![0u8; rgb_len];
        for y in 0..height {
            let (src_row, dst_row) =
                rgb_row_pair(data, &mut rgb, y, bytes_per_row, row_bytes, width_stride)?;
            unsafe {
                reorder_rgb_row_ssse3(src_row, dst_row, order);
            }
        }
        Some(rgb)
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn rgb_ssse3_matches_original_including_scalar_tail() {
        assert!(
            std::is_x86_feature_detected!("ssse3"),
            "ssse3 is required to execute the shuffled path"
        );
        for (width, height, pad) in rgb_cases() {
            let stride = width * 4 + pad;
            let data = paint(
                height * stride + 8,
                (width as u32).wrapping_mul(29).wrapping_add(height as u32),
            );
            for order in [ChannelOrder::Rgba, ChannelOrder::Bgra] {
                let label = format!("{width}x{height} stride {stride} {order:?}");
                let expected = reference_rgb(&data, width, height, stride, order);
                let got = ssse3_rgb(&data, width, height, stride, order);
                assert_rgb_path_matches_original("ssse3", got, &expected, &label);
            }
        }
    }

    #[test]
    fn rgb_packing_rejects_undersized_input() {
        assert!(rgb_from_rgba(&[0, 1, 2, 3], 2, 1, 4, ChannelOrder::Rgba).is_none());
        assert!(rgb_from_rgba(&[0, 1, 2, 3], 1, 2, 4, ChannelOrder::Bgra).is_none());
        assert!(rgb_from_rgba(&[0, 1, 2, 3, 4, 5, 6], 1, 1, 8, ChannelOrder::Rgba).is_none());
        let image = rgb_from_rgba(&[10, 20, 30, 40, 1, 2, 3, 4], 1, 1, 8, ChannelOrder::Bgra)
            .expect("padded row");
        assert_eq!(image.as_raw(), &[30, 20, 10]);
    }

    fn rgb_image(width: u32, height: u32, seed: u32) -> RgbImage {
        let buf = paint((width as usize) * (height as usize) * 3, seed);
        RgbImage::from_raw(width, height, buf).expect("rgb image")
    }

    fn reference_round(image: &RgbImage, radius: f32) -> RgbaImage {
        if radius <= 0.0 {
            return image::buffer::ConvertBuffer::convert(image);
        }

        let width = image.width();
        let height = image.height();
        let fw = width as f32;
        let fh = height as f32;
        let mut rgba = RgbaImage::new(width, height);
        for (x, y, rgb_pixel) in image.enumerate_pixels() {
            let alpha = rounded_corner_alpha(x as f32 + 0.5, y as f32 + 0.5, fw, fh, radius);
            rgba.put_pixel(
                x,
                y,
                image::Rgba([rgb_pixel[0], rgb_pixel[1], rgb_pixel[2], alpha]),
            );
        }
        rgba
    }

    fn assert_corners(width: u32, height: u32, radius: f32) {
        let image = rgb_image(width, height, width.wrapping_mul(17).wrapping_add(height));
        let expected = reference_round(&image, radius);
        let got = round_rgb_corners(image, radius);
        assert_bytes_eq(
            got.as_raw(),
            expected.as_raw(),
            &format!("corners {width}x{height} radius {radius}"),
        );
    }

    #[test]
    fn rounded_corners_match_original_for_varied_sizes() {
        let sizes = [
            (0, 0),
            (0, 4),
            (4, 0),
            (1, 1),
            (1, 2),
            (2, 1),
            (2, 2),
            (3, 3),
            (3, 5),
            (5, 3),
            (7, 9),
            (8, 8),
            (15, 17),
            (16, 9),
            (31, 20),
            (64, 63),
            (100, 50),
            (127, 13),
            (320, 180),
        ];
        let base_radii = [-1.0, 0.0, 0.25, 0.5, 1.0, 1.5, 2.2, 8.0, 10.5, 12.0];
        for (width, height) in sizes {
            for radius in base_radii {
                assert_corners(width, height, radius);
            }
            let min_dim = width.min(height) as f32;
            let max_dim = width.max(height) as f32;
            assert_corners(width, height, min_dim / 2.0);
            assert_corners(width, height, min_dim / 2.0 + 0.5);
            assert_corners(width, height, max_dim + 10.0);
        }
        assert_corners(1920, 1080, 0.0);
        assert_corners(1920, 1080, 12.0);
        assert_corners(1919, 17, 6.5);
    }

    #[test]
    fn rounded_corners_match_original_for_non_finite_radius() {
        for (width, height) in [(1, 1), (2, 3), (8, 5), (17, 9)] {
            assert_corners(width, height, f32::INFINITY);
            assert_corners(width, height, f32::NAN);
            assert_corners(width, height, f32::MIN_POSITIVE);
        }
    }

    #[test]
    fn non_window_screenshot_keeps_opaque_alpha() {
        let image = rgb_image(13, 9, 99);
        let expected: RgbaImage = image::buffer::ConvertBuffer::convert(&image);
        let got = apply_window_rounded_corners(image, &ScreenCaptureTarget::CameraOnly);
        assert_bytes_eq(got.as_raw(), expected.as_raw(), "camera only");
    }

    #[test]
    #[ignore]
    fn benchmark_screenshot_rgb_packing_and_corner_rounding() {
        use std::{hint::black_box, time::Instant};
        fn median(mut samples: Vec<f64>) -> f64 {
            samples.sort_by(f64::total_cmp);
            samples[samples.len() / 2]
        }
        for (width, height) in [(1920usize, 1080usize), (3840, 2160)] {
            let stride = width * 4 + 64;
            let data = paint(stride * height, 7);
            let mut before = Vec::new();
            let mut after = Vec::new();
            for _ in 0..15 {
                let start = Instant::now();
                black_box(reference_rgb(&data, width, height, stride, ChannelOrder::Bgra));
                before.push(start.elapsed().as_secs_f64() * 1000.);
                let start = Instant::now();
                black_box(rgb_from_rgba(&data, width, height, stride, ChannelOrder::Bgra));
                after.push(start.elapsed().as_secs_f64() * 1000.);
            }
            println!(
                "rgb packing {width}x{height}: {:.2} -> {:.2} ms",
                median(before),
                median(after)
            );

            let image = rgb_image(width as u32, height as u32, 11);
            let mut before = Vec::new();
            let mut after = Vec::new();
            for _ in 0..15 {
                let start = Instant::now();
                black_box(reference_round(&image, 16.));
                before.push(start.elapsed().as_secs_f64() * 1000.);
                let copy = image.clone();
                let start = Instant::now();
                black_box(round_rgb_corners(copy, 16.));
                after.push(start.elapsed().as_secs_f64() * 1000.);
            }
            println!(
                "corner rounding {width}x{height}: {:.2} -> {:.2} ms",
                median(before),
                median(after)
            );
        }
    }
}
