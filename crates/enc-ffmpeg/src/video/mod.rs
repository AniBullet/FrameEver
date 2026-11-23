pub mod colour;
pub mod h264;
pub mod h264_packet;
pub mod hardware_crash;
pub mod hevc;
pub mod isolated_self_test;
pub mod prores;
mod rgb_to_yuv;
pub mod standin;
#[cfg(target_os = "macos")]
pub mod videotoolbox_hw;

pub(crate) fn force_software_encoder() -> bool {
    std::env::var("FRAMEEVER_EXPORT_FORCE_SOFTWARE_ENCODER").is_ok_and(|value| {
        matches!(
            value.to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

pub(crate) fn convert_or_scale(
    src: &ffmpeg::frame::Video,
    dst: &mut ffmpeg::frame::Video,
    scaler: &mut ffmpeg::software::scaling::Context,
) -> Result<(), ffmpeg::Error> {
    if rgb_to_yuv::try_convert(src, dst) {
        Ok(())
    } else {
        scaler.run(src, dst)
    }
}
