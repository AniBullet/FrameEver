#![cfg(target_os = "macos")]

use frameever_recording::sources::screen_capture::ScreenCaptureTarget;
use frameever_recording::{SendableShareableContent, studio_recording};
use tempfile::TempDir;

#[cfg(debug_assertions)]
#[tokio::test]
async fn studio_start_failure_marks_the_recording_failed_with_real_screen() {
    ffmpeg::init().expect("failed to initialize ffmpeg");
    let shareable_content: SendableShareableContent = cidre::sc::ShareableContent::current()
        .await
        .expect("Screen Recording permission is required")
        .into();
    let temp = TempDir::new().unwrap();
    let recording_dir = temp.path().join("failed-start.ever");
    std::fs::create_dir_all(&recording_dir).unwrap();

    let fail_point = "frameever-recording::macos::ScreenCaptureActor::new";
    frameever_fail::set_fail(fail_point, true);
    let result = studio_recording::Actor::builder(
        recording_dir.clone(),
        ScreenCaptureTarget::Display {
            id: scap_targets::Display::primary().id(),
        },
    )
    .with_fragmented(true)
    .with_max_fps(30)
    .with_keyboard_capture(false)
    .build(Some(shareable_content))
    .await;
    frameever_fail::set_fail(fail_point, false);

    let Err(error) = result else {
        panic!("the injected screen capture failure did not fail the start");
    };
    let meta = frameever_project::RecordingMeta::load_for_project(&recording_dir).unwrap();
    match meta.studio_meta().unwrap().status() {
        frameever_project::StudioRecordingStatus::Failed { error: saved } => {
            assert_eq!(saved, format!("{error:#}"));
        }
        status => panic!("a failed start left the recording {status:?}"),
    }
}
