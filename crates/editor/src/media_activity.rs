use std::sync::atomic::{AtomicUsize, Ordering};

static RUNNING: AtomicUsize = AtomicUsize::new(0);

#[must_use]
pub struct MediaActivity(());

impl MediaActivity {
    pub fn begin() -> Self {
        RUNNING.fetch_add(1, Ordering::AcqRel);
        Self(())
    }

    pub fn any_running() -> bool {
        RUNNING.load(Ordering::Acquire) > 0
    }
}

impl Drop for MediaActivity {
    fn drop(&mut self) {
        RUNNING.fetch_sub(1, Ordering::AcqRel);
    }
}
