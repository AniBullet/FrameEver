use crate::session::Phase;

pub const READ_SIZE: (f32, f32) = (660., 156.);
pub const EDIT_SIZE: (f32, f32) = (660., 380.);
pub const MIN_READ_SIZE: (f32, f32) = (420., 112.);
pub const SNAP_DISTANCE: f32 = 18.;
pub const LINE_SPACINGS: [f32; 3] = [1.2, 1.35, 1.6];
pub const FONT_RANGE: (f32, f32) = (22., 52.);
pub const FONT_STEP: f32 = 2.;
pub const SPEED_STEP: i32 = 10;
pub const SPEED_MIN: u32 = 60;
pub const SPEED_MAX_WORDS: u32 = 350;
pub const SPEED_MAX_CHARACTERS: u32 = 420;
pub const READING_LINE_RANGE: (f32, f32) = (0.08, 0.8);
pub const VOICE_THRESHOLD_DB: f64 = -42.;
pub const VOICE_HOLD_SECONDS: f32 = 0.7;

fn work_contains_centre(frame: [f32; 4], work: [f32; 4]) -> bool {
    let [x, y, width, height] = frame;
    let [left, top, work_width, work_height] = work;
    let (cx, cy) = (x + width / 2., y + height / 2.);
    cx >= left && cx < left + work_width && cy >= top && cy < top + work_height
}

pub fn fit_size(size: (f32, f32), work: [f32; 4]) -> (f32, f32) {
    let [_, _, work_width, work_height] = work;
    (
        size.0.max(MIN_READ_SIZE.0).min(work_width.max(1.)),
        size.1.max(MIN_READ_SIZE.1).min(work_height.max(1.)),
    )
}

pub fn strip_frame(frame: [f32; 4], work: [f32; 4], snap: bool) -> [f32; 4] {
    let [x, y, width, height] = frame;
    let [left, top, work_width, work_height] = work;
    let width = width.min(work_width.max(1.));
    let height = height.min(work_height.max(1.));
    let mut x = x.clamp(left, left + (work_width - width).max(0.));
    let mut y = y.clamp(top, top + (work_height - height).max(0.));
    if snap {
        if y - top <= SNAP_DISTANCE {
            y = top;
        }
        let centred = left + (work_width - width) / 2.;
        if (x - centred).abs() <= SNAP_DISTANCE {
            x = centred;
        }
    }
    [x.round(), y.round(), width, height]
}

pub fn attached_to_top(frame: [f32; 4], work: [f32; 4]) -> bool {
    (frame[1] - work[1]).abs() < 0.5
}

pub fn opening_frame(saved: Option<[f32; 4]>, work: [f32; 4]) -> [f32; 4] {
    let size = fit_size(
        saved.map_or(READ_SIZE, |[_, _, width, height]| (width, height)),
        work,
    );
    if let Some([x, y, _, _]) = saved {
        let frame = [x, y, size.0, size.1];
        if work_contains_centre(frame, work) {
            return strip_frame(frame, work, false);
        }
    }
    let [left, top, work_width, _] = work;
    strip_frame(
        [left + (work_width - size.0) / 2., top, size.0, size.1],
        work,
        false,
    )
}

pub fn edit_frame(read: [f32; 4], work: [f32; 4]) -> [f32; 4] {
    let [x, y, width, _] = read;
    strip_frame([x, y, width.max(EDIT_SIZE.0), EDIT_SIZE.1], work, false)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Left,
    Right,
    Bottom,
    BottomLeft,
    BottomRight,
}

impl Edge {
    pub const ALL: [Edge; 5] = [
        Edge::Left,
        Edge::Right,
        Edge::Bottom,
        Edge::BottomLeft,
        Edge::BottomRight,
    ];

    fn moves_left(self) -> bool {
        matches!(self, Edge::Left | Edge::BottomLeft)
    }

    fn moves_right(self) -> bool {
        matches!(self, Edge::Right | Edge::BottomRight)
    }

    fn moves_bottom(self) -> bool {
        matches!(self, Edge::Bottom | Edge::BottomLeft | Edge::BottomRight)
    }
}

