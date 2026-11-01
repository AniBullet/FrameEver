#[derive(Debug, Clone, PartialEq)]
pub struct Timeline {
    durations: Vec<f64>,
}

impl Timeline {
    pub fn new(durations: Vec<f64>) -> Self {
        Self { durations }
    }

    pub fn total(&self) -> f64 {
        self.durations.iter().sum()
    }

    pub fn len(&self) -> usize {
        self.durations.len()
    }

    pub fn duration_of(&self, segment: usize) -> f64 {
        self.durations.get(segment).copied().unwrap_or(0.)
    }

    pub fn start_of(&self, segment: usize) -> f64 {
        self.durations.iter().take(segment).sum()
    }

    pub fn locate(&self, position: f64) -> (usize, f64) {
        let mut left = position.max(0.);
        for (segment, duration) in self.durations.iter().enumerate() {
            if left < *duration {
                return (segment, left);
            }
            left -= duration;
        }
        let last = self.durations.len().saturating_sub(1);
        (last, self.duration_of(last))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_land_in_the_segment_that_holds_them() {
        let timeline = Timeline::new(vec![2., 3.]);
        assert_eq!(timeline.total(), 5.);
        assert_eq!(timeline.locate(0.), (0, 0.));
        assert_eq!(timeline.locate(1.5), (0, 1.5));
        assert_eq!(timeline.locate(2.), (1, 0.));
        assert_eq!(timeline.locate(4.), (1, 2.));
    }

    #[test]
    fn positions_past_either_end_stay_on_the_timeline() {
        let timeline = Timeline::new(vec![2., 3.]);
        assert_eq!(timeline.locate(-1.), (0, 0.));
        assert_eq!(timeline.locate(9.), (1, 3.));
    }

    #[test]
    fn segments_start_where_the_earlier_ones_end() {
        let timeline = Timeline::new(vec![2., 3., 1.]);
        assert_eq!(timeline.start_of(0), 0.);
        assert_eq!(timeline.start_of(2), 5.);
    }
}
