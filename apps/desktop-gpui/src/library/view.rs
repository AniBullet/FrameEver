use std::{
    cmp::Ordering,
    path::{Path, PathBuf},
};

use chrono::{DateTime, Datelike, Days, FixedOffset, Local, NaiveDate};
use serde::{Deserialize, Serialize};

use super::MediaKind;

pub const TILE_MIN: f32 = 120.;
pub const TILE_MAX: f32 = 280.;
pub const TILE_DEFAULT: f32 = 150.;
pub const TILE_STEP: f32 = 20.;
pub const TILE_GAP: f32 = 8.;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Tab {
    #[default]
    All,
    Studio,
    Instant,
    Screenshots,
}

impl Tab {
    pub const ALL: [Self; 4] = [Self::All, Self::Studio, Self::Instant, Self::Screenshots];

    fn holds(self, kind: MediaKind) -> bool {
        match self {
            Self::All => true,
            Self::Studio => kind == MediaKind::Studio,
            Self::Instant => kind == MediaKind::Instant,
            Self::Screenshots => kind == MediaKind::Screenshot,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StatusFilter {
    Exported,
    NotExported,
    CompressedCopy,
    CompressedOnly,
}

impl StatusFilter {
    pub const ALL: [Self; 4] = [
        Self::Exported,
        Self::NotExported,
        Self::CompressedCopy,
        Self::CompressedOnly,
    ];

    fn matches(self, entry: &Entry) -> bool {
        match self {
            Self::Exported => entry.exported(),
            Self::NotExported => entry.kind != MediaKind::Screenshot && !entry.exported(),
            Self::CompressedCopy => entry.compression != Compression::None,
            Self::CompressedOnly => entry.compression == Compression::CompressedOnly,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SortKey {
    #[default]
    Date,
    Name,
    Size,
    Duration,
}

impl SortDirection {
    pub fn reversed(self) -> Self {
        match self {
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Ascending,
        }
    }
}

impl SortKey {
    pub const ALL: [Self; 4] = [Self::Date, Self::Name, Self::Size, Self::Duration];

    pub fn first_direction(self) -> SortDirection {
        match self {
            Self::Name => SortDirection::Ascending,
            Self::Date | Self::Size | Self::Duration => SortDirection::Descending,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SortDirection {
    Ascending,
    #[default]
    Descending,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ViewMode {
    #[default]
    Grid,
    List,
}

fn default_tile_size() -> f32 {
    TILE_DEFAULT
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewState {
    #[serde(default)]
    pub tab: Tab,
    #[serde(default)]
    pub statuses: Vec<StatusFilter>,
    #[serde(default)]
    pub sort: SortKey,
    #[serde(default)]
    pub direction: SortDirection,
    #[serde(default = "default_tile_size")]
    pub tile_size: f32,
    #[serde(default)]
    pub view: ViewMode,
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            tab: Tab::All,
            statuses: Vec::new(),
            sort: SortKey::Date,
            direction: SortDirection::Descending,
            tile_size: TILE_DEFAULT,
            view: ViewMode::Grid,
        }
    }
}

impl ViewState {
    pub fn toggle_status(&mut self, status: StatusFilter) {
        if let Some(index) = self.statuses.iter().position(|held| *held == status) {
            self.statuses.remove(index);
        } else {
            self.statuses.push(status);
        }
    }

    pub fn clear_filters(&mut self) {
        self.statuses.clear();
        self.tab = Tab::All;
    }

    pub fn set_sort(&mut self, sort: SortKey) {
        self.sort = sort;
        self.direction = sort.first_direction();
    }

    pub fn sort_by_column(&mut self, sort: SortKey) {
        if self.sort == sort {
            self.direction = self.direction.reversed();
        } else {
            self.set_sort(sort);
        }
    }

    pub fn set_tile_size(&mut self, size: f32) {
        self.tile_size = size.clamp(TILE_MIN, TILE_MAX);
    }

    pub fn tile_fraction(&self) -> f32 {
        (self.tile_size - TILE_MIN) / (TILE_MAX - TILE_MIN)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Compression {
    #[default]
    None,
    CompressedCopy,
    CompressedOnly,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExportFile {
    pub path: PathBuf,
    pub name: String,
    pub exported_millis: f64,
    pub size: Option<u64>,
}

impl ExportFile {
    pub fn found(&self) -> bool {
        self.size.is_some()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub path: PathBuf,
    pub kind: MediaKind,
    pub name: String,
    pub size: u64,
    pub duration: Option<f64>,
    pub created_millis: f64,
    pub exports: Vec<ExportFile>,
    pub compression: Compression,
    pub compressible: bool,
    pub clip_count: u32,
    pub thumbnail: Option<PathBuf>,
    pub thumbnail_version: Option<u128>,
    pub screenshot: Option<PathBuf>,
    pub opens_editor: bool,
}

impl Entry {
    pub fn exported(&self) -> bool {
        self.exports.iter().any(ExportFile::found)
    }

    pub fn latest_export(&self) -> Option<&ExportFile> {
        self.exports
            .iter()
            .filter(|export| export.found())
            .max_by(|a, b| a.exported_millis.total_cmp(&b.exported_millis))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateGroup {
    Today,
    Yesterday,
    ThisWeek,
    Month { year: i32, month: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DayLabel {
    Today,
    Yesterday,
    Date(NaiveDate),
}

#[derive(Debug, Clone, Copy)]
pub struct Clock {
    pub today: NaiveDate,
    pub offset: FixedOffset,
}

impl Clock {
    pub fn now() -> Self {
        let now = Local::now();
        Self {
            today: now.date_naive(),
            offset: *now.offset(),
        }
    }

    fn date_of(&self, millis: f64) -> NaiveDate {
        DateTime::from_timestamp_millis(millis as i64).map_or(self.today, |time| {
            time.with_timezone(&self.offset).date_naive()
        })
    }

    pub fn day_of(&self, millis: f64) -> DayLabel {
        let date = self.date_of(millis);
        if date == self.today {
            DayLabel::Today
        } else if self.today.checked_sub_days(Days::new(1)) == Some(date) {
            DayLabel::Yesterday
        } else {
            DayLabel::Date(date)
        }
    }

    pub fn time_of(&self, millis: f64) -> String {
        DateTime::from_timestamp_millis(millis as i64).map_or_else(String::new, |time| {
            time.with_timezone(&self.offset).format("%H:%M").to_string()
        })
    }

    fn group_of(&self, millis: f64) -> DateGroup {
        let date = self.date_of(millis);
        let week_start = self
            .today
            .checked_sub_days(Days::new(
                self.today.weekday().num_days_from_monday().into(),
            ))
            .unwrap_or(self.today);
        if date >= self.today {
            DateGroup::Today
        } else if self.today.checked_sub_days(Days::new(1)) == Some(date) {
            DateGroup::Yesterday
        } else if date >= week_start {
            DateGroup::ThisWeek
        } else {
            DateGroup::Month {
                year: date.year(),
                month: date.month(),
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub group: Option<DateGroup>,
    pub items: Vec<usize>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LibraryView {
    pub total: usize,
    pub tab_counts: [usize; 4],
    pub status_counts: [usize; 4],
    pub sections: Vec<Section>,
    pub shown: usize,
    pub shown_size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    Empty,
    NoMatch,
    Items,
}

impl LibraryView {
    pub fn surface(&self, importing: bool) -> Surface {
        if self.shown > 0 || importing {
            Surface::Items
        } else if self.total == 0 {
            Surface::Empty
        } else {
            Surface::NoMatch
        }
    }

    pub fn tab_count(&self, tab: Tab) -> usize {
        Tab::ALL
            .iter()
            .position(|held| *held == tab)
            .map_or(0, |index| self.tab_counts[index])
    }

    pub fn status_count(&self, status: StatusFilter) -> usize {
        StatusFilter::ALL
            .iter()
            .position(|held| *held == status)
            .map_or(0, |index| self.status_counts[index])
    }

    pub fn order(&self, entries: &[Entry]) -> Vec<PathBuf> {
        self.sections
            .iter()
            .flat_map(|section| &section.items)
            .filter_map(|index| entries.get(*index))
            .map(|entry| entry.path.clone())
            .collect()
    }
}

pub fn build(
    entries: &[Entry],
    state: &ViewState,
    query: &str,
    fresh: &[PathBuf],
    clock: &Clock,
) -> LibraryView {
    let needle = query.trim().to_lowercase();
    let mut items: Vec<usize> = (0..entries.len())
        .filter(|index| {
            let entry = &entries[*index];
            state.tab.holds(entry.kind)
                && (state.statuses.is_empty()
                    || state.statuses.iter().any(|status| status.matches(entry)))
                && matches_search(entry, &needle)
        })
        .collect();
    items.sort_by(|a, b| {
        let (a, b) = (&entries[*a], &entries[*b]);
        is_fresh(fresh, &b.path)
            .cmp(&is_fresh(fresh, &a.path))
            .then_with(|| match state.direction {
                SortDirection::Ascending => compare(a, b, state.sort),
                SortDirection::Descending => compare(b, a, state.sort),
            })
            .then_with(|| b.created_millis.total_cmp(&a.created_millis))
            .then_with(|| a.path.cmp(&b.path))
    });

    let groups = state.sort == SortKey::Date && state.view == ViewMode::Grid;
    let mut sections: Vec<Section> = Vec::new();
    for index in items.iter().copied() {
        let group = groups.then(|| clock.group_of(entries[index].created_millis));
        match sections.iter_mut().find(|section| section.group == group) {
            Some(section) => section.items.push(index),
            None => sections.push(Section {
                group,
                items: vec![index],
            }),
        }
    }

    LibraryView {
        total: entries.len(),
        tab_counts: Tab::ALL.map(|tab| entries.iter().filter(|e| tab.holds(e.kind)).count()),
        status_counts: StatusFilter::ALL
            .map(|status| entries.iter().filter(|e| status.matches(e)).count()),
        sections,
        shown: items.len(),
        shown_size: items.iter().map(|index| entries[*index].size).sum(),
    }
}

fn matches_search(entry: &Entry, needle: &str) -> bool {
    needle.is_empty()
        || entry.name.to_lowercase().contains(needle)
        || entry
            .exports
            .iter()
            .any(|export| export.name.to_lowercase().contains(needle))
}

fn compare(a: &Entry, b: &Entry, sort: SortKey) -> Ordering {
    match sort {
        SortKey::Date => a.created_millis.total_cmp(&b.created_millis),
        SortKey::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        SortKey::Size => a.size.cmp(&b.size),
        SortKey::Duration => a
            .duration
            .unwrap_or_default()
            .total_cmp(&b.duration.unwrap_or_default()),
    }
}

pub fn columns(width: f32, tile_size: f32) -> usize {
    (((width + TILE_GAP) / (tile_size + TILE_GAP)).floor() as usize).max(1)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Row {
    FirstLine,
    Header(DateGroup),
    Imports(Vec<usize>),
    Tiles(Vec<usize>),
    Item(usize),
}

impl Row {
    fn items(&self) -> &[usize] {
        match self {
            Self::Tiles(items) => items,
            Self::Item(item) => std::slice::from_ref(item),
            _ => &[],
        }
    }
}

pub fn rows(
    view: &LibraryView,
    mode: ViewMode,
    columns: usize,
    first_line: bool,
    imports: &[usize],
) -> Vec<Row> {
    let columns = columns.max(1);
    let mut rows = Vec::new();
    if first_line {
        rows.push(Row::FirstLine);
    }
    rows.extend(
        imports
            .chunks(columns)
            .map(|chunk| Row::Imports(chunk.to_vec())),
    );
    for section in &view.sections {
        rows.extend(section.group.map(Row::Header));
        match mode {
            ViewMode::Grid => rows.extend(
                section
                    .items
                    .chunks(columns)
                    .map(|chunk| Row::Tiles(chunk.to_vec())),
            ),
            ViewMode::List => rows.extend(section.items.iter().copied().map(Row::Item)),
        }
    }
    rows
}

pub fn row_of(rows: &[Row], item: usize) -> Option<usize> {
    rows.iter().position(|row| row.items().contains(&item))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Left,
    Right,
    Up,
    Down,
}

pub fn step_from(rows: &[Row], from: Option<usize>, step: Step) -> Option<usize> {
    let lines: Vec<&[usize]> = rows
        .iter()
        .map(Row::items)
        .filter(|items| !items.is_empty())
        .collect();
    let flat: Vec<usize> = lines
        .iter()
        .flat_map(|items| items.iter().copied())
        .collect();
    let Some(from) = from.filter(|from| flat.contains(from)) else {
        return flat.first().copied();
    };
    let line = lines.iter().position(|items| items.contains(&from))?;
    let column = lines[line].iter().position(|item| *item == from)?;
    let beside = |line: usize| lines[line][column.min(lines[line].len() - 1)];
    let at = flat.iter().position(|item| *item == from)?;
    Some(match step {
        Step::Left => flat[at.saturating_sub(1)],
        Step::Right => flat[(at + 1).min(flat.len() - 1)],
        Step::Up => beside(line.saturating_sub(1)),
        Step::Down => beside((line + 1).min(lines.len() - 1)),
    })
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Selection {
    items: Vec<PathBuf>,
    anchor: Option<PathBuf>,
    lead: Option<PathBuf>,
}

impl Selection {
    pub fn items(&self) -> &[PathBuf] {
        &self.items
    }

    pub fn lead(&self) -> Option<&Path> {
        self.lead.as_deref()
    }

    pub fn single(&self) -> Option<&Path> {
        match self.items.as_slice() {
            [only] => Some(only),
            _ => None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn contains(&self, path: &Path) -> bool {
        self.items.iter().any(|held| held == path)
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn click(&mut self, path: &Path) {
        self.items = vec![path.to_path_buf()];
        self.anchor = Some(path.to_path_buf());
        self.lead = Some(path.to_path_buf());
    }

    pub fn toggle(&mut self, path: &Path) {
        self.anchor = Some(path.to_path_buf());
        if self.contains(path) {
            self.items.retain(|held| held != path);
            self.lead = self.items.last().cloned();
        } else {
            self.items.push(path.to_path_buf());
            self.lead = Some(path.to_path_buf());
        }
    }

    pub fn extend_to(&mut self, path: &Path, order: &[PathBuf]) {
        let anchor = self.anchor.clone().unwrap_or_else(|| path.to_path_buf());
        let at = |target: &Path| order.iter().position(|held| held == target);
        let (Some(start), Some(end)) = (at(&anchor), at(path)) else {
            return self.click(path);
        };
        self.items = order[start.min(end)..=start.max(end)].to_vec();
        self.anchor = Some(anchor);
        self.lead = Some(path.to_path_buf());
    }

    pub fn context_click(&mut self, path: &Path) {
        if self.contains(path) {
            self.lead = Some(path.to_path_buf());
        } else {
            self.click(path);
        }
    }

    pub fn select_all(&mut self, order: &[PathBuf]) {
        self.items = order.to_vec();
        self.anchor = order.first().cloned();
        self.lead = order.last().cloned();
    }

    pub fn marquee(&mut self, base: &[PathBuf], hits: &[PathBuf], order: &[PathBuf]) {
        self.items = order
            .iter()
            .filter(|path| base.contains(path) || hits.contains(path))
            .cloned()
            .collect();
        self.anchor = hits
            .first()
            .cloned()
            .or_else(|| self.items.first().cloned());
        self.lead = hits.last().cloned().or_else(|| self.items.last().cloned());
    }

    pub fn retain(&mut self, order: &[PathBuf]) {
        self.items.retain(|held| order.contains(held));
        let kept = |path: &Option<PathBuf>| path.as_ref().is_some_and(|held| order.contains(held));
        if !kept(&self.anchor) {
            self.anchor = self.items.first().cloned();
        }
        if !kept(&self.lead) {
            self.lead = self.items.last().cloned();
        }
    }
}

pub fn header_above(rows: &[Row], row: usize) -> Option<DateGroup> {
    rows.iter()
        .take(row.saturating_add(1))
        .rev()
        .find_map(|row| match row {
            Row::Header(group) => Some(*group),
            _ => None,
        })
}

pub fn is_fresh(fresh: &[PathBuf], path: &Path) -> bool {
    fresh.iter().any(|held| held == path)
}

pub fn format_size(bytes: u64) -> String {
    const KB: f64 = 1024.;
    const MB: f64 = KB * 1024.;
    const GB: f64 = MB * 1024.;
    let value = bytes as f64;
    if value >= GB {
        format!("{:.1} GB", value / GB)
    } else if value >= MB {
        format!("{} MB", (value / MB).round())
    } else {
        format!("{} KB", (value / KB).ceil())
    }
}

pub fn format_clock(seconds: f64) -> String {
    let total = seconds.max(0.) as u64;
    let (hours, minutes, seconds) = (total / 3600, total / 60 % 60, total % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: f64 = 86_400_000.;

    fn clock() -> Clock {
        Clock {
            today: NaiveDate::from_ymd_opt(2026, 11, 5).unwrap(),
            offset: FixedOffset::east_opt(8 * 3600).unwrap(),
        }
    }

    fn noon_on(day: u32, month: u32) -> f64 {
        NaiveDate::from_ymd_opt(2026, month, day)
            .unwrap()
            .and_hms_opt(4, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp_millis() as f64
    }

    fn entry(name: &str, kind: MediaKind, created_millis: f64) -> Entry {
        Entry {
            path: PathBuf::from(format!("{name}.ever")),
            kind,
            name: name.to_string(),
            size: 0,
            duration: None,
            created_millis,
            exports: Vec::new(),
            compression: Compression::None,
            compressible: false,
            clip_count: 1,
            thumbnail: None,
            thumbnail_version: None,
            screenshot: None,
            opens_editor: false,
        }
    }

    fn names(view: &LibraryView, entries: &[Entry]) -> Vec<String> {
        view.sections
            .iter()
            .flat_map(|section| &section.items)
            .map(|index| entries[*index].name.clone())
            .collect()
    }

    fn export(name: &str, exported_millis: f64, size: Option<u64>) -> ExportFile {
        ExportFile {
            path: PathBuf::from(name),
            name: name.to_string(),
            exported_millis,
            size,
        }
    }

    fn library() -> Vec<Entry> {
        vec![
            Entry {
                size: 300,
                duration: Some(100.),
                exports: vec![export("Launch final.mp4", 1., Some(10))],
                ..entry("demo", MediaKind::Studio, noon_on(5, 11))
            },
            Entry {
                size: 100,
                duration: Some(300.),
                compression: Compression::CompressedCopy,
                ..entry("Bug", MediaKind::Instant, noon_on(4, 11))
            },
            Entry {
                size: 200,
                compression: Compression::None,
                ..entry("shot", MediaKind::Screenshot, noon_on(3, 11))
            },
            Entry {
                size: 400,
                duration: Some(200.),
                compression: Compression::CompressedOnly,
                ..entry("Archive", MediaKind::Instant, noon_on(12, 10))
            },
        ]
    }

    #[test]
    fn date_order_is_newest_first_in_date_groups() {
        let entries = library();
        let view = build(&entries, &ViewState::default(), "", &[], &clock());
        let groups: Vec<_> = view.sections.iter().map(|section| section.group).collect();
        assert_eq!(
            groups,
            vec![
                Some(DateGroup::Today),
                Some(DateGroup::Yesterday),
                Some(DateGroup::ThisWeek),
                Some(DateGroup::Month {
                    year: 2026,
                    month: 10
                }),
            ]
        );
        assert_eq!(names(&view, &entries), ["demo", "Bug", "shot", "Archive"]);
    }

    #[test]
    fn groups_reverse_with_the_direction() {
        let entries = library();
        let state = ViewState {
            direction: SortDirection::Ascending,
            ..ViewState::default()
        };
        let view = build(&entries, &state, "", &[], &clock());
        assert_eq!(
            view.sections[0].group,
            Some(DateGroup::Month {
                year: 2026,
                month: 10
            })
        );
        assert_eq!(view.sections[3].group, Some(DateGroup::Today));
    }

    #[test]
    fn only_date_sorting_groups() {
        let entries = library();
        for sort in [SortKey::Name, SortKey::Size, SortKey::Duration] {
            let mut state = ViewState::default();
            state.set_sort(sort);
            let view = build(&entries, &state, "", &[], &clock());
            assert_eq!(view.sections.len(), 1);
            assert_eq!(view.sections[0].group, None);
        }
    }

    #[test]
    fn the_list_view_never_groups() {
        let entries = library();
        let state = ViewState {
            view: ViewMode::List,
            ..ViewState::default()
        };
        let view = build(&entries, &state, "", &[], &clock());
        assert_eq!(view.sections.len(), 1);
    }

    #[test]
    fn a_new_sort_starts_the_natural_way_round() {
        let mut state = ViewState {
            direction: SortDirection::Ascending,
            ..ViewState::default()
        };
        state.set_sort(SortKey::Size);
        assert_eq!(state.direction, SortDirection::Descending);
        state.set_sort(SortKey::Name);
        assert_eq!(state.direction, SortDirection::Ascending);
    }

    #[test]
    fn name_size_and_duration_sort_either_way() {
        let entries = library();
        let order = |sort, direction| {
            let state = ViewState {
                sort,
                direction,
                ..ViewState::default()
            };
            names(&build(&entries, &state, "", &[], &clock()), &entries)
        };
        assert_eq!(
            order(SortKey::Name, SortDirection::Ascending),
            ["Archive", "Bug", "demo", "shot"]
        );
        assert_eq!(
            order(SortKey::Size, SortDirection::Descending),
            ["Archive", "demo", "shot", "Bug"]
        );
        assert_eq!(
            order(SortKey::Size, SortDirection::Ascending),
            ["Bug", "shot", "demo", "Archive"]
        );
        assert_eq!(
            order(SortKey::Duration, SortDirection::Descending),
            ["Bug", "Archive", "demo", "shot"]
        );
    }

    #[test]
    fn tabs_narrow_the_list_and_keep_their_counts() {
        let entries = library();
        let state = ViewState {
            tab: Tab::Instant,
            ..ViewState::default()
        };
        let view = build(&entries, &state, "", &[], &clock());
        assert_eq!(names(&view, &entries), ["Bug", "Archive"]);
        assert_eq!(view.tab_count(Tab::All), 4);
        assert_eq!(view.tab_count(Tab::Studio), 1);
        assert_eq!(view.tab_count(Tab::Instant), 2);
        assert_eq!(view.tab_count(Tab::Screenshots), 1);
    }

    #[test]
    fn status_filters_accept_any_chosen_status() {
        let entries = library();
        let mut state = ViewState::default();
        state.toggle_status(StatusFilter::Exported);
        state.toggle_status(StatusFilter::CompressedOnly);
        let view = build(&entries, &state, "", &[], &clock());
        assert_eq!(names(&view, &entries), ["demo", "Archive"]);

        state.toggle_status(StatusFilter::Exported);
        assert_eq!(state.statuses, [StatusFilter::CompressedOnly]);
    }

    #[test]
    fn status_counts_cover_the_whole_library() {
        let entries = library();
        let mut state = ViewState {
            tab: Tab::Screenshots,
            ..ViewState::default()
        };
        state.toggle_status(StatusFilter::Exported);
        let view = build(&entries, &state, "", &[], &clock());
        assert_eq!(view.status_count(StatusFilter::Exported), 1);
        assert_eq!(view.status_count(StatusFilter::NotExported), 2);
        assert_eq!(view.status_count(StatusFilter::CompressedCopy), 2);
        assert_eq!(view.status_count(StatusFilter::CompressedOnly), 1);
    }

    #[test]
    fn screenshots_are_never_counted_as_not_exported() {
        let entries = vec![entry("shot", MediaKind::Screenshot, noon_on(5, 11))];
        let view = build(&entries, &ViewState::default(), "", &[], &clock());
        assert_eq!(view.status_count(StatusFilter::NotExported), 0);
    }

    #[test]
    fn search_matches_project_and_export_names_without_case() {
        let entries = library();
        let state = ViewState::default();
        let found = |query| names(&build(&entries, &state, query, &[], &clock()), &entries);
        assert_eq!(found("ARCH"), ["Archive"]);
        assert_eq!(found("launch fin"), ["demo"]);
        assert_eq!(found("  bug "), ["Bug"]);
    }

    #[test]
    fn shown_items_are_totalled() {
        let entries = library();
        let state = ViewState {
            tab: Tab::Instant,
            ..ViewState::default()
        };
        let view = build(&entries, &state, "", &[], &clock());
        assert_eq!(view.shown, 2);
        assert_eq!(view.shown_size, 500);
        assert_eq!(view.total, 4);
    }

    #[test]
    fn a_new_recording_goes_first_under_any_sort() {
        let entries = library();
        let mut state = ViewState::default();
        state.set_sort(SortKey::Size);
        let fresh = [PathBuf::from("Bug.ever")];
        let view = build(&entries, &state, "", &fresh, &clock());
        assert_eq!(names(&view, &entries)[0], "Bug");
    }

    #[test]
    fn a_new_recording_under_oldest_first_keeps_one_header_per_day() {
        let entries = vec![
            entry("new", MediaKind::Studio, noon_on(5, 11)),
            entry("earlier", MediaKind::Studio, noon_on(5, 11) - 1000.),
            entry("old", MediaKind::Studio, noon_on(12, 10)),
        ];
        let state = ViewState {
            direction: SortDirection::Ascending,
            ..ViewState::default()
        };
        let fresh = [PathBuf::from("new.ever")];
        let view = build(&entries, &state, "", &fresh, &clock());
        assert_eq!(names(&view, &entries), ["new", "earlier", "old"]);
        assert_eq!(view.sections.len(), 2);
    }

    #[test]
    fn surface_tells_an_empty_library_from_no_matches() {
        let view = build(&[], &ViewState::default(), "", &[], &clock());
        assert_eq!(view.surface(false), Surface::Empty);
        assert_eq!(view.surface(true), Surface::Items);

        let entries = library();
        let view = build(&entries, &ViewState::default(), "zzz", &[], &clock());
        assert_eq!(view.surface(false), Surface::NoMatch);
    }

    #[test]
    fn date_groups_follow_the_local_day_not_utc() {
        let late_utc = noon_on(4, 11) + DAY / 6. * 4.;
        assert_eq!(clock().group_of(late_utc), DateGroup::Today);
    }

    #[test]
    fn this_week_starts_on_monday() {
        assert_eq!(clock().group_of(noon_on(2, 11)), DateGroup::ThisWeek);
        assert_eq!(
            clock().group_of(noon_on(1, 11)),
            DateGroup::Month {
                year: 2026,
                month: 11
            }
        );
    }

    #[test]
    fn columns_fill_the_width_like_the_design_grid() {
        assert_eq!(columns(466., 150.), 3);
        assert_eq!(columns(465., 150.), 2);
        assert_eq!(columns(100., 150.), 1);
    }

    #[test]
    fn rows_break_each_section_into_columns_under_its_header() {
        let entries = library();
        let view = build(&entries[..2], &ViewState::default(), "", &[], &clock());
        assert_eq!(
            rows(&view, ViewMode::Grid, 2, false, &[]),
            [
                Row::Header(DateGroup::Today),
                Row::Tiles(vec![0]),
                Row::Header(DateGroup::Yesterday),
                Row::Tiles(vec![1]),
            ]
        );

        let mut state = ViewState::default();
        state.set_sort(SortKey::Name);
        let view = build(&entries, &state, "", &[], &clock());
        assert_eq!(
            rows(&view, ViewMode::Grid, 3, true, &[7]),
            [
                Row::FirstLine,
                Row::Imports(vec![7]),
                Row::Tiles(vec![3, 1, 0]),
                Row::Tiles(vec![2]),
            ]
        );
    }

    #[test]
    fn the_sticky_header_is_the_last_one_at_or_above_the_top_row() {
        let table = [
            Row::Header(DateGroup::Today),
            Row::Tiles(vec![0]),
            Row::Tiles(vec![1]),
            Row::Header(DateGroup::Yesterday),
            Row::Tiles(vec![2]),
        ];
        assert_eq!(header_above(&table, 2), Some(DateGroup::Today));
        assert_eq!(header_above(&table, 4), Some(DateGroup::Yesterday));
        assert_eq!(header_above(&[Row::Tiles(vec![0])], 0), None);
    }

    fn paths(names: &[&str]) -> Vec<PathBuf> {
        names.iter().map(|name| PathBuf::from(*name)).collect()
    }

    fn held(selection: &Selection) -> Vec<&str> {
        selection
            .items()
            .iter()
            .filter_map(|path| path.to_str())
            .collect()
    }

    #[test]
    fn clicking_a_column_sorts_by_it_and_a_second_click_reverses_it() {
        let mut state = ViewState::default();
        state.sort_by_column(SortKey::Name);
        assert_eq!(
            (state.sort, state.direction),
            (SortKey::Name, SortDirection::Ascending)
        );
        state.sort_by_column(SortKey::Name);
        assert_eq!(
            (state.sort, state.direction),
            (SortKey::Name, SortDirection::Descending)
        );
        state.sort_by_column(SortKey::Size);
        assert_eq!(
            (state.sort, state.direction),
            (SortKey::Size, SortDirection::Descending)
        );
        state.sort_by_column(SortKey::Size);
        assert_eq!(state.direction, SortDirection::Ascending);
    }

    #[test]
    fn the_list_has_one_row_per_item_and_no_headers() {
        let entries = library();
        let state = ViewState {
            view: ViewMode::List,
            ..ViewState::default()
        };
        let view = build(&entries, &state, "", &[], &clock());
        assert_eq!(
            rows(&view, ViewMode::List, 3, false, &[]),
            [Row::Item(0), Row::Item(1), Row::Item(2), Row::Item(3)]
        );
    }

    #[test]
    fn a_click_replaces_the_selection_and_ctrl_toggles_one_item() {
        let mut selection = Selection::default();
        selection.click(Path::new("a"));
        selection.click(Path::new("b"));
        assert_eq!(held(&selection), ["b"]);
        assert_eq!(selection.single(), Some(Path::new("b")));

        selection.toggle(Path::new("d"));
        assert_eq!(held(&selection), ["b", "d"]);
        assert_eq!(selection.single(), None);

        selection.toggle(Path::new("b"));
        assert_eq!(held(&selection), ["d"]);
    }

    #[test]
    fn taking_the_lead_out_with_ctrl_hands_the_lead_to_what_remains() {
        let mut selection = Selection::default();
        selection.click(Path::new("a"));
        selection.toggle(Path::new("b"));
        selection.toggle(Path::new("b"));
        assert_eq!(selection.single(), Some(Path::new("a")));
        assert_eq!(selection.lead(), Some(Path::new("a")));
    }

    #[test]
    fn shift_selects_from_the_anchor_in_shown_order_and_can_shrink_back() {
        let order = paths(&["a", "b", "c", "d", "e"]);
        let mut selection = Selection::default();
        selection.click(Path::new("b"));
        selection.extend_to(Path::new("d"), &order);
        assert_eq!(held(&selection), ["b", "c", "d"]);
        selection.extend_to(Path::new("a"), &order);
        assert_eq!(held(&selection), ["a", "b"]);
        assert_eq!(selection.lead(), Some(Path::new("a")));
    }

    #[test]
    fn shift_with_nothing_selected_selects_just_that_item() {
        let order = paths(&["a", "b"]);
        let mut selection = Selection::default();
        selection.extend_to(Path::new("b"), &order);
        assert_eq!(held(&selection), ["b"]);
    }

    #[test]
    fn a_ctrl_click_moves_the_anchor_for_the_next_shift_click() {
        let order = paths(&["a", "b", "c", "d"]);
        let mut selection = Selection::default();
        selection.click(Path::new("a"));
        selection.toggle(Path::new("c"));
        selection.extend_to(Path::new("d"), &order);
        assert_eq!(held(&selection), ["c", "d"]);
    }

    #[test]
    fn a_right_click_keeps_a_selection_it_lands_inside_and_replaces_one_it_misses() {
        let order = paths(&["a", "b", "c"]);
        let mut selection = Selection::default();
        selection.select_all(&order);
        selection.context_click(Path::new("b"));
        assert_eq!(held(&selection), ["a", "b", "c"]);
        selection.click(Path::new("a"));
        selection.context_click(Path::new("c"));
        assert_eq!(held(&selection), ["c"]);
    }

    #[test]
    fn a_marquee_adds_to_what_ctrl_held_and_replaces_otherwise() {
        let order = paths(&["a", "b", "c", "d"]);
        let hits = paths(&["c", "d"]);
        let mut selection = Selection::default();
        selection.marquee(&[], &hits, &order);
        assert_eq!(held(&selection), ["c", "d"]);
        selection.marquee(&paths(&["a"]), &hits, &order);
        assert_eq!(held(&selection), ["a", "c", "d"]);
        selection.marquee(&paths(&["a"]), &[], &order);
        assert_eq!(held(&selection), ["a"]);
    }

    #[test]
    fn items_that_leave_the_shown_list_leave_the_selection() {
        let mut selection = Selection::default();
        selection.select_all(&paths(&["a", "b", "c"]));
        selection.retain(&paths(&["a", "b"]));
        assert_eq!(held(&selection), ["a", "b"]);
        assert_eq!(selection.lead(), Some(Path::new("b")));
        selection.retain(&[]);
        assert!(selection.is_empty());
        assert_eq!(selection.lead(), None);
    }

    #[test]
    fn selection_order_follows_what_is_shown() {
        let entries = library();
        let mut state = ViewState::default();
        state.set_sort(SortKey::Name);
        let view = build(&entries, &state, "", &[], &clock());
        assert_eq!(
            view.order(&entries),
            paths(&["Archive.ever", "Bug.ever", "demo.ever", "shot.ever"])
        );
    }

    #[test]
    fn arrows_walk_the_grid_by_row_and_column_across_date_groups() {
        let table = [
            Row::Header(DateGroup::Today),
            Row::Tiles(vec![0, 1, 2]),
            Row::Tiles(vec![3]),
            Row::Header(DateGroup::Yesterday),
            Row::Tiles(vec![4, 5, 6]),
        ];
        assert_eq!(step_from(&table, None, Step::Down), Some(0));
        assert_eq!(step_from(&table, Some(1), Step::Right), Some(2));
        assert_eq!(step_from(&table, Some(2), Step::Right), Some(3));
        assert_eq!(step_from(&table, Some(3), Step::Left), Some(2));
        assert_eq!(step_from(&table, Some(2), Step::Down), Some(3));
        assert_eq!(step_from(&table, Some(3), Step::Down), Some(4));
        assert_eq!(step_from(&table, Some(5), Step::Up), Some(3));
        assert_eq!(step_from(&table, Some(1), Step::Up), Some(1));
        assert_eq!(step_from(&table, Some(6), Step::Down), Some(6));
        assert_eq!(step_from(&table, Some(6), Step::Right), Some(6));
        assert_eq!(step_from(&table, Some(0), Step::Left), Some(0));
    }

    #[test]
    fn arrows_walk_the_list_one_row_at_a_time_and_skip_imports() {
        let table = [
            Row::FirstLine,
            Row::Imports(vec![9]),
            Row::Item(0),
            Row::Item(1),
        ];
        assert_eq!(step_from(&table, Some(0), Step::Down), Some(1));
        assert_eq!(step_from(&table, Some(1), Step::Up), Some(0));
        assert_eq!(step_from(&table, Some(0), Step::Up), Some(0));
        assert_eq!(row_of(&table, 1), Some(3));
        assert_eq!(row_of(&table, 9), None);
    }

    #[test]
    fn an_item_that_is_gone_starts_the_arrows_from_the_top() {
        assert_eq!(step_from(&[Row::Item(4)], Some(1), Step::Down), Some(4));
        assert_eq!(step_from(&[], None, Step::Down), None);
    }

    #[test]
    fn a_moved_export_stays_searchable_but_no_longer_counts_as_exported() {
        let entries = vec![Entry {
            exports: vec![export("Moved.mp4", 1., None)],
            ..entry("demo", MediaKind::Studio, noon_on(5, 11))
        }];
        let mut state = ViewState::default();
        let view = build(&entries, &state, "moved", &[], &clock());
        assert_eq!(view.shown, 1);
        assert_eq!(view.status_count(StatusFilter::Exported), 0);
        assert_eq!(view.status_count(StatusFilter::NotExported), 1);
        state.toggle_status(StatusFilter::Exported);
        assert_eq!(build(&entries, &state, "", &[], &clock()).shown, 0);
    }

    #[test]
    fn the_latest_export_skips_files_that_are_gone() {
        let project = Entry {
            exports: vec![
                export("old.mp4", 1., Some(5)),
                export("new.mp4", 3., None),
                export("mid.mp4", 2., Some(5)),
            ],
            ..entry("demo", MediaKind::Studio, noon_on(5, 11))
        };
        assert_eq!(project.latest_export().unwrap().name, "mid.mp4");
    }

    #[test]
    fn days_read_as_today_yesterday_or_a_date_and_times_are_local() {
        let clock = clock();
        assert_eq!(clock.day_of(noon_on(5, 11)), DayLabel::Today);
        assert_eq!(clock.day_of(noon_on(4, 11)), DayLabel::Yesterday);
        assert_eq!(
            clock.day_of(noon_on(3, 11)),
            DayLabel::Date(NaiveDate::from_ymd_opt(2026, 11, 3).unwrap())
        );
        assert_eq!(clock.time_of(noon_on(5, 11)), "12:00");
    }

    #[test]
    fn tile_size_stays_inside_the_slider_range() {
        let mut state = ViewState::default();
        state.set_tile_size(TILE_MAX + 100.);
        assert_eq!(state.tile_size, TILE_MAX);
        assert_eq!(state.tile_fraction(), 1.);
        state.set_tile_size(0.);
        assert_eq!(state.tile_size, TILE_MIN);
    }

    #[test]
    fn view_state_survives_a_round_trip_and_an_older_file() {
        let mut state = ViewState {
            tab: Tab::Instant,
            view: ViewMode::List,
            tile_size: 200.,
            ..ViewState::default()
        };
        state.set_sort(SortKey::Duration);
        state.toggle_status(StatusFilter::CompressedCopy);
        let saved = serde_json::to_string(&state).unwrap();
        assert_eq!(serde_json::from_str::<ViewState>(&saved).unwrap(), state);

        let older: ViewState = serde_json::from_str("{}").unwrap();
        assert_eq!(older, ViewState::default());
    }

    #[test]
    fn sizes_and_durations_read_like_the_design_page() {
        assert_eq!(format_size(860 * 1024), "860 KB");
        assert_eq!(format_size(182 * 1024 * 1024), "182 MB");
        assert_eq!(format_size(1536 * 1024 * 1024), "1.5 GB");
        assert_eq!(format_clock(48.2), "0:48");
        assert_eq!(format_clock(222.), "3:42");
        assert_eq!(format_clock(3723.), "1:02:03");
    }
}
