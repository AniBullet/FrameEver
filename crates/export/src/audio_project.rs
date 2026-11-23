use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use frameever_project::{
    AUDIO_ONLY_CANVAS_ENCODERS, AUDIO_ONLY_CANVAS_FPS, AUDIO_ONLY_CANVAS_HEIGHT,
    AUDIO_ONLY_CANVAS_WIDTH, AudioMeta, Cursors, MultipleSegment, MultipleSegments, Platform,
    ProjectConfiguration, RecordingMeta, RecordingMetaInner, StudioRecordingMeta,
    StudioRecordingStatus, VideoMeta,
};

const SEGMENT_DIR: &str = "content/segments/segment-0";
const AUDIO_STEM: &str = "audio-input";

#[derive(Debug, thiserror::Error)]
pub enum AudioProjectError {
    #[error("The file has no audio")]
    NoAudio,
    #[error("The length of the audio is unknown")]
    UnknownLength,
    #[error("Could not read the audio: {0}")]
    Unreadable(String),
    #[error("{context}: {source}")]
    Io {
        context: String,
        source: std::io::Error,
    },
    #[error("Could not write the audio only display track: {0}")]
    Standin(String),
    #[error("Could not save the project: {0}")]
    Save(String),
}

pub fn create_audio_only_project(
    parent: &Path,
    name: &str,
    pretty_name: &str,
    source: &Path,
) -> Result<RecordingMeta, AudioProjectError> {
    ffmpeg::init().map_err(|error| AudioProjectError::Unreadable(error.to_string()))?;
    let duration = audio_duration(source)?;
    let project_path = frameever_utils::storage::create_project(parent, name)
        .map_err(|source| io_error(parent, source))?;
    let mut cleanup = RemoveOnDrop(Some(project_path.clone()));
    let meta = fill_project(&project_path, pretty_name, source, duration)?;
    cleanup.0 = None;
    Ok(meta)
}

fn fill_project(
    project_path: &Path,
    pretty_name: &str,
    source: &Path,
    duration: Duration,
) -> Result<RecordingMeta, AudioProjectError> {
    let segment_dir = project_path.join(SEGMENT_DIR);
    std::fs::create_dir_all(&segment_dir).map_err(|error| io_error(&segment_dir, error))?;

    let extension = source
        .extension()
        .and_then(|extension| extension.to_str())
        .map_or_else(|| "m4a".to_string(), str::to_ascii_lowercase);
    let audio_name = format!("{AUDIO_STEM}.{extension}");
    link_or_copy(source, &segment_dir.join(&audio_name))?;

    frameever_enc_ffmpeg::standin::write_black_standin(
        &segment_dir.join("display.mp4"),
        AUDIO_ONLY_CANVAS_WIDTH,
        AUDIO_ONLY_CANVAS_HEIGHT,
        AUDIO_ONLY_CANVAS_FPS,
        duration,
        AUDIO_ONLY_CANVAS_ENCODERS,
    )
    .map_err(AudioProjectError::Standin)?;

    let meta = audio_only_meta(project_path, pretty_name, &audio_name);
    ProjectConfiguration::for_new_project()
        .write(project_path)
        .map_err(|error| io_error(project_path, error))?;
    meta.save_for_project()
        .map_err(|error| AudioProjectError::Save(format!("{error:?}")))?;
    Ok(meta)
}

fn audio_only_meta(project_path: &Path, pretty_name: &str, audio_name: &str) -> RecordingMeta {
    RecordingMeta {
        platform: Some(Platform::default()),
        project_path: project_path.to_path_buf(),
        pretty_name: pretty_name.to_string(),
        sharing: None,
        inner: RecordingMetaInner::Studio(Box::new(StudioRecordingMeta::MultipleSegments {
            inner: MultipleSegments {
                segments: vec![MultipleSegment {
                    display: VideoMeta {
                        path: format!("{SEGMENT_DIR}/display.mp4").into(),
                        fps: AUDIO_ONLY_CANVAS_FPS,
                        start_time: Some(0.0),
                        device_id: None,
                    },
                    camera: None,
                    mic: Some(AudioMeta {
                        path: format!("{SEGMENT_DIR}/{audio_name}").into(),
                        start_time: Some(0.0),
                        device_id: None,
                        gap_summary: None,
                    }),
                    system_audio: None,
                    cursor: None,
                    keyboard: None,
                    display_notch: None,
                    cursor_shape_version: None,
                }],
                cursors: Cursors::default(),
                status: Some(StudioRecordingStatus::Complete),
                cursor_shape_version: frameever_project::CURSOR_SHAPE_VERSION,
                audio_only: true,
            },
        })),
        upload: None,
    }
}

