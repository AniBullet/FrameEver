use frameever_utils::storage::ChapterMarker;

const MIN_MARK_GAP: f64 = 0.05;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct ChapterMarks {
    marks: Vec<ChapterMarker>,
    deleted: Option<ChapterMarker>,
}

impl ChapterMarks {
    pub fn new(mut marks: Vec<ChapterMarker>) -> Self {
        marks.sort_by(|a, b| a.recorded_at.total_cmp(&b.recorded_at));
        Self {
            marks,
            deleted: None,
        }
    }

    pub fn list(&self) -> &[ChapterMarker] {
        &self.marks
    }

    pub fn recorded_times(&self) -> Vec<f64> {
        self.marks.iter().map(|mark| mark.recorded_at).collect()
    }

    pub fn deleted(&self) -> Option<&ChapterMarker> {
        self.deleted.as_ref()
    }

    pub fn add(&mut self, recorded_at: f64) -> Option<usize> {
        let index = self.insert(ChapterMarker {
            recorded_at,
            name: None,
        })?;
        self.deleted = None;
        Some(index)
    }

    pub fn rename(&mut self, index: usize, name: &str) -> bool {
        let name = Some(name.trim()).filter(|name| !name.is_empty());
        let Some(mark) = self.marks.get_mut(index) else {
            return false;
        };
        if mark.name.as_deref() == name {
            return false;
        }
        mark.name = name.map(str::to_string);
        self.deleted = None;
        true
    }

    pub fn move_to(&mut self, index: usize, recorded_at: f64) -> Option<usize> {
        if index >= self.marks.len() {
            return None;
        }
        let mark = self.marks.remove(index);
        let moved = ChapterMarker {
            recorded_at,
            name: mark.name.clone(),
        };
        match self.insert(moved) {
            Some(new_index) => {
                self.deleted = None;
                Some(new_index)
            }
            None => {
                self.marks.insert(index, mark);
                None
            }
        }
    }

    pub fn delete(&mut self, index: usize) -> bool {
        if index >= self.marks.len() {
            return false;
        }
        self.deleted = Some(self.marks.remove(index));
        true
    }

    pub fn restore_deleted(&mut self) -> Option<usize> {
        let mark = self.deleted.take()?;
        self.insert(mark)
    }

    fn insert(&mut self, mark: ChapterMarker) -> Option<usize> {
        if !mark.recorded_at.is_finite()
            || mark.recorded_at < 0.0
            || self
                .marks
                .iter()
                .any(|other| (other.recorded_at - mark.recorded_at).abs() < MIN_MARK_GAP)
        {
            return None;
        }
        let index = self
            .marks
            .partition_point(|other| other.recorded_at < mark.recorded_at);
        self.marks.insert(index, mark);
        Some(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mark(recorded_at: f64, name: Option<&str>) -> ChapterMarker {
        ChapterMarker {
            recorded_at,
            name: name.map(str::to_string),
        }
    }

    fn times(marks: &ChapterMarks) -> Vec<f64> {
        marks.list().iter().map(|mark| mark.recorded_at).collect()
    }

    #[test]
    fn marks_load_in_recorded_order_and_a_new_one_lands_in_place() {
        let mut marks = ChapterMarks::new(vec![mark(9.0, None), mark(2.0, Some("Intro"))]);

        assert_eq!(marks.add(5.0), Some(1));
        assert_eq!(times(&marks), vec![2.0, 5.0, 9.0]);
        assert_eq!(marks.add(5.01), None);
        assert_eq!(times(&marks), vec![2.0, 5.0, 9.0]);
    }

    #[test]
    fn a_rename_trims_and_an_empty_name_goes_back_to_the_numbered_one() {
        let mut marks = ChapterMarks::new(vec![mark(2.0, None)]);

        assert!(marks.rename(0, "  Setup  "));
        assert_eq!(marks.list()[0].name.as_deref(), Some("Setup"));
        assert!(!marks.rename(0, "Setup"));
        assert!(marks.rename(0, "   "));
        assert_eq!(marks.list()[0].name, None);
        assert!(!marks.rename(3, "Nowhere"));
    }

    #[test]
    fn a_moved_mark_keeps_its_name_and_takes_its_new_place_in_order() {
        let mut marks = ChapterMarks::new(vec![mark(2.0, Some("Intro")), mark(9.0, None)]);

        assert_eq!(marks.move_to(0, 12.0), Some(1));
        assert_eq!(marks.list()[1], mark(12.0, Some("Intro")));
        assert_eq!(marks.move_to(1, 9.0), None);
        assert_eq!(times(&marks), vec![9.0, 12.0]);
    }

    #[test]
    fn a_deleted_mark_can_be_put_back_until_the_next_change() {
        let mut marks = ChapterMarks::new(vec![mark(2.0, Some("Intro")), mark(9.0, None)]);

        assert!(marks.delete(0));
        assert_eq!(times(&marks), vec![9.0]);
        assert_eq!(marks.deleted(), Some(&mark(2.0, Some("Intro"))));
        assert_eq!(marks.restore_deleted(), Some(0));
        assert_eq!(times(&marks), vec![2.0, 9.0]);
        assert_eq!(marks.deleted(), None);

        assert!(marks.delete(1));
        assert_eq!(marks.add(4.0), Some(1));
        assert_eq!(marks.deleted(), None);
        assert_eq!(marks.restore_deleted(), None);
        assert!(!marks.delete(5));
    }
}