pub fn resize_frame(start: [f32; 4], edge: Edge, delta: (f32, f32), work: [f32; 4]) -> [f32; 4] {
    let [mut x, y, mut width, mut height] = start;
    let [left, top, work_width, work_height] = work;
    if edge.moves_right() {
        width = (width + delta.0)
            .max(MIN_READ_SIZE.0)
            .min(left + work_width - x);
    }
    if edge.moves_left() {
        let right = x + width;
        let next_x = (x + delta.0).max(left).min(right - MIN_READ_SIZE.0);
        width = right - next_x;
        x = next_x;
    }
    if edge.moves_bottom() {
        height = (height + delta.1)
            .max(MIN_READ_SIZE.1)
            .min(top + work_height - y);
    }
    [x.round(), y, width.round(), height.round()]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureStatus {
    CantHide,
    PausedWithRecording,
    HiddenFromThisRecording,
    HiddenFromRecordingsAndSharing,
    HiddenFromRecordings,
}

impl CaptureStatus {
    pub fn warns(self) -> bool {
        matches!(self, Self::CantHide | Self::PausedWithRecording)
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::CantHide => "icons/triangle-alert.svg",
            Self::PausedWithRecording => "icons/pause.svg",
            _ => "icons/eye-off.svg",
        }
    }

    pub fn shows_notice(self) -> bool {
        self.warns()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureInputs {
    pub phase: Phase,
    pub pause_uncertain: bool,
    pub hide_from_sharing: bool,
    pub hiding_requested: bool,
    pub hidden_confirmed: Option<bool>,
    pub streamed: bool,
}

pub fn capture_status(inputs: CaptureInputs) -> CaptureStatus {
    let recording = inputs.phase != Phase::Idle;
    let wants_hiding = recording || inputs.hide_from_sharing;
    if inputs.streamed
        || (wants_hiding && inputs.hiding_requested && inputs.hidden_confirmed == Some(false))
    {
        return CaptureStatus::CantHide;
    }
    if matches!(inputs.phase, Phase::Recording { paused: true }) || inputs.pause_uncertain {
        return CaptureStatus::PausedWithRecording;
    }
    if recording {
        return CaptureStatus::HiddenFromThisRecording;
    }
    if inputs.hide_from_sharing {
        CaptureStatus::HiddenFromRecordingsAndSharing
    } else {
        CaptureStatus::HiddenFromRecordings
    }
}

#[cfg(target_os = "windows")]
pub fn readback_hides(affinity: usize) -> bool {
    affinity != 0
}

#[cfg(not(target_os = "windows"))]
pub fn readback_hides(sharing: usize) -> bool {
    sharing == 0
}

pub fn phrase_ends(text: &str) -> Vec<usize> {
    let mut ends = Vec::new();
    let mut chars = text.char_indices().peekable();
    while let Some((index, character)) = chars.next() {
        if matches!(
            character,
            '.' | '。' | '!' | '?' | '！' | '？' | ',' | '，' | '\n'
        ) {
            let mut end = index + character.len_utf8();
            while let Some(&(next, space)) = chars.peek() {
                if space.is_whitespace() && space != '\n' {
                    end = next + space.len_utf8();
                    chars.next();
                } else {
                    break;
                }
            }
            ends.push(end);
        }
    }
    if ends.last() != Some(&text.len()) && !text.trim_end().is_empty() {
        ends.push(text.len());
    }
    ends
}

pub fn format_clock(seconds: u32) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

pub fn total_seconds(words: usize, speed: u32) -> u32 {
    ((words as f32 / speed.max(1) as f32) * 60.).round() as u32
}

pub fn seconds_left(total: u32, position: f32, maximum: f32) -> u32 {
    if maximum <= 0. {
        return total;
    }
    let remaining = 1. - (position / maximum).clamp(0., 1.);
    (total as f32 * remaining).round() as u32
}

pub fn step_speed(current: u32, steps: i32, chinese: bool) -> u32 {
    let maximum = if chinese {
        SPEED_MAX_CHARACTERS
    } else {
        SPEED_MAX_WORDS
    };
    (current as i32 + steps * SPEED_STEP).clamp(SPEED_MIN as i32, maximum as i32) as u32
}

pub fn step_font(current: f32, steps: f32) -> f32 {
    (current + steps * FONT_STEP).clamp(FONT_RANGE.0, FONT_RANGE.1)
}

pub fn nearest_line_spacing(value: f32) -> f32 {
    LINE_SPACINGS
        .iter()
        .copied()
        .min_by(|a, b| (a - value).abs().total_cmp(&(b - value).abs()))
        .unwrap_or(LINE_SPACINGS[1])
}

pub fn clamp_reading_line(fraction: f32) -> f32 {
    fraction.clamp(READING_LINE_RANGE.0, READING_LINE_RANGE.1)
}

pub fn band_top(height: f32, fraction: f32) -> f32 {
    (height * clamp_reading_line(fraction)).round()
}

pub fn reading_line_for(band_top: f32, height: f32) -> f32 {
    if height <= 0. {
        return READING_LINE_RANGE.0;
    }
    clamp_reading_line(band_top / height)
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct VoiceGate {
    hold: f32,
}

impl VoiceGate {
    pub fn update(&mut self, level_db: f64, elapsed: f32) -> bool {
        if level_db >= VOICE_THRESHOLD_DB {
            self.hold = VOICE_HOLD_SECONDS;
            return true;
        }
        self.hold = (self.hold - elapsed.max(0.)).max(0.);
        self.hold > 0.
    }

    pub fn reset(&mut self) {
        self.hold = 0.;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadingKey {
    TogglePlay,
    Faster,
    Slower,
    Larger,
    Smaller,
    JumpDown,
    JumpUp,
    Top,
    Edit,
    FullScreen,
    Leave,
}

pub fn reading_key(key: &str, control: bool, alt: bool) -> Option<ReadingKey> {
    if control || alt {
        return None;
    }
    Some(match key {
        "space" => ReadingKey::TogglePlay,
        "up" => ReadingKey::Faster,
        "down" => ReadingKey::Slower,
        "+" | "=" => ReadingKey::Larger,
        "-" => ReadingKey::Smaller,
        "pagedown" => ReadingKey::JumpDown,
        "pageup" => ReadingKey::JumpUp,
        "home" => ReadingKey::Top,
        "e" => ReadingKey::Edit,
        "f" => ReadingKey::FullScreen,
        "escape" => ReadingKey::Leave,
        _ => return None,
    })
}

pub fn jump(position: f32, lines: f32, line_height: f32, maximum: f32) -> f32 {
    (position + lines * line_height).clamp(0., maximum.max(0.))
}

pub fn passed_phrases(ends_y: &[(usize, f32)], line_height: f32, band_top: f32) -> usize {
    ends_y
        .iter()
        .take_while(|(_, top)| top + line_height < band_top + line_height * 0.2)
        .last()
        .map_or(0, |(end, _)| *end)
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlaybackState {
    pub requested: bool,
    pub recording_paused: bool,
    pub recording_active: bool,
    recording_started: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordingCue {
    None,
    StartFromTop,
}

impl PlaybackState {
    pub fn running(&self) -> bool {
        self.requested && !self.recording_paused
    }

    pub fn update_recording(
        &mut self,
        phase: Phase,
        uncertain: bool,
        start_with_recording: bool,
    ) -> RecordingCue {
        if phase == Phase::Stopping || (phase == Phase::Idle && self.recording_active) {
            self.requested = false;
        }
        let mut cue = RecordingCue::None;
        if phase == Phase::Idle {
            self.recording_started = false;
        } else if matches!(phase, Phase::Recording { paused: false }) && !self.recording_started {
            self.recording_started = true;
            if start_with_recording && !self.requested {
                self.requested = true;
                cue = RecordingCue::StartFromTop;
            }
        }
        self.recording_active = phase != Phase::Idle;
        self.recording_paused =
            matches!(phase, Phase::Recording { paused: true } | Phase::Stopping) || uncertain;
        cue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORK: [f32; 4] = [0., 0., 1280., 680.];

    #[test]
    fn the_strip_opens_attached_to_the_top_of_the_cursor_display_centred() {
        assert_eq!(opening_frame(None, WORK), [310., 0., 660., 156.]);
        assert_eq!(
            opening_frame(None, [-1920., -200., 1920., 1040.]),
            [-1290., -200., 660., 156.]
        );
    }

    #[test]
    fn a_remembered_frame_on_this_display_is_kept_and_one_elsewhere_keeps_only_its_size() {
        assert_eq!(
            opening_frame(Some([100., 240., 720., 200.]), WORK),
            [100., 240., 720., 200.]
        );
        assert_eq!(
            opening_frame(Some([2400., 30., 720., 200.]), WORK),
            [280., 0., 720., 200.]
        );
        assert_eq!(
            opening_frame(Some([0., 0., 100., 40.]), WORK),
            [0., 0., 420., 112.]
        );
    }

    #[test]
    fn dragging_snaps_back_to_the_top_within_18_px_and_ctrl_keeps_it_free() {
        assert_eq!(
            strip_frame([100., 18., 660., 156.], WORK, true),
            [100., 0., 660., 156.]
        );
        assert_eq!(
            strip_frame([100., 19., 660., 156.], WORK, true),
            [100., 19., 660., 156.]
        );
        assert_eq!(
            strip_frame([100., 12., 660., 156.], WORK, false),
            [100., 12., 660., 156.]
        );
        assert_eq!(
            strip_frame([322., 300., 660., 156.], WORK, true),
            [310., 300., 660., 156.]
        );
        assert_eq!(
            strip_frame([5000., -40., 660., 156.], WORK, true),
            [620., 0., 660., 156.]
        );
        assert!(attached_to_top([310., 0., 660., 156.], WORK));
        assert!(!attached_to_top([310., 4., 660., 156.], WORK));
    }

    #[test]
    fn the_editing_view_opens_to_660_by_380_from_where_the_strip_is() {
        assert_eq!(
            edit_frame([310., 0., 660., 156.], WORK),
            [310., 0., 660., 380.]
        );
        assert_eq!(
            edit_frame([310., 500., 800., 156.], WORK),
            [310., 300., 800., 380.]
        );
    }

    #[test]
    fn resizing_keeps_the_minimum_and_the_display() {
        let start = [310., 0., 660., 156.];
        assert_eq!(
            resize_frame(start, Edge::BottomRight, (40., 30.), WORK),
            [310., 0., 700., 186.]
        );
        assert_eq!(
            resize_frame(start, Edge::Left, (-400., 0.), WORK),
            [0., 0., 970., 156.]
        );
        assert_eq!(
            resize_frame(start, Edge::Left, (500., 0.), WORK),
            [550., 0., 420., 156.]
        );
        assert_eq!(
            resize_frame(start, Edge::Bottom, (0., -200.), WORK),
            [310., 0., 660., 112.]
        );
        assert_eq!(
            resize_frame(start, Edge::Right, (1000., 0.), WORK),
            [310., 0., 970., 156.]
        );
    }

    fn inputs() -> CaptureInputs {
        CaptureInputs {
            phase: Phase::Idle,
            pause_uncertain: false,
            hide_from_sharing: true,
            hiding_requested: true,
            hidden_confirmed: Some(true),
            streamed: false,
        }
    }

    #[test]
    fn the_status_says_what_is_true_now() {
        assert_eq!(
            capture_status(inputs()),
            CaptureStatus::HiddenFromRecordingsAndSharing
        );
        assert_eq!(
            capture_status(CaptureInputs {
                hide_from_sharing: false,
                hiding_requested: false,
                hidden_confirmed: Some(false),
                ..inputs()
            }),
            CaptureStatus::HiddenFromRecordings
        );
        assert_eq!(
            capture_status(CaptureInputs {
                phase: Phase::Recording { paused: false },
                ..inputs()
            }),
            CaptureStatus::HiddenFromThisRecording
        );
        assert_eq!(
            capture_status(CaptureInputs {
                phase: Phase::Recording { paused: true },
                ..inputs()
            }),
            CaptureStatus::PausedWithRecording
        );
        assert!(CaptureStatus::PausedWithRecording.warns());
        assert!(!CaptureStatus::HiddenFromThisRecording.warns());
    }

    #[test]
    fn remote_desktop_or_a_refused_affinity_says_it_cant_hide_in_amber() {
        let streamed = capture_status(CaptureInputs {
            phase: Phase::Recording { paused: false },
            streamed: true,
            ..inputs()
        });
        assert_eq!(streamed, CaptureStatus::CantHide);
        assert!(streamed.warns());
        assert_eq!(
            capture_status(CaptureInputs {
                streamed: true,
                hide_from_sharing: false,
                hiding_requested: false,
                ..inputs()
            }),
            CaptureStatus::CantHide
        );
        assert_eq!(
            capture_status(CaptureInputs {
                phase: Phase::Recording { paused: true },
                hidden_confirmed: Some(false),
                ..inputs()
            }),
            CaptureStatus::CantHide
        );
        assert_eq!(
            capture_status(CaptureInputs {
                hidden_confirmed: None,
                ..inputs()
            }),
            CaptureStatus::HiddenFromRecordingsAndSharing
        );
    }

    #[test]
    fn phrases_end_at_punctuation_in_both_languages() {
        let english = "Hi, I'm Lin. Open FrameEver";
        assert_eq!(phrase_ends(english), vec![4, 13, english.len()]);
        let chinese = "大家好，我是林。";
        assert_eq!(phrase_ends(chinese), vec![12, 24]);
        assert_eq!(phrase_ends("one\ntwo"), vec![4, 7]);
        assert!(phrase_ends("  ").is_empty());
    }

    #[test]
    fn passed_text_is_every_phrase_above_the_reading_band() {
        let at_start = [(4, 31.), (13, 69.), (27, 107.)];
        assert_eq!(passed_phrases(&at_start, 38., 31.), 0);
        let one_line_on = [(4, -7.), (13, 31.), (27, 69.)];
        assert_eq!(passed_phrases(&one_line_on, 38., 31.), 4);
        let at_end = [(4, -83.), (13, -45.), (27, -7.)];
        assert_eq!(passed_phrases(&at_end, 38., 31.), 27);
    }

    #[test]
    fn time_left_counts_down_with_the_scroll() {
        assert_eq!(total_seconds(260, 260), 60);
        assert_eq!(total_seconds(75, 150), 30);
        assert_eq!(seconds_left(60, 0., 600.), 60);
        assert_eq!(seconds_left(60, 300., 600.), 30);
        assert_eq!(seconds_left(60, 900., 600.), 0);
        assert_eq!(format_clock(65), "1:05");
        assert_eq!(format_clock(0), "0:00");
    }

    #[test]
    fn speed_and_text_size_step_inside_their_ranges() {
        assert_eq!(step_speed(150, 1, false), 160);
        assert_eq!(step_speed(345, 1, false), 350);
        assert_eq!(step_speed(415, 1, true), 420);
        assert_eq!(step_speed(65, -1, true), 60);
        assert_eq!(step_font(28., 1.), 30.);
        assert_eq!(step_font(52., 1.), 52.);
        assert_eq!(step_font(22., -1.), 22.);
        assert_eq!(nearest_line_spacing(1.5), 1.6);
        assert_eq!(nearest_line_spacing(1.3), 1.35);
    }

    #[test]
    fn the_reading_line_moves_inside_the_strip() {
        assert_eq!(band_top(156., 0.2), 31.);
        assert_eq!(band_top(156., 2.), 125.);
        assert_eq!(reading_line_for(78., 156.), 0.5);
        assert_eq!(reading_line_for(0., 156.), 0.08);
    }

    #[test]
    fn scrolling_while_you_speak_follows_the_microphone_with_a_short_hold() {
        let mut gate = VoiceGate::default();
        assert!(!gate.update(-70., 0.016));
        assert!(gate.update(-30., 0.016));
        assert!(gate.update(-70., 0.5));
        assert!(!gate.update(-70., 0.3));
        gate.reset();
        assert!(!gate.update(-96., 0.016));
    }

    #[test]
    fn reading_keys_match_the_design_and_leave_modified_keys_alone() {
        assert_eq!(
            reading_key("space", false, false),
            Some(ReadingKey::TogglePlay)
        );
        assert_eq!(reading_key("up", false, false), Some(ReadingKey::Faster));
        assert_eq!(reading_key("down", false, false), Some(ReadingKey::Slower));
        assert_eq!(reading_key("=", false, false), Some(ReadingKey::Larger));
        assert_eq!(reading_key("-", false, false), Some(ReadingKey::Smaller));
        assert_eq!(
            reading_key("pagedown", false, false),
            Some(ReadingKey::JumpDown)
        );
        assert_eq!(
            reading_key("pageup", false, false),
            Some(ReadingKey::JumpUp)
        );
        assert_eq!(reading_key("home", false, false), Some(ReadingKey::Top));
        assert_eq!(reading_key("e", false, false), Some(ReadingKey::Edit));
        assert_eq!(reading_key("space", true, false), None);
        assert_eq!(jump(10., 3., 38., 500.), 124.);
        assert_eq!(jump(10., -3., 38., 500.), 0.);
    }

    #[test]
    fn recording_pause_preserves_manual_playback_intent() {
        let mut playback = PlaybackState {
            requested: true,
            ..Default::default()
        };
        playback.update_recording(Phase::Recording { paused: false }, false, false);
        assert!(playback.running());
        playback.update_recording(Phase::Recording { paused: true }, false, false);
        assert!(!playback.running());
        assert!(playback.requested);
        playback.update_recording(Phase::Recording { paused: false }, false, false);
        assert!(playback.running());
    }

    #[test]
    fn play_cannot_override_recording_pause_or_unconfirmed_capture_state() {
        for uncertain in [false, true] {
            let mut playback = PlaybackState::default();
            playback.update_recording(Phase::Recording { paused: !uncertain }, uncertain, false);
            playback.requested = true;
            assert!(!playback.running());
        }
    }

    #[test]
    fn recording_stop_clears_automatic_scroll_resume() {
        for terminal in [Phase::Stopping, Phase::Idle] {
            let mut playback = PlaybackState {
                requested: true,
                ..Default::default()
            };
            playback.update_recording(Phase::Recording { paused: true }, false, false);
            playback.update_recording(terminal, false, false);
            playback.update_recording(Phase::Recording { paused: false }, false, false);
            assert!(!playback.running());
        }
    }

    #[test]
    fn start_with_recording_waits_for_the_countdown_and_fires_once_per_take() {
        let mut playback = PlaybackState::default();
        assert_eq!(
            playback.update_recording(Phase::Starting, false, true),
            RecordingCue::None
        );
        assert!(!playback.running());
        assert_eq!(
            playback.update_recording(Phase::Recording { paused: false }, false, true),
            RecordingCue::StartFromTop
        );
        assert!(playback.running());
        playback.requested = false;
        playback.update_recording(Phase::Recording { paused: true }, false, true);
        assert_eq!(
            playback.update_recording(Phase::Recording { paused: false }, false, true),
            RecordingCue::None
        );
        assert!(!playback.running());
        playback.update_recording(Phase::Idle, false, true);
        playback.update_recording(Phase::Starting, false, true);
        assert_eq!(
            playback.update_recording(Phase::Recording { paused: false }, false, true),
            RecordingCue::StartFromTop
        );
    }

    #[test]
    fn start_with_recording_off_leaves_playback_to_the_reader() {
        let mut playback = PlaybackState::default();
        playback.update_recording(Phase::Starting, false, false);
        assert_eq!(
            playback.update_recording(Phase::Recording { paused: false }, false, false),
            RecordingCue::None
        );
        assert!(!playback.running());
    }
}
