use cap_mediafoundation_utils::*;
use ffmpeg::{Rational, ffi::av_rescale_q, packet};
use tracing::*;
use windows::Win32::Media::MediaFoundation::{IMFSample, MFSampleExtension_CleanPoint};

use crate::MuxerConfig;

pub struct HevcStreamMuxer {
    stream_index: usize,
    time_base: ffmpeg::Rational,
    is_finished: bool,
    frame_count: u64,
    extradata_set: bool,
}

impl HevcStreamMuxer {
    pub fn new(
        output: &mut ffmpeg::format::context::Output,
        config: MuxerConfig,
    ) -> Result<Self, ffmpeg::Error> {
        info!("Adding HEVC stream to output context");

        let hevc_codec = ffmpeg::codec::decoder::find(ffmpeg::codec::Id::HEVC)
            .ok_or(ffmpeg::Error::DecoderNotFound)?;

        let mut stream = output.add_stream(hevc_codec)?;
        let stream_index = stream.index();

        let time_base = ffmpeg::Rational::new(1, config.fps as i32 * 1000);
        stream.set_time_base(time_base);

        unsafe {
            let codecpar = (*stream.as_mut_ptr()).codecpar;
            (*codecpar).codec_type = ffmpeg::ffi::AVMediaType::AVMEDIA_TYPE_VIDEO;
            (*codecpar).codec_id = ffmpeg::ffi::AVCodecID::AV_CODEC_ID_HEVC;
            (*codecpar).width = config.width as i32;
            (*codecpar).height = config.height as i32;
            (*codecpar).bit_rate = config.bitrate as i64;
            (*codecpar).format = ffmpeg::ffi::AVPixelFormat::AV_PIX_FMT_NV12 as i32;

            (*stream.as_mut_ptr()).avg_frame_rate = ffmpeg::ffi::AVRational {
                num: config.fps as i32,
                den: 1,
            };
            (*stream.as_mut_ptr()).r_frame_rate = ffmpeg::ffi::AVRational {
                num: config.fps as i32,
                den: 1,
            };
        }

        info!(
            "HEVC stream added: {}x{} @ {} fps, {} kbps, fragmented={}",
            config.width,
            config.height,
            config.fps,
            config.bitrate / 1000,
            config.fragmented
        );

        Ok(Self {
            stream_index,
            time_base,
            is_finished: false,
            frame_count: 0,
            extradata_set: false,
        })
    }

    pub fn write_sample(
        &mut self,
        sample: &IMFSample,
        output: &mut ffmpeg::format::context::Output,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.is_finished {
            return Err("Muxer is already finished".into());
        }

        let mut packet = self.mf_sample_to_avpacket(sample)?;

        if !self.extradata_set {
            if let Some(data) = packet.data() {
                self.try_set_extradata(data, output);
            }
        }

        packet.rescale_ts(
            self.time_base,
            output.stream(self.stream_index).unwrap().time_base(),
        );

        packet.write_interleaved(output)?;
        self.frame_count += 1;

        Ok(())
    }

    fn try_set_extradata(&mut self, data: &[u8], output: &mut ffmpeg::format::context::Output) {
        let param_nals = extract_hevc_parameter_sets(data);
        if param_nals.is_empty() {
            return;
        }

        let mut extradata_buf = Vec::new();
        for nal in &param_nals {
            extradata_buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
            extradata_buf.extend_from_slice(nal);
        }

        unsafe {
            let stream = *(*output.as_mut_ptr()).streams.add(self.stream_index);
            let codecpar = (*stream).codecpar;

            if !(*codecpar).extradata.is_null() {
                ffmpeg::ffi::av_free((*codecpar).extradata as *mut _);
            }

            let buf_size = extradata_buf.len();
            let alloc_size = buf_size + ffmpeg::ffi::AV_INPUT_BUFFER_PADDING_SIZE as usize;
            let ptr = ffmpeg::ffi::av_mallocz(alloc_size) as *mut u8;
            if !ptr.is_null() {
                std::ptr::copy_nonoverlapping(extradata_buf.as_ptr(), ptr, buf_size);
                (*codecpar).extradata = ptr;
                (*codecpar).extradata_size = buf_size as i32;
                info!(
                    "HEVC extradata set: {} bytes from {} parameter NAL units",
                    buf_size,
                    param_nals.len()
                );
            }
        }

        self.extradata_set = true;
    }

