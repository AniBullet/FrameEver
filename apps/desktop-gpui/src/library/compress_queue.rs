use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{Mutex, PoisonError},
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    pub project: PathBuf,
    pub drop_original: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Progress,
    Finished,
}

#[derive(Debug, Default)]
pub struct CompressQueue {
    waiting: VecDeque<Job>,
    running: Option<(Job, u64)>,
    summary: QueueSummary,
}

impl CompressQueue {
    pub fn add(&mut self, projects: impl IntoIterator<Item = PathBuf>, drop_original: bool) {
        for project in projects {
            let known = self
                .running
                .iter_mut()
                .map(|(job, _)| job)
                .chain(self.waiting.iter_mut())
                .find(|job| job.project == project);
            match known {
                Some(job) => job.drop_original |= drop_original,
                None => self.waiting.push_back(Job {
                    project,
                    drop_original,
                }),
            }
        }
    }

    pub fn state(&self, project: &Path) -> Option<JobState> {
        match &self.running {
            Some((running, percent)) if running.project == project => {
                Some(JobState::Running { percent: *percent })
            }
            _ => self
                .waiting
                .iter()
                .any(|waiting| waiting.project == project)
                .then_some(JobState::Waiting),
        }
    }

    pub fn start_next(&mut self) -> Option<Job> {
        if self.running.is_some() {
            return None;
        }
        let job = self.waiting.pop_front()?;
        self.running = Some((job.clone(), 0));
        Some(job)
    }

    pub fn progress(&mut self, percent: u64) -> bool {
        match &mut self.running {
            Some((_, shown)) if *shown != percent.min(100) => {
                *shown = percent.min(100);
                true
            }
            _ => false,
        }
    }

    fn drop_original(&self) -> bool {
        self.running
            .as_ref()
            .is_some_and(|(job, _)| job.drop_original)
    }

    pub fn finish(&mut self, result: Result<u64, String>) -> Option<QueueSummary> {
        let (job, _) = self.running.take()?;
        match result {
            Ok(saved) => {
                self.summary.finished.push(job.project);
                self.summary.saved += saved;
            }
            Err(error) => self.summary.failed.push((job.project, error)),
        }
        self.waiting
            .is_empty()
            .then(|| std::mem::take(&mut self.summary))
    }

    pub fn forget_waiting(&mut self, projects: &[PathBuf]) {
        self.waiting.retain(|job| !projects.contains(&job.project));
    }
}

pub struct Compressions {
    queue: Mutex<CompressQueue>,
    listeners: Mutex<Vec<flume::Sender<Change>>>,
}

static COMPRESSIONS: Compressions = Compressions::new();

impl Compressions {
    const fn new() -> Self {
        Self {
            queue: Mutex::new(CompressQueue {
                waiting: VecDeque::new(),
                running: None,
                summary: QueueSummary {
                    finished: Vec::new(),
                    failed: Vec::new(),
                    saved: 0,
                },
            }),
            listeners: Mutex::new(Vec::new()),
        }
    }

    fn queue(&self) -> std::sync::MutexGuard<'_, CompressQueue> {
        self.queue.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn tell(&self, change: Change) {
        self.listeners
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|listener| listener.send(change).is_ok());
    }

    pub fn subscribe(&self) -> flume::Receiver<Change> {
        let (sender, receiver) = flume::unbounded();
        self.listeners
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(sender);
        receiver
    }

    pub fn state(&self, project: &Path) -> Option<JobState> {
        self.queue().state(project)
    }

    pub fn forget_waiting(&self, projects: &[PathBuf]) {
        self.queue().forget_waiting(projects);
    }

    fn add(&self, projects: impl IntoIterator<Item = PathBuf>, drop_original: bool) -> Option<Job> {
        let first = {
            let mut queue = self.queue();
            queue.add(projects, drop_original);
            queue.start_next()
        };
        self.tell(Change::Progress);
        first
    }

    fn drain(
        &self,
        mut job: Job,
        work: impl Fn(&Job, &mut dyn FnMut(u64)) -> Result<u64, String>,
        report: impl Fn(QueueSummary),
    ) {
        loop {
            let result = work(&job, &mut |percent| {
                if self.queue().progress(percent) {
                    self.tell(Change::Progress);
                }
            });
            let (summary, next) = {
                let mut queue = self.queue();
                let summary = queue.finish(result);
                (summary, queue.start_next())
            };
            self.tell(Change::Finished);
            if let Some(summary) = summary {
                report(summary);
            }
            match next {
                Some(next) => job = next,
                None => return,
            }
        }
    }
}

pub fn compressions() -> &'static Compressions {
    &COMPRESSIONS
}

