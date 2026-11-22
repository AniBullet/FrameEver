pub(crate) mod cuts;
pub(crate) mod energy;
pub(crate) mod model;
#[cfg(test)]
mod tests;
mod view;
pub(crate) mod word_edit;

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use frameever_project::filler_words::FillerKind;
use gpui::{FocusHandle, ListAlignment, ListOffset, ListState, Pixels, Point, TextLayout, px};

use self::energy::EnergyEnvelope;
use self::model::{TranscriptDoc, TranscriptEdl};

const STORE_SECTION: &str = "editorTranscript";
const PAUSE_LONGER_KEY: &str = "pauseLongerThan";
const PAUSE_KEEP_KEY: &str = "pauseKeep";

pub(crate) const DEFAULT_PAUSE_LONGER_THAN: f64 = 1.0;
pub(crate) const DEFAULT_PAUSE_KEEP: f64 = 0.5;
pub(crate) const PAUSE_LONGER_RANGE: (f64, f64) = (0.3, 5.0);
pub(crate) const PAUSE_KEEP_RANGE: (f64, f64) = (0.1, 2.0);

pub(crate) const FOLLOW_PAUSE: std::time::Duration = std::time::Duration::from_secs(4);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WordSelection {
    pub anchor: usize,
    pub focus: usize,
}

impl WordSelection {
    pub fn single(word: usize) -> Self {
        Self {
            anchor: word,
            focus: word,
        }
    }

