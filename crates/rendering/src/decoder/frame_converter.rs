#![allow(dead_code)]

use ffmpeg::{format, frame, software};

use crate::cpu_yuv::YuvMatrix;

pub struct FrameConverter {
    scaler: Option<ScalerState>,
    rgba_frame: frame::Video,
}

struct ScalerState {
    context: software::scaling::Context,
    input_format: format::Pixel,
    matrix: YuvMatrix,
    width: u32,
    height: u32,
}

impl FrameConverter {
    pub fn new() -> Self {
        Self {
            scaler: None,
            rgba_frame: frame::Video::empty(),
        }
    }

    pub fn convert(&mut self, frame: &mut frame::Video) -> Vec<u8> {
        if frame.format() == format::Pixel::RGBA {
            let width = frame.width() as usize;
            let height = frame.height() as usize;
            let stride = frame.stride(0);

            return copy_rgba_plane(frame.data(0), stride, width, height);
        }

        let width = frame.width();
        let height = frame.height();

        self.ensure_scaler(
            frame.format(),
            YuvMatrix::from_color_space(frame.color_space()),
            width,
            height,
        );
        self.ensure_rgba_frame(width, height);

        {
            let rgba_frame = &mut self.rgba_frame;
            self.scaler
                .as_mut()
                .expect("scaler must be initialised")
                .context
                .run(frame, rgba_frame)
                .expect("frame conversion should succeed");
        }

        let rgba_frame = &self.rgba_frame;
        copy_rgba_plane(
            rgba_frame.data(0),
            rgba_frame.stride(0),
            rgba_frame.width() as usize,
            rgba_frame.height() as usize,
        )
    }

    fn ensure_scaler(
        &mut self,
        input_format: format::Pixel,
        matrix: YuvMatrix,
        width: u32,
        height: u32,
    ) {
        let needs_new = self.scaler.as_ref().is_none_or(|state| {
            state.input_format != input_format
                || state.matrix != matrix
                || state.width != width
                || state.height != height
        });

        if needs_new {
            let mut context =
                software::converter((width, height), input_format, format::Pixel::RGBA)
                    .expect("failed to create frame scaler");
            if matrix == YuvMatrix::Bt709 {
                use_bt709_source(&mut context);
            }
            self.scaler = Some(ScalerState {
                context,
                input_format,
                matrix,
                width,
                height,
            });
            self.ensure_rgba_frame(width, height);
        }
    }

    fn ensure_rgba_frame(&mut self, width: u32, height: u32) {
        if self.rgba_frame.width() != width || self.rgba_frame.height() != height {
            self.rgba_frame = frame::Video::new(format::Pixel::RGBA, width, height);
        }
    }
}

/// swscale converts YUV with BT.601 unless told otherwise: switch the source
/// matrix to BT.709 and keep the ranges and adjustments it chose.
fn use_bt709_source(context: &mut software::scaling::Context) {
    unsafe {
        let mut inv_table = std::ptr::null_mut();
        let mut table = std::ptr::null_mut();
        let (mut src_range, mut dst_range) = (0, 0);
        let (mut brightness, mut contrast, mut saturation) = (0, 0, 0);
        if ffmpeg::ffi::sws_getColorspaceDetails(
            context.as_mut_ptr(),
            &mut inv_table,
            &mut src_range,
            &mut table,
            &mut dst_range,
            &mut brightness,
            &mut contrast,
            &mut saturation,
        ) < 0
        {
            return;
        }
        let bt709 = ffmpeg::ffi::sws_getCoefficients(ffmpeg::ffi::SWS_CS_ITU709);
        ffmpeg::ffi::sws_setColorspaceDetails(
            context.as_mut_ptr(),
            bt709,
            src_range,
            table,
            dst_range,
            brightness,
            contrast,
            saturation,
        );
    }
}

pub fn copy_rgba_plane(data: &[u8], stride: usize, width: usize, height: usize) -> Vec<u8> {
    debug_assert!(stride >= width * 4, "stride too small for RGBA frame");

    let row_len = width * 4;
    let total = row_len * height;

    if stride == row_len && data.len() >= total {
        return data[..total].to_vec();
    }

    let mut frame_buffer = Vec::with_capacity(total);
    for row in data.chunks(stride).take(height) {
        frame_buffer.extend_from_slice(&row[..row_len]);
    }
    frame_buffer
}
