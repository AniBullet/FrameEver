use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use frameever_audio::{AudioData, waveform_peaks};
use frameever_project::{RecordingMeta, StudioRecordingMeta};
use frameever_rendering::AudioLevels;

const CHUNK_FRAMES: usize = AudioData::SAMPLE_RATE as usize * 60;

pub fn compute_audio_levels(
    recording: &RecordingMeta,
    meta: &StudioRecordingMeta,
    cancel: &AtomicBool,
) -> Result<AudioLevels, String> {
    let StudioRecordingMeta::MultipleSegments { inner } = meta else {
        return Ok(AudioLevels::default());
    };
    let mut mic = Vec::with_capacity(inner.segments.len());
    let mut system = Vec::with_capacity(inner.segments.len());
    for segment in &inner.segments {
        for (audio, out) in [
            (segment.mic.as_ref(), &mut mic),
            (segment.system_audio.as_ref(), &mut system),
        ] {
            out.push(match audio {
                Some(audio) => track_peaks(&recording.path(&audio.path), cancel)?,
                None => Vec::new(),
            });
        }
    }
    Ok(AudioLevels::from_waveform_peaks(&mic, &system))
}

fn track_peaks(path: &Path, cancel: &AtomicBool) -> Result<Vec<f32>, String> {
    let mut peaks = Vec::new();
    let mut start = 0usize;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err("Cancelled".into());
        }
        let chunk = AudioData::from_file_range(path, start, start + CHUNK_FRAMES)
            .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
        if chunk.sample_count() == 0 {
            break;
        }
        peaks.extend(waveform_peaks([chunk.samples()], chunk.channels()));
        start += CHUNK_FRAMES;
    }
    Ok(peaks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::completed_audio::tests::{metadata, segments_mut};
    use frameever_project::RecordingMetaInner;

    fn write_steps(path: &Path, sections: &[i16]) {
        let rate = AudioData::SAMPLE_RATE;
        let frames = rate as usize * sections.len();
        let data = frames * 2;
        let mut bytes = Vec::with_capacity(44 + data);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data as u32).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&rate.to_le_bytes());
        bytes.extend_from_slice(&(rate * 2).to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&(data as u32).to_le_bytes());
        for value in sections {
            for frame in 0..rate as usize {
                let sample = if frame % 2 == 0 { *value } else { -*value };
                bytes.extend_from_slice(&sample.to_le_bytes());
            }
        }
        std::fs::write(path, bytes).unwrap();
    }

    #[test]
    fn levels_follow_each_clip_loud_then_silent() {
        let directory = tempfile::tempdir().unwrap();
        let mut recording = metadata();
        recording.project_path = directory.path().to_path_buf();
        let segments = segments_mut(&mut recording);
        segments[0].mic.as_mut().unwrap().path = "loud-then-quiet.wav".into();
        segments[0].system_audio = None;
        segments[1].mic = None;
        segments[1].system_audio = None;
        write_steps(&directory.path().join("loud-then-quiet.wav"), &[16_000, 0]);
        let RecordingMetaInner::Studio(studio) = recording.inner.clone() else {
            panic!()
        };

        let levels = compute_audio_levels(&recording, &studio, &AtomicBool::new(false)).unwrap();

        assert_eq!(levels.clips.len(), 2);
        assert!(levels.clips[1].is_empty());
        let clip = &levels.clips[0];
        assert!((19..=21).contains(&clip.len()), "{}", clip.len());
        assert!(clip[..9].iter().all(|level| *level > 0.8), "{clip:?}");
        assert!(clip[11..].iter().all(|level| *level == 0.0), "{clip:?}");
        assert!(levels.level_at(0, 0.5) > levels.level_at(0, 1.5));
    }

    #[test]
    fn cancelling_stops_before_reading() {
        let directory = tempfile::tempdir().unwrap();
        let mut recording = metadata();
        recording.project_path = directory.path().to_path_buf();
        let RecordingMetaInner::Studio(studio) = recording.inner.clone() else {
            panic!()
        };
        assert_eq!(
            compute_audio_levels(&recording, &studio, &AtomicBool::new(true)),
            Err("Cancelled".into())
        );
    }
}