    pub fn range(self) -> Range<usize> {
        self.anchor.min(self.focus)..self.anchor.max(self.focus) + 1
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct WordEditor {
    pub word: usize,
    pub text: String,
    pub start: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WordAnchor {
    pub origin: Point<Pixels>,
    pub width: Pixels,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DragUnit {
    Word,
    Sentence,
    Paragraph,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct WordDrag {
    pub unit: DragUnit,
    pub origin: (usize, usize),
}

pub(crate) struct SearchState {
    pub input: gpui::Entity<crate::ui::TextInputState>,
    pub matches: Arc<Vec<Range<usize>>>,
    pub current: usize,
    pub query: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CleanupTab {
    Fillers,
    Pauses,
}

pub(crate) struct CleanupState {
    pub open: bool,
    pub tab: CleanupTab,
    pub categories: [bool; 4],
    pub ignored: HashSet<usize>,
    pub pause_longer_than: f64,
    pub pause_keep: f64,
    pub filler_rows: gpui::UniformListScrollHandle,
    pub pause_rows: gpui::UniformListScrollHandle,
}

impl CleanupState {
    fn new() -> Self {
        let section = crate::store::store_section(STORE_SECTION);
        let read = |key: &str, default: f64, (min, max): (f64, f64)| {
            section
                .get(key)
                .and_then(serde_json::Value::as_f64)
                .filter(|value| value.is_finite())
                .map_or(default, |value| value.clamp(min, max))
        };
        Self {
            open: false,
            tab: CleanupTab::Fillers,
            categories: [true, false, true, true],
            ignored: HashSet::new(),
            pause_longer_than: read(
                PAUSE_LONGER_KEY,
                DEFAULT_PAUSE_LONGER_THAN,
                PAUSE_LONGER_RANGE,
            ),
            pause_keep: read(PAUSE_KEEP_KEY, DEFAULT_PAUSE_KEEP, PAUSE_KEEP_RANGE),
            filler_rows: gpui::UniformListScrollHandle::new(),
            pause_rows: gpui::UniformListScrollHandle::new(),
        }
    }

    pub fn category_enabled(&self, kind: FillerKind) -> bool {
        self.categories[category_index(kind)]
    }

    pub fn persist_pauses(&self) {
        let longer = crate::store::set_store_setting(
            STORE_SECTION,
            PAUSE_LONGER_KEY,
            serde_json::json!(self.pause_longer_than),
        );
        let keep = crate::store::set_store_setting(
            STORE_SECTION,
            PAUSE_KEEP_KEY,
            serde_json::json!(self.pause_keep),
        );
        if !(longer && keep) {
            tracing::warn!("failed to persist transcript pause settings");
        }
    }
}

pub(crate) fn category_index(kind: FillerKind) -> usize {
    match kind {
        FillerKind::Hesitation => 0,
        FillerKind::FillerPhrase => 1,
        FillerKind::RepeatedWord => 2,
        FillerKind::FalseStart => 3,
    }
}

pub(crate) struct TranscriptAudio {
    pub mic: Vec<Option<Arc<frameever_audio::DecodedAudio>>>,
    pub system: Vec<Option<Arc<frameever_audio::DecodedAudio>>>,
}

pub(crate) struct TranscriptState {
    pub open: bool,
    pub doc: Option<Arc<TranscriptDoc>>,
    pub edl: Option<Arc<TranscriptEdl>>,
    pub dirty: bool,
    edl_key: u64,
    pub list: ListState,
    pub selection: Option<WordSelection>,
    pub drag: Option<WordDrag>,
    pub active: Option<usize>,
    pub hovered_cut: Option<usize>,
    pub user_scrolled: Option<Instant>,
    pub search: Option<SearchState>,
    pub cleanup: CleanupState,
    pub focus: FocusHandle,
    pub layouts: Rc<RefCell<HashMap<usize, TextLayout>>>,
    pub audio: Option<Arc<TranscriptAudio>>,
    pub energy: Option<Arc<EnergyEnvelope>>,
    pub energy_loading: bool,
    pub energy_offsets: Vec<(f64, f64)>,
    pub voiced_for: usize,
    pub notice: Option<String>,
    pub editing: Option<WordEditor>,
    pub edit_input: Option<gpui::Entity<crate::ui::TextInputState>>,
    pub edit_anchor: Rc<Cell<Option<WordAnchor>>>,
    pub restored_on_press: bool,
}

impl TranscriptState {
    pub fn new(focus: FocusHandle) -> Self {
        Self {
            open: false,
            doc: None,
            edl: None,
            dirty: true,
            edl_key: 0,
            list: ListState::new(0, ListAlignment::Top, px(600.)),
            selection: None,
            drag: None,
            active: None,
            hovered_cut: None,
            user_scrolled: None,
            search: None,
            cleanup: CleanupState::new(),
            focus,
            layouts: Rc::new(RefCell::new(HashMap::new())),
            audio: None,
            energy: None,
            energy_loading: false,
            energy_offsets: Vec::new(),
            voiced_for: 0,
            notice: None,
            editing: None,
            edit_input: None,
            edit_anchor: Rc::new(Cell::new(None)),
            restored_on_press: false,
        }
    }

    pub fn refresh(
        &mut self,
        project: &frameever_project::ProjectConfiguration,
        recording_durations: &[f64],
    ) -> bool {
        if !self.dirty && self.doc.is_some() == has_words(project) {
            return false;
        }
        self.dirty = false;
        let Some(captions) = project.captions.as_ref().filter(|_| has_words(project)) else {
            let changed = self.doc.is_some();
            self.doc = None;
            self.editing = None;
            self.edl = None;
            self.list.reset(0);
            self.selection = None;
            self.active = None;
            return changed;
        };
        let mut changed = false;
        let signature = model::captions_signature(Some(captions));
        if self
            .doc
            .as_ref()
            .is_none_or(|doc| doc.signature != signature)
        {
            let doc = TranscriptDoc::build(captions);
            if let Some(editing) = self.editing.as_mut() {
                match word_edit::locate(&doc.words, editing.word, &editing.text, editing.start) {
                    Some(word) => {
                        editing.word = word;
                        editing.start = doc.words[word].start;
                    }
                    None => self.editing = None,
                }
            }
            let scroll = self.doc.is_some().then(|| self.list.logical_scroll_top());
            self.list.reset(doc.paragraphs.len());
            if let Some(scroll) = scroll.filter(|_| !doc.paragraphs.is_empty()) {
                self.list.scroll_to(ListOffset {
                    item_ix: scroll.item_ix.min(doc.paragraphs.len() - 1),
                    offset_in_item: scroll.offset_in_item,
                });
            }
            self.layouts.borrow_mut().clear();
            self.selection = None;
            self.active = None;
            self.hovered_cut = None;
            self.cleanup.ignored.clear();
            self.doc = Some(Arc::new(doc));
            self.edl_key = 0;
            self.voiced_for = 0;
            changed = true;
        }
        let energy_key = self
            .energy
            .as_ref()
            .map_or(1, |energy| Arc::as_ptr(energy) as usize);
        if energy_key != self.voiced_for
            && let Some(doc) = self.doc.as_mut()
        {
            Arc::make_mut(doc).refine_voiced(self.energy.as_deref());
            self.voiced_for = energy_key;
            changed = true;
        }
        let key = timeline_key(project.timeline.as_ref(), recording_durations);
        if changed || key != self.edl_key || self.edl.is_none() {
            self.edl_key = key;
            if let Some(doc) = self.doc.as_ref() {
                self.edl = project.timeline.as_ref().map(|timeline| {
                    Arc::new(TranscriptEdl::build(doc, timeline, recording_durations))
                });
            }
            self.hovered_cut = None;
            changed = true;
        }
        if let (Some(search), Some(doc)) = (self.search.as_mut(), self.doc.as_ref())
            && changed
        {
            search.matches = Arc::new(view::find_matches(doc, &search.query));
            search.current = search.current.min(search.matches.len().saturating_sub(1));
        }
        changed
    }
}

pub(crate) fn has_words(project: &frameever_project::ProjectConfiguration) -> bool {
    project.captions.as_ref().is_some_and(|captions| {
        captions
            .segments
            .iter()
            .any(|segment| !segment.words.is_empty())
    })
}

fn timeline_key(
    timeline: Option<&frameever_project::TimelineConfiguration>,
    recording_durations: &[f64],
) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::hash::DefaultHasher::new();
    for duration in recording_durations {
        duration.to_bits().hash(&mut hasher);
    }
    if let Some(timeline) = timeline {
        for segment in &timeline.segments {
            segment.recording_clip.hash(&mut hasher);
            segment.start.to_bits().hash(&mut hasher);
            segment.end.to_bits().hash(&mut hasher);
            segment.timescale.to_bits().hash(&mut hasher);
        }
        for transition in &timeline.transitions {
            transition.segment_index.hash(&mut hasher);
            transition.duration.to_bits().hash(&mut hasher);
        }
        for (start, end) in timeline.hold_windows() {
            start.to_bits().hash(&mut hasher);
            end.to_bits().hash(&mut hasher);
        }
    } else {
        1u8.hash(&mut hasher);
    }
    hasher.finish()
}
