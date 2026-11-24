//! On-disk bookkeeping for detached recordings started with `frameever record start --detach`.
//!
//! A detached recording runs in a re-exec'd worker process. The worker writes a `<id>.json` session
//! file describing the live recording; `frameever record stop` requests a stop by creating a `<id>.stop`
//! file (which the worker polls for, so it works on Windows where there is no SIGTERM) and waits for
//! the worker to flip the session status. `frameever record status` lists these files.

use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Recording,
    Stopped,
    Error,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub recording_id: String,
    pub pid: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid_started_at: Option<u64>,
    pub path: PathBuf,
    pub status: SessionStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recording_meta_exists: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub fn now_unix() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
}

pub fn sessions_dir() -> Result<PathBuf, String> {
    dirs::home_dir()
        .ok_or_else(|| "Could not determine home directory".to_string())
        .map(|home| home.join(".cap").join("sessions"))
}

pub fn session_file(id: &str) -> Result<PathBuf, String> {
    Ok(sessions_dir()?.join(format!("{id}.json")))
}

pub fn stop_file(id: &str) -> Result<PathBuf, String> {
    Ok(sessions_dir()?.join(format!("{id}.stop")))
}

pub fn log_file(id: &str) -> Result<PathBuf, String> {
    Ok(sessions_dir()?.join(format!("{id}.log")))
}

pub fn write_session(session: &Session) -> Result<(), String> {
    let dir = sessions_dir()?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Could not create sessions dir: {e}"))?;
    let path = session_file(&session.recording_id)?;
    let tmp = path.with_extension("json.tmp");
    let body = serde_json::to_vec_pretty(session).map_err(|e| e.to_string())?;
    // Write to a temp file then rename so a concurrent `stop`/`status` never reads a half-written file.
    std::fs::write(&tmp, body).map_err(|e| format!("Could not write session file: {e}"))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("Could not finalize session file: {e}"))
}

pub fn read_session(id: &str) -> Result<Session, String> {
    let path = session_file(id)?;
    let body = std::fs::read(&path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => format!("No recording session found with id '{id}'"),
        _ => format!("Could not read session file: {e}"),
    })?;
    serde_json::from_slice(&body).map_err(|e| format!("Corrupt session file for '{id}': {e}"))
}

pub fn list_sessions() -> Result<Vec<Session>, String> {
    let dir = sessions_dir()?;
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut sessions: Vec<Session> = std::fs::read_dir(&dir)
        .map_err(|e| format!("Could not read sessions dir: {e}"))?
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            if path.extension()?.to_str()? != "json" {
                return None;
            }
            serde_json::from_slice(&std::fs::read(&path).ok()?).ok()
        })
        .collect();
    sessions.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    Ok(sessions)
}

pub fn request_stop(id: &str) -> Result<(), String> {
    let dir = sessions_dir()?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Could not create sessions dir: {e}"))?;
    std::fs::write(stop_file(id)?, b"").map_err(|e| format!("Could not request stop: {e}"))
}

pub fn stop_requested(path: &Path) -> bool {
    path.exists()
}

pub fn cleanup(id: &str) {
    for path in [session_file(id), stop_file(id), log_file(id)]
        .into_iter()
        .flatten()
    {
        let _ = std::fs::remove_file(path);
    }
}

pub fn archive_log(id: &str, project_path: &Path) -> Result<Option<PathBuf>, String> {
    let source = log_file(id)?;
    if !source.exists() {
        return Ok(None);
    }
    if project_path.as_os_str().is_empty() || !project_path.is_dir() {
        return Ok(None);
    }

    let destination = project_path.join("detached-session.log");
    std::fs::copy(&source, &destination).map_err(|e| {
        format!(
            "Could not archive session log {} to {}: {e}",
            source.display(),
            destination.display()
        )
    })?;

    Ok(Some(destination))
}

impl Session {
    pub fn worker_running(&self) -> bool {
        self.pid_started_at
            .is_some_and(|started_at| process_started_at(self.pid) == Some(started_at))
    }
}

pub fn process_started_at(pid: u32) -> Option<u64> {
    let pid = sysinfo::Pid::from_u32(pid);
    let mut system = sysinfo::System::new();
    system.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::Some(&[pid]),
        true,
        sysinfo::ProcessRefreshKind::nothing(),
    );
    system.process(pid).map(sysinfo::Process::start_time)
}

#[cfg(unix)]
pub fn terminate(session: &Session) {
    if session.worker_running() {
        unsafe {
            libc::kill(session.pid as libc::pid_t, libc::SIGTERM);
        }
    }
}

#[cfg(not(unix))]
pub fn terminate(_session: &Session) {}
