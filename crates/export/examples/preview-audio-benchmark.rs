use frameever_export::{
    ExporterBase,
    preview::{ExportPreviewSettings, render_preview},
};
use frameever_project::{ProjectConfiguration, XY};
use std::{path::PathBuf, time::Instant};

fn main() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_stack_size(16 * 1024 * 1024)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(runtime.spawn(run())).unwrap();
}

async fn run() {
    let root = PathBuf::from(std::env::args_os().nth(1).expect("project path"));
    if !root.join("project-config.json").exists() {
        ProjectConfiguration::default().write(&root).unwrap();
    }
    let settings = ExportPreviewSettings {
        fps: 30,
        resolution_base: XY::new(1280, 720),
        compression_bpp: 0.2,
        cursor_only: false,
        gif_quality: None,
    };
    println!("sample,eager_build_ms,video_only_build_ms,render_preview_ms");
    for sample in 0..5 {
        let start = Instant::now();
        drop(
            ExporterBase::builder(root.clone())
                .with_force_ffmpeg_decoder(true)
                .build()
                .await
                .expect("eager build"),
        );
        let eager = start.elapsed().as_secs_f64() * 1e3;
        let start = Instant::now();
        drop(
            ExporterBase::builder(root.clone())
                .with_force_ffmpeg_decoder(true)
                .build_video_only()
                .await
                .expect("video-only build"),
        );
        let video_only = start.elapsed().as_secs_f64() * 1e3;
        let start = Instant::now();
        render_preview(root.clone(), 1.0, settings, true)
            .await
            .expect("render preview");
        let preview = start.elapsed().as_secs_f64() * 1e3;
        println!("{sample},{eager:.0},{video_only:.0},{preview:.0}");
    }
}
