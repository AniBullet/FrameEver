use serde::{Deserialize, Serialize};
use specta::Type;

use crate::BackgroundSource;

pub const AUDIO_ONLY_CANVAS_WIDTH: u32 = 1920;
pub const AUDIO_ONLY_CANVAS_HEIGHT: u32 = 1080;
pub const AUDIO_ONLY_CANVAS_FPS: u32 = 30;
pub const AUDIO_ONLY_CANVAS_ENCODERS: &[&str] = &["libx264"];

pub const AUDIO_VISUALIZER_LIGHT_COLOR: &str = "#FFFFFF";
pub const AUDIO_VISUALIZER_DARK_COLOR: &str = "#111111";

#[derive(Type, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum AudioVisualizerStyle {
    #[default]
    Bars,
    Wave,
    Circle,
}

impl AudioVisualizerStyle {
    pub const ALL: [Self; 3] = [Self::Bars, Self::Wave, Self::Circle];
}

#[derive(Type, Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AudioVisualsConfiguration {
    pub enabled: bool,
    pub style: AudioVisualizerStyle,
    pub color: String,
    pub size: f32,
}

impl AudioVisualsConfiguration {
    pub const MIN_SIZE: f32 = 0.2;
    pub const MAX_SIZE: f32 = 1.0;
    pub const SIZES: [f32; 3] = [0.4, 0.6, 0.85];

    pub fn for_background(background: &BackgroundSource) -> Self {
        let mut config = Self::default();
        if is_light(background) {
            config.color = AUDIO_VISUALIZER_DARK_COLOR.to_string();
        }
        config
    }

    pub fn clamped_size(&self) -> f32 {
        if self.size.is_finite() {
            self.size.clamp(Self::MIN_SIZE, Self::MAX_SIZE)
        } else {
            Self::default().size
        }
    }

    pub fn size_index(&self) -> usize {
        let size = self.clamped_size();
        Self::SIZES
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| (*a - size).abs().total_cmp(&(*b - size).abs()))
            .map_or(1, |(index, _)| index)
    }
}

impl Default for AudioVisualsConfiguration {
    fn default() -> Self {
        Self {
            enabled: false,
            style: AudioVisualizerStyle::Bars,
            color: AUDIO_VISUALIZER_LIGHT_COLOR.to_string(),
            size: Self::SIZES[1],
        }
    }
}

impl crate::ProjectConfiguration {
    pub fn match_audio_visuals_to_background(&mut self) {
        if let Some(visuals) = self.audio_visuals.as_deref_mut() {
            visuals.color =
                AudioVisualsConfiguration::for_background(&self.background.source).color;
        }
    }
}

fn is_light(background: &BackgroundSource) -> bool {
    let luminance = |[r, g, b]: [u16; 3]| {
        let channel = |value: u16| f32::from(value.min(255)) / 255.0;
        0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
    };
    match background {
        BackgroundSource::Color { value, alpha } => *alpha > 127 && luminance(*value) > 0.6,
        BackgroundSource::Gradient { from, to, .. } => {
            (luminance(*from) + luminance(*to)) / 2.0 > 0.6
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visuals_start_dark_on_a_light_background_and_white_otherwise() {
        let color = |value: [u16; 3]| BackgroundSource::Color { value, alpha: 255 };
        let start =
            |background: BackgroundSource| AudioVisualsConfiguration::for_background(&background);
        assert_eq!(
            start(color([255, 255, 255])).color,
            AUDIO_VISUALIZER_DARK_COLOR
        );
        assert_eq!(
            start(color([20, 20, 30])).color,
            AUDIO_VISUALIZER_LIGHT_COLOR
        );
        assert_eq!(
            start(BackgroundSource::Color {
                value: [255, 255, 255],
                alpha: 0
            })
            .color,
            AUDIO_VISUALIZER_LIGHT_COLOR
        );
        assert_eq!(
            start(BackgroundSource::Wallpaper { path: None }).color,
            AUDIO_VISUALIZER_LIGHT_COLOR
        );
        assert!(!start(color([255, 255, 255])).enabled);
    }

    #[test]
    fn visuals_change_colour_when_the_background_turns_light_or_dark() {
        let mut config = crate::ProjectConfiguration::default();
        config.match_audio_visuals_to_background();
        assert!(config.audio_visuals.is_none());
        config.background.source = BackgroundSource::Color {
            value: [10, 10, 10],
            alpha: 255,
        };
        config.audio_visuals = Some(Box::new(AudioVisualsConfiguration::for_background(
            &config.background.source,
        )));
        config.background.source = BackgroundSource::Color {
            value: [250, 250, 250],
            alpha: 255,
        };
        config.match_audio_visuals_to_background();
        assert_eq!(
            config.audio_visuals.as_ref().unwrap().color,
            AUDIO_VISUALIZER_DARK_COLOR
        );
    }

    #[test]
    fn old_project_files_have_no_audio_visuals() {
        let config: crate::ProjectConfiguration = serde_json::from_str("{}").unwrap();
        assert!(config.audio_visuals.is_none());
        let written = serde_json::to_value(&config).unwrap();
        assert!(written.get("audioVisuals").is_none());
    }

    #[test]
    fn audio_visuals_round_trip_and_fill_missing_fields() {
        let config: crate::ProjectConfiguration =
            serde_json::from_str(r#"{"audioVisuals":{"enabled":true,"style":"circle"}}"#).unwrap();
        let visuals = config.audio_visuals.clone().unwrap();
        assert!(visuals.enabled);
        assert_eq!(visuals.style, AudioVisualizerStyle::Circle);
        assert_eq!(visuals.color, AUDIO_VISUALIZER_LIGHT_COLOR);
        assert_eq!(visuals.size, 0.6);
        let reread: crate::ProjectConfiguration =
            serde_json::from_value(serde_json::to_value(&config).unwrap()).unwrap();
        assert_eq!(reread.audio_visuals, Some(visuals));
    }

    #[test]
    fn size_is_clamped_and_snaps_to_the_nearest_choice() {
        let mut visuals = AudioVisualsConfiguration {
            size: 4.0,
            ..Default::default()
        };
        assert_eq!(visuals.clamped_size(), 1.0);
        assert_eq!(visuals.size_index(), 2);
        visuals.size = f32::NAN;
        assert_eq!(visuals.clamped_size(), 0.6);
        assert_eq!(visuals.size_index(), 1);
        visuals.size = 0.0;
        assert_eq!(visuals.clamped_size(), 0.2);
        assert_eq!(visuals.size_index(), 0);
    }
}
