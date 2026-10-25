//! Engine verification suite: marker codecs, fixture generation, a reference
//! timeline model and oracle checkers.

pub mod audio_code;
pub mod audio_fixture;
pub mod audio_model;
pub mod catalog;
pub mod fixture;
pub mod media;
pub mod model;
pub mod oracle;
pub mod pipeline;
pub mod report;
#[path = "sched_windows.rs"]
pub mod sched;
pub mod sentinel;
pub mod stimulus;
pub mod video_code;
