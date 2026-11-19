#![cfg(windows)]

use frameever_export::{
    ExporterBase,
    mp4::{ExportCompression, Mp4ExportSettings},
};
use frameever_project::{ProjectConfiguration, XY};
use std::{fs, path::Path, process::Command};

const CHILD_ENTRY: &str = "self_test_child_entry";

#[test]
fn self_test_child_entry() {
    frameever_enc_ffmpeg::isolated_self_test::run_child_if_requested();
}

fn create_project(path: &Path, width: u32, height: u32) {
    fs::create_dir_all(path).unwrap();
    let video = Command::new("ffmpeg")
        .args(["-v", "error", "-f", "lavfi", "-i"])
        .arg(format!("testsrc2=size={width}x{height}:rate=30:duration=1"))
        .args([
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-pix_fmt",
            "yuv420p",
        ])
        .arg(path.join("display.mp4"))
        .output()
        .expect("ffmpeg on PATH");
    assert!(
        video.status.success(),
        "{}",
        String::from_utf8_lossy(&video.stderr)
    );
    fs::write(
        path.join("recording-meta.json"),
        serde_json::to_vec(&serde_json::json!({
            "pretty_name": "Odd size export",
            "display": { "path": "display.mp4", "fps": 30 }
        }))
        .unwrap(),
    )
    .unwrap();
}

fn probe(path: &Path) -> (u32, u32, u32) {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-count_frames",
            "-show_entries",
            "stream=width,height,nb_read_frames",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .output()
        .expect("ffprobe on PATH");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8_lossy(&output.stdout);
    let fields: Vec<u32> = text
        .trim()
        .split(',')
        .map(|field| field.trim().parse().unwrap())
        .collect();
    (fields[0], fields[1], fields[2])
}

#[tokio::test(flavor = "multi_thread")]
async fn odd_size_export_survives_a_crashing_media_foundation_self_test() {
    frameever_enc_ffmpeg::isolated_self_test::register_host(
        std::env::current_exe().unwrap(),
        vec![
            CHILD_ENTRY.into(),
            "--exact".into(),
            "--nocapture".into(),
            "--test-threads=1".into(),
        ],
    );

    for (width, height) in [(1446, 812), (1366, 770), (1914, 1078)] {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("project.ever");
        create_project(&project, width, height);

        let base = Box::pin(
            ExporterBase::builder(project.clone())
                .with_config(ProjectConfiguration::default())
                .with_output_path(directory.path().join("export.mp4"))
                .build(),
        )
        .await
        .unwrap();
        let written = Box::pin(
            Mp4ExportSettings {
                fps: 30,
                resolution_base: XY::new(width, height),
                compression: ExportCompression::Maximum,
                custom_bpp: None,
                force_ffmpeg_decoder: false,
                optimize_filesize: false,
            }
            .export(base, |_| true),
        )
        .await
        .unwrap_or_else(|e| panic!("{width}x{height}: {e}"));

        let (out_width, out_height, frames) = probe(&written);
        eprintln!("{width}x{height} exported as {out_width}x{out_height}, {frames} frames");
        assert!(
            !out_width.is_multiple_of(16),
            "{width}x{height} exported at {out_width}x{out_height}, which Intel's transform takes"
        );
        assert!(out_width.abs_diff(width) <= 4 && out_height.abs_diff(height) <= 2);
        assert!(frames >= 29, "{width}x{height}: {frames} frames");
    }
}