struct RemoveOnDrop(Option<PathBuf>);

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        if let Some(path) = self.0.take()
            && let Err(error) = std::fs::remove_dir_all(&path)
        {
            tracing::warn!(path = %path.display(), %error, "could not remove an unfinished audio project");
        }
    }
}

fn io_error(path: &Path, source: std::io::Error) -> AudioProjectError {
    AudioProjectError::Io {
        context: path.display().to_string(),
        source,
    }
}

fn audio_duration(path: &Path) -> Result<Duration, AudioProjectError> {
    let input = ffmpeg::format::input(&path)
        .map_err(|error| AudioProjectError::Unreadable(error.to_string()))?;
    let stream = input
        .streams()
        .best(ffmpeg::media::Type::Audio)
        .ok_or(AudioProjectError::NoAudio)?;
    let stream_seconds =
        (stream.duration() > 0).then(|| stream.duration() as f64 * f64::from(stream.time_base()));
    let container_seconds = (input.duration() > 0)
        .then(|| input.duration() as f64 / f64::from(ffmpeg::ffi::AV_TIME_BASE));
    stream_seconds
        .or(container_seconds)
        .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
        .map(Duration::from_secs_f64)
        .ok_or(AudioProjectError::UnknownLength)
}

fn link_or_copy(source: &Path, destination: &Path) -> Result<(), AudioProjectError> {
    if std::fs::hard_link(source, destination).is_ok() {
        return Ok(());
    }
    std::fs::copy(source, destination)
        .map(|_| ())
        .map_err(|error| io_error(source, error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::{AudioExportFormat, export_audio};
    use std::sync::{Arc, atomic::AtomicBool};

    fn write_tone(path: &Path, seconds: f64) {
        let rate = 48_000u32;
        let frames = (seconds * f64::from(rate)) as u32;
        let mut bytes = Vec::with_capacity(44 + frames as usize * 4);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + frames * 4).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&rate.to_le_bytes());
        bytes.extend_from_slice(&(rate * 4).to_le_bytes());
        bytes.extend_from_slice(&4u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&(frames * 4).to_le_bytes());
        for frame in 0..frames {
            let t = f64::from(frame) / f64::from(rate);
            let value = ((t * 440.0 * std::f64::consts::TAU).sin() * 12_000.0) as i16;
            bytes.extend_from_slice(&value.to_le_bytes());
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        std::fs::write(path, bytes).unwrap();
    }

    fn media_seconds(path: &Path) -> f64 {
        let input = ffmpeg::format::input(&path).unwrap();
        input.duration() as f64 / f64::from(ffmpeg::ffi::AV_TIME_BASE)
    }

    fn project_dirs(parent: &Path) -> Vec<PathBuf> {
        std::fs::read_dir(parent)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect()
    }

    async fn export(project: &Path, format: AudioExportFormat, output: PathBuf) -> PathBuf {
        export_audio(
            project.to_path_buf(),
            None,
            format,
            output,
            Arc::new(AtomicBool::new(false)),
            |_| {},
        )
        .await
        .unwrap()
    }

    #[test]
    fn a_project_from_audio_is_an_audio_only_studio_project_of_the_same_length() {
        let dir = tempfile::tempdir().unwrap();
        let tone = dir.path().join("Meeting.wav");
        write_tone(&tone, 3.2);
        let library = dir.path().join("library");

        let meta = create_audio_only_project(&library, "Meeting", "Meeting", &tone).unwrap();

        let project = meta.project_path.clone();
        assert!(frameever_utils::storage::is_project(&project));
        assert!(project.join("project-config.json").is_file());
        let reread = RecordingMeta::load_for_project(&project).unwrap();
        assert_eq!(reread.pretty_name, "Meeting");
        let studio = reread.studio_meta().unwrap();
        assert!(studio.is_audio_only());
        assert!(matches!(studio.status(), StudioRecordingStatus::Complete));
        let StudioRecordingMeta::MultipleSegments { inner } = studio else {
            panic!("multiple segments expected");
        };
        let segment = &inner.segments[0];
        assert!(segment.system_audio.is_none());
        let mic = project.join(segment.mic.as_ref().unwrap().path.as_str());
        assert_eq!(std::fs::read(&mic).unwrap(), std::fs::read(&tone).unwrap());
        let display = media_seconds(&project.join(segment.display.path.as_str()));
        assert!((display - 3.2).abs() < 0.05, "stand-in lasts {display}");
        let recordings = frameever_rendering::ProjectRecordingsMeta::new(&project, studio).unwrap();
        assert!((recordings.segments[0].display.duration - 3.2).abs() < 0.05);
    }

    #[test]
    fn a_folder_someone_else_holds_is_never_touched() {
        let dir = tempfile::tempdir().unwrap();
        let tone = dir.path().join("tone.wav");
        write_tone(&tone, 1.0);
        let library = dir.path().join("library");
        let theirs = library.join(frameever_utils::storage::project_dir_name("Call"));
        std::fs::create_dir_all(&theirs).unwrap();
        std::fs::write(theirs.join("theirs.txt"), b"keep").unwrap();

        let meta = create_audio_only_project(&library, "Call", "Call", &tone).unwrap();

        assert_ne!(meta.project_path, theirs);
        assert_eq!(std::fs::read(theirs.join("theirs.txt")).unwrap(), b"keep");
        assert_eq!(std::fs::read_dir(&theirs).unwrap().count(), 1);
    }

    #[test]
    fn an_unreadable_file_leaves_nothing_behind() {
        let dir = tempfile::tempdir().unwrap();
        let bogus = dir.path().join("bogus.wav");
        std::fs::write(&bogus, b"not audio").unwrap();
        let library = dir.path().join("library");
        std::fs::create_dir_all(&library).unwrap();

        let result = create_audio_only_project(&library, "x", "x", &bogus);

        assert!(matches!(result, Err(AudioProjectError::Unreadable(_))));
        assert!(project_dirs(&library).is_empty());
    }

    #[test]
    fn a_video_without_sound_is_refused_as_having_no_audio() {
        let dir = tempfile::tempdir().unwrap();
        let silent = dir.path().join("silent.mp4");
        ffmpeg::init().unwrap();
        frameever_enc_ffmpeg::standin::write_black_standin(
            &silent,
            64,
            64,
            30,
            Duration::from_secs(1),
            &["libx264"],
        )
        .unwrap();
        let library = dir.path().join("library");
        std::fs::create_dir_all(&library).unwrap();

        let result = create_audio_only_project(&library, "x", "x", &silent);

        assert!(matches!(result, Err(AudioProjectError::NoAudio)));
        assert!(project_dirs(&library).is_empty());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn projects_from_wav_and_mp3_export_audio_of_the_same_length() {
        let dir = tempfile::tempdir().unwrap();
        let tone = dir.path().join("tone.wav");
        write_tone(&tone, 4.0);
        let library = dir.path().join("library");
        let from_wav = create_audio_only_project(&library, "Wav", "Wav", &tone).unwrap();
        let mp3 = export(
            &from_wav.project_path,
            AudioExportFormat::Mp3,
            dir.path().join("tone.mp3"),
        )
        .await;
        assert!((media_seconds(&mp3) - 4.0).abs() < 0.1);

        let from_mp3 = create_audio_only_project(&library, "Mp3", "Mp3", &mp3).unwrap();
        let wav = export(
            &from_mp3.project_path,
            AudioExportFormat::Wav,
            dir.path().join("again.wav"),
        )
        .await;

        let seconds = media_seconds(&wav);
        assert!(
            (seconds - 4.0).abs() < 0.1,
            "re-exported audio lasts {seconds}"
        );
    }
}
