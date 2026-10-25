pub mod colour;
pub mod h264;
pub mod h264_packet;
pub mod hevc;
pub mod prores;
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
