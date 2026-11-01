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
            Self::Exported => !entry.export_files.is_empty(),
            Self::NotExported => {
                entry.kind != MediaKind::Screenshot && entry.export_files.is_empty()
            }
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
pub struct Entry {
    pub path: PathBuf,
    pub kind: MediaKind,
    pub name: String,
    pub size: u64,
    pub duration: Option<f64>,
    pub created_millis: f64,
    pub export_files: Vec<String>,
    pub compression: Compression,
    pub clip_count: u32,
    pub thumbnail: Option<PathBuf>,
    pub thumbnail_version: Option<u128>,
    pub screenshot: Option<PathBuf>,
    pub opens_editor: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateGroup {
    Today,
    Yesterday,
    ThisWeek,
    Month { year: i32, month: u32 },
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
            .export_files
            .iter()
            .any(|file| file.to_lowercase().contains(needle))
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
}

pub fn rows(view: &LibraryView, columns: usize, first_line: bool, imports: &[usize]) -> Vec<Row> {
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
        rows.extend(
            section
                .items
                .chunks(columns)
                .map(|chunk| Row::Tiles(chunk.to_vec())),
        );
    }
    rows
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
            export_files: Vec::new(),
            compression: Compression::None,
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

    fn library() -> Vec<Entry> {
        vec![
            Entry {
                size: 300,
                duration: Some(100.),
                export_files: vec!["Launch final.mp4".into()],
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
            rows(&view, 2, false, &[]),
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
            rows(&view, 3, true, &[7]),
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
