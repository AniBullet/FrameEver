mod animated_gradient;
mod audio_only;
pub mod auto_zoom;
mod caption_timing;
mod configuration;
pub mod cursor;
pub mod filler_words;
pub mod input_display;
pub mod keyboard;
mod late_audio;
mod meta;
mod shapes;

pub use animated_gradient::*;
pub use audio_only::*;
pub use caption_timing::{
    MAX_CAPTION_WORD_DURATION, caption_text_from_words, derive_caption_track_segments,
    held_tail_clip, is_cjk, is_cjk_punctuation, source_caption_id, synchronize_captions,
};
pub use configuration::*;
pub use cursor::*;
pub use input_display::{InputDisplay, InputDisplayStyle};
pub use keyboard::*;
pub use late_audio::{
    late_audio_trim_limits, migrate_late_audio_alignment, pending_late_audio_shifts,
};
pub use meta::*;
pub use shapes::*;

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RecordingConfig {
    pub fps: u32,
    pub resolution: Resolution,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

impl Default for RecordingConfig {
    fn default() -> Self {
        Self {
            fps: 30,
            resolution: Resolution {
                width: 1920,
                height: 1080,
            },
        }
    }
}
