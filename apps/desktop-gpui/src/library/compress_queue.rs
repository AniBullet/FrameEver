use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{Condvar, Mutex, MutexGuard, PoisonError},
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobState {
    Waiting,
    Running {
        percent: u64,
        left: Option<Duration>,
    },
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
    pub automatic: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Saved(u64),
    Failed(String),
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Progress,
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Overview {
    pub position: usize,
    pub total: usize,
    pub percent: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    pub project: PathBuf,
    pub automatic: bool,
    pub state: JobState,
}

#[derive(Debug)]
struct Running {
    job: Job,
    percent: u64,
    started: Instant,
    cancelled: bool,
}

impl Running {
    fn left(&self, now: Instant) -> Option<Duration> {
        (self.percent >= 3 && self.percent < 100).then(|| {
            now.saturating_duration_since(self.started)
                .mul_f64((100 - self.percent) as f64 / self.percent as f64)
        })
    }
}

#[derive(Debug, Default)]
pub struct CompressQueue {
    waiting: VecDeque<Job>,
    running: Option<Running>,
    summary: QueueSummary,
    paused: bool,
}

impl CompressQueue {
    pub fn add(&mut self, projects: impl IntoIterator<Item = PathBuf>, request: Request) {
        for project in projects {
            let known = self
                .running
                .iter_mut()
                .map(|running| &mut running.job)
                .chain(self.waiting.iter_mut())
                .find(|job| job.project == project);
            match known {
                Some(job) => job.drop_original |= request.drop_original,
                None => self.waiting.push_back(Job {
                    project,
                    drop_original: request.drop_original,
                    automatic: request.automatic,
                }),
            }
        }
    }

    pub fn state(&self, project: &Path) -> Option<JobState> {
        match &self.running {
            Some(running) if running.job.project == project => Some(JobState::Running {
                percent: running.percent,
                left: running.left(Instant::now()),
            }),
            _ => self
                .waiting
                .iter()
                .any(|waiting| waiting.project == project)
                .then_some(JobState::Waiting),
        }
    }

    pub fn list(&self) -> Vec<Listed> {
        let now = Instant::now();
        self.running
            .iter()
            .map(|running| Listed {
                project: running.job.project.clone(),
                automatic: running.job.automatic,
                state: JobState::Running {
                    percent: running.percent,
                    left: running.left(now),
                },
            })
            .chain(self.waiting.iter().map(|job| Listed {
                project: job.project.clone(),
                automatic: job.automatic,
                state: JobState::Waiting,
            }))
            .collect()
    }

    pub fn overview(&self) -> Option<Overview> {
        let running = self.running.as_ref()?;
        let position = self.summary.total() + 1;
        Some(Overview {
            position,
            total: position + self.waiting.len(),
            percent: running.percent,
        })
    }

    pub fn start_next(&mut self) -> Option<Job> {
        if self.running.is_some() {
            return None;
        }
        let job = self.waiting.pop_front()?;
        self.running = Some(Running {
            job: job.clone(),
            percent: 0,
            started: Instant::now(),
            cancelled: false,
        });
        Some(job)
    }

    pub fn progress(&mut self, percent: u64) -> bool {
        match &mut self.running {
            Some(running) if running.percent != percent.min(100) => {
                running.percent = percent.min(100);
                true
            }
            _ => false,
        }
    }

    fn cancelled(&self) -> bool {
        self.running
            .as_ref()
            .is_some_and(|running| running.cancelled)
    }

    fn drop_original(&self) -> bool {
        self.running
            .as_ref()
            .is_some_and(|running| running.job.drop_original)
    }

    pub fn cancel(&mut self, project: &Path) {
        self.waiting.retain(|job| job.project != project);
        if let Some(running) = &mut self.running
            && running.job.project == project
        {
            running.cancelled = true;
        }
    }

    pub fn cancel_all(&mut self) {
        self.waiting.clear();
        if let Some(running) = &mut self.running {
            running.cancelled = true;
        }
    }

    pub fn finish(&mut self, outcome: Outcome) -> Option<QueueSummary> {
        let running = self.running.take()?;
        match outcome {
            Outcome::Saved(saved) => {
                self.summary.finished.push(running.job.project);
                self.summary.saved += saved;
            }
            Outcome::Failed(error) => self.summary.failed.push((running.job.project, error)),
            Outcome::Cancelled => {}
        }
        self.waiting
            .is_empty()
            .then(|| std::mem::take(&mut self.summary))
    }

    pub fn forget_waiting(&mut self, projects: &[PathBuf]) {
        self.waiting.retain(|job| !projects.contains(&job.project));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Request {
    pub drop_original: bool,
    pub automatic: bool,
}

impl Request {
    pub const MANUAL: Self = Self {
        drop_original: false,
        automatic: false,
    };
}

pub struct Compressions {
    queue: Mutex<CompressQueue>,
    resumed: Condvar,
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
                paused: false,
            }),
            resumed: Condvar::new(),
            listeners: Mutex::new(Vec::new()),
        }
    }

    fn queue(&self) -> MutexGuard<'_, CompressQueue> {
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

    pub fn list(&self) -> Vec<Listed> {
        self.queue().list()
    }

    pub fn overview(&self) -> Option<Overview> {
        self.queue().overview()
    }

    pub fn forget_waiting(&self, projects: &[PathBuf]) {
        self.queue().forget_waiting(projects);
    }

    pub fn cancel(&self, project: &Path) {
        self.queue().cancel(project);
        self.resumed.notify_all();
        self.tell(Change::Progress);
    }

    pub fn cancel_all(&self) {
        self.queue().cancel_all();
        self.resumed.notify_all();
        self.tell(Change::Progress);
    }

    pub fn set_paused(&self, paused: bool) {
        self.queue().paused = paused;
        self.resumed.notify_all();
    }

    fn keep_going(&self, percent: u64) -> bool {
        let mut queue = self.queue();
        let moved = queue.progress(percent);
        while queue.paused && !queue.cancelled() {
            queue = self
                .resumed
                .wait(queue)
                .unwrap_or_else(PoisonError::into_inner);
        }
        let going = !queue.cancelled();
        drop(queue);
        if moved {
            self.tell(Change::Progress);
        }
        going
    }

    fn add(&self, projects: impl IntoIterator<Item = PathBuf>, request: Request) -> Option<Job> {
        let first = {
            let mut queue = self.queue();
            queue.add(projects, request);
            queue.start_next()
        };
        self.tell(Change::Progress);
        first
    }

    fn drain(
        &self,
        mut job: Job,
        work: impl Fn(&Job, &mut dyn FnMut(u64) -> bool) -> Result<u64, String>,
        report: impl Fn(QueueSummary),
    ) {
        loop {
            let result = work(&job, &mut |percent| self.keep_going(percent));
            let (summary, next) = {
                let mut queue = self.queue();
                let outcome = match result {
                    _ if queue.cancelled() => Outcome::Cancelled,
                    Ok(saved) => Outcome::Saved(saved),
                    Err(error) => Outcome::Failed(error),
                };
                let summary = queue.finish(outcome);
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

pub fn compress(projects: impl IntoIterator<Item = PathBuf>, request: Request) {
    let Some(first) = COMPRESSIONS.add(projects, request) else {
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
            queue.finish(Outcome::Failed(error.to_string()))
        };
        COMPRESSIONS.tell(Change::Finished);
        if let Some(summary) = summary {
            crate::auto_compress::report(summary);
        }
    }
}

fn compress_one(job: &Job, keep_going: &mut dyn FnMut(u64) -> bool) -> Result<u64, String> {
    super::versions::compress(&job.project, |done, total| {
        keep_going(done * 100 / total.max(1))
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

    const AUTO_DROP: Request = Request {
        drop_original: true,
        automatic: true,
    };

    fn paths(names: &[&str]) -> Vec<PathBuf> {
        names.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn only_one_job_runs_and_the_rest_wait_in_the_order_they_came() {
        let mut queue = CompressQueue::default();
        queue.add(paths(&["a", "b"]), Request::MANUAL);
        assert_eq!(
            queue.start_next().map(|job| job.project),
            Some(PathBuf::from("a"))
        );
        assert_eq!(queue.start_next(), None);
        assert_eq!(
            queue.state(Path::new("a")),
            Some(JobState::Running {
                percent: 0,
                left: None
            })
        );
        assert_eq!(queue.state(Path::new("b")), Some(JobState::Waiting));
        assert_eq!(queue.state(Path::new("c")), None);

        queue.progress(40);
        assert!(matches!(
            queue.state(Path::new("a")),
            Some(JobState::Running {
                percent: 40,
                left: Some(_)
            })
        ));

        assert_eq!(queue.finish(Outcome::Saved(10)), None);
        assert_eq!(
            queue.start_next().map(|job| job.project),
            Some(PathBuf::from("b"))
        );
    }

    #[test]
    fn adding_a_project_twice_queues_it_once() {
        let mut queue = CompressQueue::default();
        queue.add(paths(&["a", "a"]), Request::MANUAL);
        queue.start_next();
        queue.add(paths(&["a"]), Request::MANUAL);
        assert_eq!(
            queue
                .finish(Outcome::Saved(0))
                .map(|summary| summary.total()),
            Some(1)
        );
        assert_eq!(queue.start_next(), None);
    }

    #[test]
    fn the_summary_comes_once_at_the_end_with_each_failure_listed_once() {
        let mut queue = CompressQueue::default();
        queue.add(paths(&["a", "b", "c"]), Request::MANUAL);
        queue.start_next();
        assert_eq!(queue.finish(Outcome::Saved(5)), None);
        queue.start_next();
        assert_eq!(queue.finish(Outcome::Failed("full".to_string())), None);
        queue.start_next();
        assert_eq!(
            queue.finish(Outcome::Saved(7)),
            Some(QueueSummary {
                finished: paths(&["a", "c"]),
                failed: vec![(PathBuf::from("b"), "full".to_string())],
                saved: 12,
            })
        );

        queue.add(paths(&["d"]), Request::MANUAL);
        queue.start_next();
        assert_eq!(
            queue
                .finish(Outcome::Saved(1))
                .map(|summary| summary.total()),
            Some(1)
        );
    }

    #[test]
    fn a_deleted_project_leaves_the_queue_before_its_turn() {
        let mut queue = CompressQueue::default();
        queue.add(paths(&["a", "b"]), Request::MANUAL);
        queue.start_next();
        queue.forget_waiting(&paths(&["a", "b"]));
        assert_eq!(queue.state(Path::new("b")), None);
        assert_eq!(
            queue.state(Path::new("a")),
            Some(JobState::Running {
                percent: 0,
                left: None
            })
        );
        assert_eq!(
            queue.finish(Outcome::Saved(3)).map(|summary| summary.saved),
            Some(3)
        );
    }

    #[test]
    fn a_click_while_the_automatic_compression_runs_joins_it_instead_of_starting_another() {
        let compressions = Compressions::new();
        let changes = compressions.subscribe();
        let first = compressions
            .add(paths(&["a"]), AUTO_DROP)
            .expect("the automatic job starts");
        let runs = std::cell::RefCell::new(Vec::new());
        let reports = std::cell::RefCell::new(Vec::new());
        compressions.drain(
            first,
            |job, progress| {
                progress(50);
                assert!(matches!(
                    compressions.state(&job.project),
                    Some(JobState::Running { percent: 50, .. })
                ));
                assert_eq!(compressions.add(paths(&["a"]), Request::MANUAL), None);
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
                automatic: true,
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
        queue.add(paths(&["busy", "a"]), Request::MANUAL);
        queue.start_next();
        queue.add(paths(&["a"]), AUTO_DROP);
        queue.finish(Outcome::Saved(0));
        assert_eq!(
            queue.start_next(),
            Some(Job {
                project: PathBuf::from("a"),
                drop_original: true,
                automatic: false,
            })
        );
    }

    #[test]
    fn cancelling_stops_the_running_job_drops_a_waiting_one_and_reports_neither_as_failed() {
        let compressions = Compressions::new();
        let first = compressions
            .add(paths(&["a", "b", "c"]), Request::MANUAL)
            .expect("a starts");
        let reports = std::cell::RefCell::new(Vec::new());
        compressions.drain(
            first,
            |job, keep_going| {
                if job.project == Path::new("a") {
                    compressions.cancel(Path::new("b"));
                    compressions.cancel(&job.project);
                    assert!(!keep_going(10));
                    return Err("Cancelled".to_string());
                }
                Ok(2)
            },
            |summary| reports.borrow_mut().push(summary),
        );
        assert_eq!(
            reports.into_inner(),
            vec![QueueSummary {
                finished: paths(&["c"]),
                failed: Vec::new(),
                saved: 2,
            }]
        );
    }

    #[test]
    fn a_paused_queue_waits_until_it_is_resumed() {
        let compressions = std::sync::Arc::new(Compressions::new());
        let first = compressions
            .add(paths(&["a"]), Request::MANUAL)
            .expect("a starts");
        compressions.set_paused(true);
        let worker = {
            let compressions = compressions.clone();
            std::thread::spawn(move || {
                compressions.drain(first, |_, keep_going| Ok(u64::from(keep_going(50))), |_| {})
            })
        };
        std::thread::sleep(Duration::from_millis(50));
        assert!(matches!(
            compressions.state(Path::new("a")),
            Some(JobState::Running { percent: 50, .. })
        ));
        compressions.set_paused(false);
        worker.join().unwrap();
        assert_eq!(compressions.state(Path::new("a")), None);
    }

    #[test]
    fn the_overview_counts_the_whole_batch_and_the_estimate_follows_the_pace() {
        let mut queue = CompressQueue::default();
        queue.add(paths(&["a", "b", "c"]), Request::MANUAL);
        queue.start_next();
        queue.finish(Outcome::Saved(1));
        queue.start_next();
        queue.progress(25);
        assert_eq!(
            queue.overview(),
            Some(Overview {
                position: 2,
                total: 3,
                percent: 25,
            })
        );
        let running = Running {
            job: Job {
                project: PathBuf::from("a"),
                drop_original: false,
                automatic: false,
            },
            percent: 25,
            started: Instant::now(),
            cancelled: false,
        };
        assert_eq!(
            running.left(running.started + Duration::from_secs(20)),
            Some(Duration::from_secs(60))
        );
    }
}
