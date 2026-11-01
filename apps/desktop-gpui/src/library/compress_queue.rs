use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobState {
    Waiting,
    Running { percent: u64 },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QueueSummary {
    pub finished: Vec<PathBuf>,
    pub failed: Vec<(PathBuf, String)>,
    pub saved: u64,
}

impl QueueSummary {
    pub fn total(&self) -> usize {
        self.finished.len() + self.failed.len()
    }
}

#[derive(Debug, Default)]
pub struct CompressQueue {
    waiting: VecDeque<PathBuf>,
    running: Option<(PathBuf, u64)>,
    summary: QueueSummary,
}

impl CompressQueue {
    pub fn add(&mut self, projects: impl IntoIterator<Item = PathBuf>) {
        for project in projects {
            if self.state(&project).is_none() {
                self.waiting.push_back(project);
            }
        }
    }

    pub fn state(&self, project: &Path) -> Option<JobState> {
        match &self.running {
            Some((running, percent)) if running == project => {
                Some(JobState::Running { percent: *percent })
            }
            _ => self
                .waiting
                .iter()
                .any(|waiting| waiting == project)
                .then_some(JobState::Waiting),
        }
    }

    pub fn start_next(&mut self) -> Option<PathBuf> {
        if self.running.is_some() {
            return None;
        }
        let project = self.waiting.pop_front()?;
        self.running = Some((project.clone(), 0));
        Some(project)
    }

    pub fn progress(&mut self, percent: u64) {
        if let Some((_, shown)) = &mut self.running {
            *shown = percent.min(100);
        }
    }

    pub fn finish(&mut self, result: Result<u64, String>) -> Option<QueueSummary> {
        let (project, _) = self.running.take()?;
        match result {
            Ok(saved) => {
                self.summary.finished.push(project);
                self.summary.saved += saved;
            }
            Err(error) => self.summary.failed.push((project, error)),
        }
        self.waiting
            .is_empty()
            .then(|| std::mem::take(&mut self.summary))
    }

    pub fn forget_waiting(&mut self, projects: &[PathBuf]) {
        self.waiting.retain(|project| !projects.contains(project));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(names: &[&str]) -> Vec<PathBuf> {
        names.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn only_one_job_runs_and_the_rest_wait_in_the_order_they_came() {
        let mut queue = CompressQueue::default();
        queue.add(paths(&["a", "b"]));
        assert_eq!(queue.start_next(), Some(PathBuf::from("a")));
        assert_eq!(queue.start_next(), None);
        assert_eq!(
            queue.state(Path::new("a")),
            Some(JobState::Running { percent: 0 })
        );
        assert_eq!(queue.state(Path::new("b")), Some(JobState::Waiting));
        assert_eq!(queue.state(Path::new("c")), None);

        queue.progress(40);
        assert_eq!(
            queue.state(Path::new("a")),
            Some(JobState::Running { percent: 40 })
        );

        assert_eq!(queue.finish(Ok(10)), None);
        assert_eq!(queue.start_next(), Some(PathBuf::from("b")));
    }

    #[test]
    fn adding_a_project_twice_queues_it_once() {
        let mut queue = CompressQueue::default();
        queue.add(paths(&["a", "a"]));
        queue.start_next();
        queue.add(paths(&["a"]));
        assert_eq!(queue.finish(Ok(0)).map(|summary| summary.total()), Some(1));
        assert_eq!(queue.start_next(), None);
    }

    #[test]
    fn the_summary_comes_once_at_the_end_with_each_failure_listed_once() {
        let mut queue = CompressQueue::default();
        queue.add(paths(&["a", "b", "c"]));
        queue.start_next();
        assert_eq!(queue.finish(Ok(5)), None);
        queue.start_next();
        assert_eq!(queue.finish(Err("full".to_string())), None);
        queue.start_next();
        assert_eq!(
            queue.finish(Ok(7)),
            Some(QueueSummary {
                finished: paths(&["a", "c"]),
                failed: vec![(PathBuf::from("b"), "full".to_string())],
                saved: 12,
            })
        );

        queue.add(paths(&["d"]));
        queue.start_next();
        assert_eq!(queue.finish(Ok(1)).map(|summary| summary.total()), Some(1));
    }

    #[test]
    fn a_deleted_project_leaves_the_queue_before_its_turn() {
        let mut queue = CompressQueue::default();
        queue.add(paths(&["a", "b"]));
        queue.start_next();
        queue.forget_waiting(&paths(&["a", "b"]));
        assert_eq!(queue.state(Path::new("b")), None);
        assert_eq!(
            queue.state(Path::new("a")),
            Some(JobState::Running { percent: 0 })
        );
        assert_eq!(queue.finish(Ok(3)).map(|summary| summary.saved), Some(3));
    }
}