pub fn compress(projects: impl IntoIterator<Item = PathBuf>, drop_original: bool) {
    let Some(first) = COMPRESSIONS.add(projects, drop_original) else {
        return;
    };
    let spawned = std::thread::Builder::new()
        .name("compression".to_string())
        .spawn(move || COMPRESSIONS.drain(first, compress_one, crate::auto_compress::report));
    if let Err(error) = spawned {
        tracing::error!("starting the compression thread failed: {error}");
        let summary = {
            let mut queue = COMPRESSIONS.queue();
            queue.waiting.clear();
            queue.finish(Err(error.to_string()))
        };
        COMPRESSIONS.tell(Change::Finished);
        if let Some(summary) = summary {
            crate::auto_compress::report(summary);
        }
    }
}

fn compress_one(job: &Job, progress: &mut dyn FnMut(u64)) -> Result<u64, String> {
    super::versions::compress(&job.project, |done, total| {
        progress(done * 100 / total.max(1));
        true
    })?;
    let saved = super::video_versions(&job.project).map_or(0, super::VideoVersions::saved);
    let keep_only = COMPRESSIONS.queue().drop_original();
    if keep_only && let Err(error) = super::delete_original(&job.project) {
        tracing::warn!(path = %job.project.display(), "could not remove the original after compressing: {error}");
    }
    Ok(saved)
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
        queue.add(paths(&["a", "b"]), false);
        assert_eq!(
            queue.start_next().map(|job| job.project),
            Some(PathBuf::from("a"))
        );
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
        assert_eq!(
            queue.start_next().map(|job| job.project),
            Some(PathBuf::from("b"))
        );
    }

    #[test]
    fn adding_a_project_twice_queues_it_once() {
        let mut queue = CompressQueue::default();
        queue.add(paths(&["a", "a"]), false);
        queue.start_next();
        queue.add(paths(&["a"]), false);
        assert_eq!(queue.finish(Ok(0)).map(|summary| summary.total()), Some(1));
        assert_eq!(queue.start_next(), None);
    }

    #[test]
    fn the_summary_comes_once_at_the_end_with_each_failure_listed_once() {
        let mut queue = CompressQueue::default();
        queue.add(paths(&["a", "b", "c"]), false);
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

        queue.add(paths(&["d"]), false);
        queue.start_next();
        assert_eq!(queue.finish(Ok(1)).map(|summary| summary.total()), Some(1));
    }

    #[test]
    fn a_deleted_project_leaves_the_queue_before_its_turn() {
        let mut queue = CompressQueue::default();
        queue.add(paths(&["a", "b"]), false);
        queue.start_next();
        queue.forget_waiting(&paths(&["a", "b"]));
        assert_eq!(queue.state(Path::new("b")), None);
        assert_eq!(
            queue.state(Path::new("a")),
            Some(JobState::Running { percent: 0 })
        );
        assert_eq!(queue.finish(Ok(3)).map(|summary| summary.saved), Some(3));
    }

    #[test]
    fn a_click_while_the_automatic_compression_runs_joins_it_instead_of_starting_another() {
        let compressions = Compressions::new();
        let changes = compressions.subscribe();
        let first = compressions
            .add(paths(&["a"]), true)
            .expect("the automatic job starts");
        let runs = std::cell::RefCell::new(Vec::new());
        let reports = std::cell::RefCell::new(Vec::new());
        compressions.drain(
            first,
            |job, progress| {
                progress(50);
                assert_eq!(
                    compressions.state(&job.project),
                    Some(JobState::Running { percent: 50 })
                );
                assert_eq!(compressions.add(paths(&["a"]), false), None);
                runs.borrow_mut().push(job.clone());
                Ok(4)
            },
            |summary| reports.borrow_mut().push(summary),
        );
        assert_eq!(
            runs.into_inner(),
            vec![Job {
                project: PathBuf::from("a"),
                drop_original: true,
            }]
        );
        assert_eq!(
            reports.into_inner(),
            vec![QueueSummary {
                finished: paths(&["a"]),
                failed: Vec::new(),
                saved: 4,
            }]
        );
        assert_eq!(compressions.state(Path::new("a")), None);
        assert_eq!(changes.try_iter().last(), Some(Change::Finished));
    }

    #[test]
    fn a_manual_request_waiting_for_its_turn_takes_on_the_automatic_drop_of_the_original() {
        let mut queue = CompressQueue::default();
        queue.add(paths(&["busy", "a"]), false);
        queue.start_next();
        queue.add(paths(&["a"]), true);
        queue.finish(Ok(0));
        assert_eq!(
            queue.start_next(),
            Some(Job {
                project: PathBuf::from("a"),
                drop_original: true,
            })
        );
    }
}
