use std::io::Cursor;

use anyhow::Context as _;
use rodio::{Decoder, DeviceSinkBuilder, Player};

#[path = "recording_start_sound.rs"]
mod recording_start_sound;

pub use recording_start_sound::StartCue;

pub fn prime_recording_start_sound() -> Option<StartCue> {
    crate::store::recording_sounds_enabled()
        .then(|| recording_start_sound::prime(AppSound::StartRecording.bytes()))
}

pub fn play_recording_stop() {
    if crate::store::recording_sounds_enabled() {
        AppSound::StopRecording.play();
    }
}

pub fn play_screenshot() {
    if crate::store::recording_sounds_enabled() {
        AppSound::Notification.play();
    }
}

pub async fn play_recording_start_sound(
    cue: StartCue,
    gate: frameever_recording::RecordingStartGate,
) {
    recording_start_sound::play(cue, gate).await;
}

#[derive(Clone, Copy, Debug)]
pub enum AppSound {
    StartRecording,
    StopRecording,
    Notification,
    ChapterMark,
}

impl AppSound {
    fn bytes(self) -> &'static [u8] {
        match self {
            Self::StartRecording => {
                include_bytes!("../assets/sounds/start-recording.ogg")
            }
            Self::StopRecording => {
                include_bytes!("../assets/sounds/stop-recording.ogg")
            }
            Self::Notification | Self::ChapterMark => {
                include_bytes!("../assets/sounds/action.ogg")
            }
        }
    }

    fn volume(self) -> f32 {
        match self {
            Self::ChapterMark => 0.45,
            Self::StartRecording | Self::StopRecording | Self::Notification => 1.0,
        }
    }

    pub fn play(self) {
        if let Err(error) = std::thread::Builder::new()
            .name("app-sound".into())
            .spawn(move || {
                if let Err(error) = self.play_to_end() {
                    tracing::warn!(?self, %error, "App sound unavailable");
                }
            })
        {
            tracing::warn!(%error, "Could not start app sound playback");
        }
    }

    fn play_to_end(self) -> anyhow::Result<()> {
        let mut device = DeviceSinkBuilder::open_default_sink().context("opening audio output")?;
        device.log_on_drop(false);
        let source = Decoder::new(Cursor::new(self.bytes())).context("decoding app sound")?;
        let player = Player::connect_new(device.mixer());
        player.set_volume(self.volume());
        player.append(source);
        player.sleep_until_end();
        tracing::debug!(?self, "App sound played");
        Ok(())
    }
}

pub fn play_chapter_mark() {
    if crate::store::chapter_mark_sound_enabled() {
        AppSound::ChapterMark.play();
    }
}

pub fn play_notification() {
    if crate::store::notification_sounds_enabled() {
        AppSound::Notification.play();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rodio::Source as _;

    const SOUNDS: [AppSound; 4] = [
        AppSound::StartRecording,
        AppSound::StopRecording,
        AppSound::Notification,
        AppSound::ChapterMark,
    ];

    #[test]
    fn tauri_sound_assets_decode_to_audible_pcm() {
        for sound in SOUNDS {
            let source = Decoder::new(Cursor::new(sound.bytes())).unwrap();
            let channels = source.channels().get();
            let rate = source.sample_rate().get();
            assert!(channels > 0);
            assert!(rate > 0);
            let samples: Vec<f32> = source.collect();
            assert!(samples.iter().any(|sample| *sample != 0.0), "{sound:?}");
            let duration = samples.len() as f64 / f64::from(channels) / f64::from(rate);
            assert!((0.05..10.0).contains(&duration), "{sound:?}: {duration}");
        }
    }

    #[test]
    #[ignore = "plays the recording and notification cues through the default audio output"]
    fn native_app_sound_playback() {
        for sound in SOUNDS {
            sound.play_to_end().unwrap();
        }
    }
}
