#[tokio::main]
pub async fn main() {
    {
        use scap_direct3d::*;
        use scap_targets::Display;
        use std::time::Duration;

        let display = Display::primary();

        let mut capturer = Capturer::new(
            display.raw_handle().try_as_capture_item().unwrap(),
            Settings {
                is_border_required: Some(true),
                is_cursor_capture_enabled: Some(true),
                pixel_format: PixelFormat::R8G8B8A8Unorm,
                ..Default::default()
            },
            |frame| {
                use scap_ffmpeg::AsFFmpeg;

                let ff_frame = frame.as_ffmpeg()?;

                println!(
                    "Frame: {}x{} format={:?}",
                    ff_frame.width(),
                    ff_frame.height(),
                    ff_frame.format()
                );

                Ok(())
            },
            || Ok(()),
            None,
        )
        .unwrap();

        capturer.start().unwrap();

        std::thread::sleep(Duration::from_secs(3));

        capturer.stop().unwrap();

        std::thread::sleep(Duration::from_secs(3));
    }
}