    fn mf_sample_to_avpacket(&self, sample: &IMFSample) -> windows::core::Result<ffmpeg::Packet> {
        let len = unsafe { sample.GetTotalLength()? };
        let mut packet = ffmpeg::Packet::new(len as usize);

        {
            let buffer = unsafe { sample.ConvertToContiguousBuffer()? };
            let data = buffer.lock()?;

            packet
                .data_mut()
                .unwrap()
                .copy_from_slice(&data[0..len as usize]);
        }

        let pts = unsafe { sample.GetSampleTime() }
            .ok()
            .map(|v| mf_from_mf_time(self.time_base, v));
        packet.set_pts(pts);
        packet.set_dts(pts);

        let duration = unsafe { sample.GetSampleDuration() }
            .ok()
            .map(|v| mf_from_mf_time(self.time_base, v))
            .unwrap_or_default();
        packet.set_duration(duration);

        if let Ok(t) = unsafe { sample.GetUINT32(&MFSampleExtension_CleanPoint) }
            && t != 0
        {
            packet.set_flags(packet::Flags::KEY);
        }

        packet.set_stream(self.stream_index);

        Ok(packet)
    }

    pub fn finish(&mut self) -> Result<(), ffmpeg::Error> {
        if self.is_finished {
            return Ok(());
        }

        self.is_finished = true;

        info!("Finishing HEVC muxer, wrote {} frames", self.frame_count);

        Ok(())
    }

    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    pub fn is_finished(&self) -> bool {
        self.is_finished
    }
}

fn extract_hevc_parameter_sets(data: &[u8]) -> Vec<&[u8]> {
    let mut result = Vec::new();
    let nals = find_annexb_nal_units(data);
    for nal in nals {
        if nal.is_empty() {
            continue;
        }
        let nal_type = (nal[0] >> 1) & 0x3F;
        match nal_type {
            32 | 33 | 34 => {
                result.push(nal);
            }
            _ => {}
        }
    }
    result
}

fn find_annexb_nal_units(data: &[u8]) -> Vec<&[u8]> {
    let mut units = Vec::new();
    let mut i = 0;
    let mut start = None;

    while i + 2 < data.len() {
        let is_start_code_3 = data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1;
        let is_start_code_4 = i + 3 < data.len()
            && data[i] == 0
            && data[i + 1] == 0
            && data[i + 2] == 0
            && data[i + 3] == 1;

        if is_start_code_4 {
            if let Some(s) = start {
                let end = if i > 0 && data[i - 1] == 0 { i - 1 } else { i };
                if end > s {
                    units.push(&data[s..end]);
                }
            }
            start = Some(i + 4);
            i += 4;
        } else if is_start_code_3 {
            if let Some(s) = start {
                let end = if i > 0 && data[i - 1] == 0 { i - 1 } else { i };
                if end > s {
                    units.push(&data[s..end]);
                }
            }
            start = Some(i + 3);
            i += 3;
        } else {
            i += 1;
        }
    }

    if let Some(s) = start {
        if s < data.len() {
            units.push(&data[s..]);
        }
    }

    units
}

const MF_TIMEBASE: ffmpeg::Rational = ffmpeg::Rational(1, 10_000_000);

fn mf_from_mf_time(tb: Rational, stime: i64) -> i64 {
    unsafe { av_rescale_q(stime, MF_TIMEBASE.into(), tb.into()) }
}
