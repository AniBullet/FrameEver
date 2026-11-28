use std::{path::PathBuf, time::Duration};

use futures_util::future::{Either, select};
use gpui::{App, Global};
use semver::Version;

use crate::{
    i18n::{n, t, tr},
    installer::{self, ArtifactKind, Release},
    session::RecordingSession,
    store::{GeneralSettings, UpdateChannel, UpdateDownloadPolicy},
};

mod offer;

pub(crate) use offer::{Indicator, UpdateFailure, UpdateMemory, UpdateStatus};
use offer::{Notice, indicator, notice, offered};

const STABLE_FIRST_CHECK_DELAY: Duration = Duration::from_secs(10);
const NIGHTLY_FIRST_CHECK_DELAY: Duration = Duration::from_secs(60);
const NIGHTLY_CHECK_INTERVAL: Duration = Duration::from_secs(2 * 60 * 60);
const BUSY_RETRY_DELAY: Duration = Duration::from_secs(5 * 60);
const BUSY_MESSAGE: &str =
    n!("Finish your recording, export, import, or transcription task before updating.");

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct PendingUpdateRequests {
    manual: bool,
    channel: Option<UpdateChannel>,
}

impl PendingUpdateRequests {
    fn request_manual(&mut self, manual_in_flight: bool) -> bool {
        if self.manual || manual_in_flight {
            return false;
        }

        self.manual = true;
        true
    }

    fn request_channel(&mut self, channel: UpdateChannel) {
        self.channel = Some(channel);
    }
}

struct StagedUpdate {
    kind: ArtifactKind,
    package: PathBuf,
}

struct UpdateScheduler {
    wake: flume::Sender<()>,
    pending: PendingUpdateRequests,
    manual_in_flight: bool,
    status: UpdateStatus,
    download_policy: UpdateDownloadPolicy,
    channel: UpdateChannel,
    staged: Option<StagedUpdate>,
    download: Option<gpui::Task<()>>,
    download_epoch: u64,
    dismissed: bool,
    notice_waiting: bool,
    memory: UpdateMemory,
}

impl Global for UpdateScheduler {}

fn qualifies(
    current: &Version,
    remote: &Version,
    configured_channel: UpdateChannel,
    remote_channel: UpdateChannel,
) -> bool {
    remote > current
        || (configured_channel == UpdateChannel::Stable
            && remote_channel == UpdateChannel::Stable
            && !current.pre.is_empty()
            && remote.pre.is_empty()
            && remote != current)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReleaseStep {
    Skip,
    Download,
    Ask,
}

fn release_step(policy: UpdateDownloadPolicy, manual: bool) -> ReleaseStep {
    match policy {
        UpdateDownloadPolicy::Never if !manual => ReleaseStep::Skip,
        UpdateDownloadPolicy::Auto => ReleaseStep::Download,
        UpdateDownloadPolicy::Manual | UpdateDownloadPolicy::Never => ReleaseStep::Ask,
    }
}

fn first_scheduled_delay(policy: UpdateDownloadPolicy, channel: UpdateChannel) -> Option<Duration> {
    (policy != UpdateDownloadPolicy::Never).then(|| first_check_delay(channel))
}

fn delay_after_policy_change(
    previous: UpdateDownloadPolicy,
    next: UpdateDownloadPolicy,
    channel: UpdateChannel,
    delay: Option<Duration>,
) -> Option<Duration> {
    if next == UpdateDownloadPolicy::Never {
        None
    } else if previous == UpdateDownloadPolicy::Never {
        Some(first_check_delay(channel))
    } else {
        delay
    }
}

fn current_version() -> Result<Version, String> {
    Version::parse(env!("CARGO_PKG_VERSION")).map_err(|error| error.to_string())
}

async fn available_release(channel: UpdateChannel) -> Result<Option<Release>, String> {
    let current = current_version()?;
    let stable = installer::fetch_release(UpdateChannel::Stable, &current)
        .await
        .map(|candidate| {
            candidate.filter(|remote| {
                qualifies(&current, &remote.version, channel, UpdateChannel::Stable)
            })
        });

    if channel == UpdateChannel::Stable {
        return stable;
    }

    let nightly = installer::fetch_release(UpdateChannel::Nightly, &current)
        .await
        .map(|candidate| {
            candidate.filter(|remote| {
                qualifies(&current, &remote.version, channel, UpdateChannel::Nightly)
            })
        });

    select_available_release(stable, nightly)
}

fn select_available_release(
    stable: Result<Option<Release>, String>,
    nightly: Result<Option<Release>, String>,
) -> Result<Option<Release>, String> {
    match (stable, nightly) {
        (Ok(Some(stable)), Ok(Some(nightly))) => Ok(Some(if nightly.version > stable.version {
            nightly
        } else {
            stable
        })),
        (Ok(stable), Ok(nightly)) => Ok(stable.or(nightly)),
        (Ok(candidate), Err(error)) | (Err(error), Ok(candidate)) => {
            tracing::warn!("update check failed for one channel: {error}");
            Ok(candidate)
        }
        (Err(error), Err(_)) => Err(error),
    }
}

pub(crate) fn work_in_flight(cx: &mut App) -> bool {
    RecordingSession::work_in_flight(cx)
}

pub(crate) fn busy_message() -> &'static str {
    tr(BUSY_MESSAGE)
}

pub(crate) fn status(cx: &App) -> UpdateStatus {
    if cx.has_global::<UpdateScheduler>() {
        cx.global::<UpdateScheduler>().status.clone()
    } else {
        UpdateStatus::Idle
    }
}

pub(crate) fn current_indicator(cx: &App) -> Option<Indicator> {
    let scheduler = cx.try_global::<UpdateScheduler>()?;
    indicator(&scheduler.status, scheduler.dismissed)
}

pub(crate) fn channel(cx: &App) -> UpdateChannel {
    cx.try_global::<UpdateScheduler>().map_or_else(
        || GeneralSettings::load().update_channel,
        |scheduler| scheduler.channel,
    )
}

pub(crate) fn skipped_version(cx: &App) -> Option<Version> {
    let scheduler = cx.try_global::<UpdateScheduler>()?;
    scheduler.memory.skipped(scheduler.channel)
}

fn remember(cx: &mut App, change: impl Fn(&mut UpdateMemory) + Send + 'static) {
    change(&mut cx.global_mut::<UpdateScheduler>().memory);
    cx.background_executor()
        .spawn(async move { crate::store::update(|state| change(&mut state.updates)) })
        .detach();
}

pub(crate) fn current_version_label() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn set_status(status: UpdateStatus, cx: &mut App) {
    let scheduler = cx.global_mut::<UpdateScheduler>();
    if scheduler.status == status {
        return;
    }
    scheduler.status = status;
    refresh_surfaces(cx);
    maybe_notify(cx);
}

fn refresh_surfaces(cx: &mut App) {
    crate::app_windows::refresh_update_surfaces(cx);
    crate::update_panel::refresh(cx);
    crate::tray::refresh_menu(cx);
}

pub(crate) fn open_panel(cx: &mut App) {
    cx.defer(crate::update_panel::open);
}

pub(crate) fn check_manually(cx: &mut App) {
    if !cx.has_global::<UpdateScheduler>() {
        return;
    }
    open_panel(cx);
    if !matches!(
        status(cx),
        UpdateStatus::Idle
            | UpdateStatus::Checking
            | UpdateStatus::UpToDate
            | UpdateStatus::Failed(_)
    ) {
        return;
    }

    let scheduler = cx.global_mut::<UpdateScheduler>();
    if !scheduler.pending.request_manual(scheduler.manual_in_flight) {
        return;
    }
    let _ = scheduler.wake.try_send(());
}

pub(crate) fn download(cx: &mut App) {
    if let UpdateStatus::Available { release } = status(cx) {
        cx.global_mut::<UpdateScheduler>().dismissed = false;
        start_download(release, cx);
    }
}

pub(crate) fn cancel_download(cx: &mut App) {
    let UpdateStatus::Downloading { release, .. } = status(cx) else {
        return;
    };
    let scheduler = cx.global_mut::<UpdateScheduler>();
    scheduler.download_epoch = scheduler.download_epoch.wrapping_add(1);
    scheduler.download = None;
    set_status(UpdateStatus::Available { release }, cx);
}

pub(crate) fn remind_later(cx: &mut App) {
    if !cx.has_global::<UpdateScheduler>() {
        return;
    }
    cx.global_mut::<UpdateScheduler>().dismissed = true;
    refresh_surfaces(cx);
}

pub(crate) fn skip_version(cx: &mut App) {
    let UpdateStatus::Available { release } = status(cx) else {
        return;
    };
    let channel = channel(cx);
    let version = release.version.clone();
    remember(cx, move |memory| memory.skip(channel, &version));
    tracing::info!(version = %release.version, ?channel, "update version skipped");
    cx.global_mut::<UpdateScheduler>().dismissed = false;
    set_status(UpdateStatus::Idle, cx);
}

pub(crate) fn retry(cx: &mut App) {
    match status(cx) {
        UpdateStatus::Failed(UpdateFailure::Download { release, .. }) => {
            start_download(release, cx);
        }
        UpdateStatus::Failed(UpdateFailure::Check(_)) => check_manually(cx),
        _ => {}
    }
}

fn maybe_notify(cx: &mut App) {
    let scheduler = cx.global::<UpdateScheduler>();
    if scheduler.notice_waiting
        || !matches!(
            scheduler.status,
            UpdateStatus::Available { .. } | UpdateStatus::Ready { .. }
        )
        || !crate::notifications::enabled()
    {
        return;
    }
    let Some(notice) = notice(
        &scheduler.status,
        scheduler.download_policy,
        &scheduler.memory,
    ) else {
        return;
    };
    if work_in_flight(cx) {
        cx.global_mut::<UpdateScheduler>().notice_waiting = true;
        cx.spawn(async move |cx| {
            cx.background_executor().timer(BUSY_RETRY_DELAY).await;
            cx.update(|cx| {
                cx.global_mut::<UpdateScheduler>().notice_waiting = false;
                maybe_notify(cx);
            });
        })
        .detach();
        return;
    }
    send_notice(notice, cx);
}

fn send_notice(notice: Notice, cx: &mut App) {
    let version = notice.version().to_string();
    remember(cx, move |memory| memory.notified = Some(version.clone()));
    let body = match &notice {
        Notice::Found(version) => t!("Version {version} is available.", version = version),
        Notice::Ready(version) => t!(
            "Version {version} is ready. Restart to update.",
            version = version
        ),
    };
    let (clicked, clicks) = flume::bounded::<()>(1);
    cx.spawn(async move |cx| {
        if clicks.recv_async().await.is_ok() {
            cx.update(open_panel);
        }
    })
    .detach();
    cx.background_executor()
        .spawn(async move {
            if let Err(error) =
                crate::notifications::show_with_click("FrameEver", &body, move || {
                    let _ = clicked.try_send(());
                })
            {
                tracing::warn!("update notification failed: {error}");
            }
        })
        .detach();
}

pub(crate) fn update_download_policy_changed(policy: UpdateDownloadPolicy, cx: &mut App) {
    if !cx.has_global::<UpdateScheduler>() {
        return;
    }
    cx.global_mut::<UpdateScheduler>().download_policy = policy;
    let _ = cx.global::<UpdateScheduler>().wake.try_send(());
}

pub(crate) fn update_channel_changed(channel: UpdateChannel, cx: &mut App) {
    if !cx.has_global::<UpdateScheduler>() {
        return;
    }

    drop_pending_update_from_previous_channel(cx);
    let scheduler = cx.global_mut::<UpdateScheduler>();
    scheduler.channel = channel;
    scheduler.pending.request_channel(channel);
    let _ = scheduler.wake.try_send(());
}

fn channel_switch_drops_pending_package(status: &UpdateStatus) -> bool {
    !matches!(
        status,
        UpdateStatus::Idle | UpdateStatus::Installing { .. } | UpdateStatus::Installed { .. }
    )
}

fn drop_pending_update_from_previous_channel(cx: &mut App) {
    if !channel_switch_drops_pending_package(&status(cx)) {
        return;
    }
    let scheduler = cx.global_mut::<UpdateScheduler>();
    scheduler.download_epoch = scheduler.download_epoch.wrapping_add(1);
    scheduler.download = None;
    scheduler.staged = None;
    scheduler.dismissed = false;
    set_status(UpdateStatus::Idle, cx);
}

fn first_check_delay(channel: UpdateChannel) -> Duration {
    match channel {
        UpdateChannel::Stable => STABLE_FIRST_CHECK_DELAY,
        UpdateChannel::Nightly => NIGHTLY_FIRST_CHECK_DELAY,
    }
}

fn next_check_delay(channel: UpdateChannel) -> Option<Duration> {
    (channel == UpdateChannel::Nightly).then_some(NIGHTLY_CHECK_INTERVAL)
}

fn finish_manual_check(cx: &mut App, manual: bool) {
    if manual {
        cx.global_mut::<UpdateScheduler>().manual_in_flight = false;
    }
}

fn shows_checking(manual: bool, status: &UpdateStatus) -> bool {
    manual || matches!(status, UpdateStatus::Idle | UpdateStatus::UpToDate)
}

pub(crate) fn schedule_startup_check(cx: &mut App) {
    let (wake, requests) = flume::bounded(1);
    let settings = GeneralSettings::load();
    cx.set_global(UpdateScheduler {
        wake,
        pending: PendingUpdateRequests::default(),
        manual_in_flight: false,
        status: UpdateStatus::Idle,
        download_policy: settings.update_download_policy,
        channel: settings.update_channel,
        staged: None,
        download: None,
        download_epoch: 0,
        dismissed: false,
        notice_waiting: false,
        memory: crate::store::load().updates,
    });
    #[cfg(debug_assertions)]
    preview_from_env(cx);

    cx.spawn(async move |cx| {
        let mut channel = cx.update(|cx| cx.global::<UpdateScheduler>().channel);
        let mut policy = cx.update(|cx| cx.global::<UpdateScheduler>().download_policy);
        let mut delay = first_scheduled_delay(policy, channel).filter(|_| !cfg!(debug_assertions));

        loop {
            let signaled = match delay {
                Some(delay) => {
                    let timer = cx.background_executor().timer(delay);
                    let request = requests.recv_async();
                    futures_util::pin_mut!(timer, request);
                    match select(timer, request).await {
                        Either::Left(_) => false,
                        Either::Right((Ok(()), _)) => true,
                        Either::Right((Err(_), _)) => return,
                    }
                }
                None => {
                    if requests.recv_async().await.is_err() {
                        return;
                    }
                    true
                }
            };

            let request = cx.update(|cx| {
                let scheduler = cx.global_mut::<UpdateScheduler>();
                std::mem::take(&mut scheduler.pending)
            });

            let latest_policy = cx.update(|cx| cx.global::<UpdateScheduler>().download_policy);
            if latest_policy != policy {
                delay = delay_after_policy_change(policy, latest_policy, channel, delay);
                policy = latest_policy;
            }

            if signaled && request == PendingUpdateRequests::default() {
                continue;
            }

            if let Some(next_channel) = request.channel {
                channel = next_channel;
                cx.update(drop_pending_update_from_previous_channel);
            }

            let manual = request.manual;
            if policy == UpdateDownloadPolicy::Never && !manual {
                delay = None;
                continue;
            }
            if cfg!(debug_assertions) && !manual {
                delay = None;
                continue;
            }

            if manual {
                cx.update(|cx| cx.global_mut::<UpdateScheduler>().manual_in_flight = true);
            }

            delay = next_check_delay(channel);

            if cx.update(|cx| !status(cx).accepts_check()) {
                cx.update(|cx| finish_manual_check(cx, manual));
                continue;
            }

            cx.update(|cx| {
                if shows_checking(manual, &status(cx)) {
                    set_status(UpdateStatus::Checking, cx);
                }
            });

            let result = match cx
                .update(|cx| gpui_tokio::Tokio::spawn(cx, available_release(channel)))
                .await
            {
                Ok(result) => result,
                Err(error) => Err(error.to_string()),
            };

            let superseded = cx.update(|cx| {
                let scheduler = cx.global_mut::<UpdateScheduler>();
                let channel_changed = scheduler.pending.channel.is_some();
                if channel_changed && manual {
                    scheduler.pending.manual = true;
                    scheduler.manual_in_flight = false;
                }
                channel_changed || (!manual && scheduler.pending.manual)
            });
            if superseded {
                cx.update(|cx| set_status(UpdateStatus::Idle, cx));
                continue;
            }

            let release = match result {
                Ok(Some(release)) => release,
                Ok(None) => {
                    cx.update(|cx| {
                        set_status(
                            if manual {
                                UpdateStatus::UpToDate
                            } else {
                                UpdateStatus::Idle
                            },
                            cx,
                        );
                        finish_manual_check(cx, manual);
                    });
                    continue;
                }
                Err(error) => {
                    tracing::warn!("update check failed: {error}");
                    cx.update(|cx| {
                        if manual {
                            set_status(
                                UpdateStatus::Failed(UpdateFailure::Check(t!(
                                    "Couldn't check for updates: {error}",
                                    error = error
                                ))),
                                cx,
                            );
                        } else if status(cx) == UpdateStatus::Checking {
                            set_status(UpdateStatus::Idle, cx);
                        }
                        finish_manual_check(cx, manual);
                    });
                    continue;
                }
            };

            let skipped = cx.update(|cx| cx.global::<UpdateScheduler>().memory.skipped(channel));
            if !offered(&release.version, skipped.as_ref(), manual) {
                cx.update(|cx| set_status(UpdateStatus::Idle, cx));
                continue;
            }

            cx.update(|cx| {
                if std::mem::take(&mut cx.global_mut::<UpdateScheduler>().dismissed) {
                    refresh_surfaces(cx);
                }
                finish_manual_check(cx, manual);
                let latest_policy = cx.global::<UpdateScheduler>().download_policy;
                match release_step(latest_policy, manual) {
                    ReleaseStep::Skip => set_status(UpdateStatus::Idle, cx),
                    ReleaseStep::Download => start_download(release, cx),
                    ReleaseStep::Ask => set_status(UpdateStatus::Available { release }, cx),
                }
            });
        }
    })
    .detach();
}

#[cfg(debug_assertions)]
fn preview_from_env(cx: &mut App) {
    let Ok(spec) = std::env::var("FRAMEEVER_GPUI_UPDATE_PREVIEW") else {
        return;
    };
    let release = Release {
        version: Version::new(9, 9, 9),
        url: String::new(),
        signature: String::new(),
        notes: Some(
            "- Library: open an export with the default app\n- Compression shows its queue in the library toolbar\n- Fixed: update prompt hidden behind other windows".into(),
        ),
    };
    let status = match spec.as_str() {
        "available" => UpdateStatus::Available { release },
        "downloading" => UpdateStatus::Downloading {
            release,
            fraction: Some(0.42),
        },
        "ready" => UpdateStatus::Ready { release },
        "installing" => UpdateStatus::Installing {
            version: release.version,
        },
        "failed" => UpdateStatus::Failed(UpdateFailure::Download {
            release,
            message: "Couldn't download the update. Check your connection and try again.".into(),
        }),
        "current" => UpdateStatus::UpToDate,
        _ => return,
    };
    cx.global_mut::<UpdateScheduler>().status = status;
}

fn start_download(release: Release, cx: &mut App) {
    let kind = match installer::update_platform()
        .and_then(|platform| ArtifactKind::for_platform(&platform))
    {
        Ok(kind) => kind,
        Err(error) => {
            update_failed(release, error, cx);
            return;
        }
    };

    set_status(
        UpdateStatus::Downloading {
            release: release.clone(),
            fraction: None,
        },
        cx,
    );
    let download_epoch = cx.global::<UpdateScheduler>().download_epoch;

    let (progress_sender, progress) = flume::bounded::<Option<f32>>(1);
    let download_release = release.clone();
    let download = gpui_tokio::Tokio::spawn(cx, async move {
        let mut reported = None::<u16>;
        installer::download(kind, &download_release, move |done, total| {
            let fraction = total
                .filter(|total| *total > 0)
                .map(|total| (done as f64 / total as f64).clamp(0., 1.) as f32);
            let step = fraction.map(|fraction| (fraction * 100.) as u16);
            if step != reported {
                reported = step;
                let _ = progress_sender.try_send(fraction);
            }
        })
        .await
    });

    let task = cx.spawn(async move |cx| {
        let progress_release = release.clone();
        let progress_epoch = download_epoch;
        let progress_updates = cx.spawn(async move |cx| {
            while let Ok(fraction) = progress.recv_async().await {
                cx.update(|cx| {
                    if cx.global::<UpdateScheduler>().download_epoch != progress_epoch {
                        return;
                    }
                    set_status(
                        UpdateStatus::Downloading {
                            release: progress_release.clone(),
                            fraction,
                        },
                        cx,
                    )
                });
            }
        });

        let result = match download.await {
            Ok(result) => result,
            Err(error) => Err(error.to_string()),
        };
        drop(progress_updates);

        cx.update(|cx| {
            if cx.global::<UpdateScheduler>().download_epoch != download_epoch {
                return;
            }
            cx.global_mut::<UpdateScheduler>().download = None;
            match result {
                Ok(package) => {
                    tracing::info!(version = %release.version, path = %package.display(), "update downloaded and verified");
                    cx.global_mut::<UpdateScheduler>().staged =
                        Some(StagedUpdate { kind, package });
                    set_status(UpdateStatus::Ready { release }, cx);
                }
                Err(error) => {
                    tracing::warn!(version = %release.version, "update download failed: {error}");
                    let message = t!(
                        "Couldn't download version {version}: {error}",
                        version = release.version,
                        error = error
                    );
                    update_failed(release, message, cx);
                }
            }
        });
    });
    cx.global_mut::<UpdateScheduler>().download = Some(task);
}

fn update_failed(release: Release, message: String, cx: &mut App) {
    set_status(
        UpdateStatus::Failed(UpdateFailure::Download { release, message }),
        cx,
    );
}

pub(crate) fn install_and_relaunch(cx: &mut App) {
    if !cx.has_global::<UpdateScheduler>() {
        return;
    }
    let status = status(cx);
    if !matches!(
        status,
        UpdateStatus::Ready { .. } | UpdateStatus::Installed { .. }
    ) {
        return;
    }
    if work_in_flight(cx) {
        open_panel(cx);
        return;
    }
    if let Err(error) = crate::app_windows::flush_pending_editor_saves(cx) {
        cx.spawn(async move |_| {
            crate::platform::alert_dialog(t!("FrameEver is still open"), &error);
        })
        .detach();
        return;
    }
    let release = match status {
        UpdateStatus::Installed {
            version,
            executable,
        } => {
            relaunch_into(&version, &executable, cx);
            return;
        }
        UpdateStatus::Ready { release } => release,
        _ => return,
    };
    let Some((package, kind)) = cx
        .global::<UpdateScheduler>()
        .staged
        .as_ref()
        .map(|staged| (staged.package.clone(), staged.kind))
    else {
        return;
    };

    let version = release.version.clone();
    set_status(
        UpdateStatus::Installing {
            version: version.clone(),
        },
        cx,
    );
    cx.spawn(async move |cx| {
        let applied = cx
            .background_executor()
            .spawn(async move { apply(&package, kind) })
            .await;
        cx.update(|cx| match applied {
            Ok(Some(executable)) => restart_after_install(version, executable, cx),
            Ok(None) => {
                tracing::info!(%version, "update installer started; quitting");
                crate::menus::quit(crate::menus::QuitSource::UpdateInstaller, cx);
            }
            Err(error) => {
                tracing::error!(%version, "update install failed: {error}");
                cx.global_mut::<UpdateScheduler>().staged = None;
                let message = t!(
                    "Couldn't install version {version}: {error}",
                    version = version,
                    error = error
                );
                update_failed(release, message, cx);
            }
        });
    })
    .detach();
}

fn restart_after_install(version: Version, executable: PathBuf, cx: &mut App) {
    let deferred = if work_in_flight(cx) {
        tracing::info!(%version, "update installed; waiting for in-flight work before restarting");
        true
    } else if let Err(error) = crate::app_windows::flush_pending_editor_saves(cx) {
        tracing::warn!(%version, "update installed; editor changes are still unsaved: {error}");
        true
    } else {
        false
    };
    if deferred {
        set_status(
            UpdateStatus::Installed {
                version,
                executable,
            },
            cx,
        );
        return;
    }
    relaunch_into(&version, &executable, cx);
}

fn relaunch_into(version: &Version, executable: &std::path::Path, cx: &mut App) {
    tracing::info!(%version, "update installed; relaunching");
    if let Err(error) = crate::permissions::relaunch_executable(executable, cx) {
        tracing::error!(%error, "could not relaunch after updating");
        crate::menus::quit(crate::menus::QuitSource::UpdateRelaunchFailed, cx);
    }
}

fn apply(package: &std::path::Path, kind: ArtifactKind) -> Result<Option<PathBuf>, String> {
    installer::check_artifact_kind(package, kind)?;
    apply_package(package, kind)
}

#[cfg(target_os = "macos")]
fn apply_package(
    package: &std::path::Path,
    _kind: ArtifactKind,
) -> Result<Option<PathBuf>, String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let bundle = executable
        .ancestors()
        .find(|path| path.extension().is_some_and(|extension| extension == "app"))
        .ok_or_else(|| t!("This copy of FrameEver isn't running from an app bundle").to_string())?;
    installer::install_app_bundle(package, bundle)?;
    Ok(Some(executable))
}

#[cfg(windows)]
fn apply_package(
    package: &std::path::Path,
    _kind: ArtifactKind,
) -> Result<Option<PathBuf>, String> {
    installer::launch_installer(package, &["--update"])?;
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(value: &str) -> Version {
        Version::parse(value).unwrap()
    }

    fn release(value: &str) -> Release {
        Release {
            version: version(value),
            url: format!(
                "https://github.com/AniBullet/FrameEver/releases/download/v{value}/setup.exe"
            ),
            signature: "c2ln".to_string(),
            notes: None,
        }
    }

    #[test]
    fn stable_channel_accepts_newer_versions_and_explicit_prerelease_downgrades() {
        assert!(qualifies(
            &version("0.6.0"),
            &version("0.6.1"),
            UpdateChannel::Stable,
            UpdateChannel::Stable,
        ));
        assert!(qualifies(
            &version("0.6.1-nightly.4"),
            &version("0.6.0"),
            UpdateChannel::Stable,
            UpdateChannel::Stable,
        ));
        assert!(!qualifies(
            &version("0.6.1"),
            &version("0.6.0"),
            UpdateChannel::Stable,
            UpdateChannel::Stable,
        ));
    }

    #[test]
    fn nightly_channel_never_uses_the_stable_downgrade_rule() {
        assert!(!qualifies(
            &version("0.6.1-nightly.4"),
            &version("0.6.0"),
            UpdateChannel::Nightly,
            UpdateChannel::Stable,
        ));
        assert!(qualifies(
            &version("0.6.1-nightly.4"),
            &version("0.6.1-nightly.5"),
            UpdateChannel::Nightly,
            UpdateChannel::Nightly,
        ));
    }

    #[test]
    fn nightly_channel_survives_individual_endpoint_failures() {
        assert_eq!(
            select_available_release(Err("stable unavailable".into()), Ok(Some(release("0.6.1"))))
                .unwrap(),
            Some(release("0.6.1")),
        );
        assert_eq!(
            select_available_release(
                Ok(Some(release("0.6.2"))),
                Err("nightly unavailable".into())
            )
            .unwrap(),
            Some(release("0.6.2")),
        );
        assert_eq!(
            select_available_release(
                Err("stable unavailable".into()),
                Err("nightly unavailable".into()),
            ),
            Err("stable unavailable".into()),
        );
    }

    #[test]
    fn nightly_channel_prefers_the_newest_successful_version() {
        assert_eq!(
            select_available_release(
                Ok(Some(release("0.6.1"))),
                Ok(Some(release("0.6.2-nightly.4"))),
            )
            .unwrap(),
            Some(release("0.6.2-nightly.4")),
        );
        assert_eq!(
            select_available_release(
                Ok(Some(release("0.6.3"))),
                Ok(Some(release("0.6.3-nightly.9"))),
            )
            .unwrap(),
            Some(release("0.6.3")),
        );
    }

    #[test]
    fn check_cadence_preserves_each_update_channel_contract() {
        assert_eq!(
            first_check_delay(UpdateChannel::Stable),
            Duration::from_secs(10)
        );
        assert_eq!(
            first_check_delay(UpdateChannel::Nightly),
            Duration::from_secs(60)
        );
        assert_eq!(next_check_delay(UpdateChannel::Stable), None);
        assert_eq!(
            next_check_delay(UpdateChannel::Nightly),
            Some(Duration::from_secs(2 * 60 * 60))
        );
    }

    #[test]
    fn a_release_follows_the_download_policy() {
        use UpdateDownloadPolicy::{Auto, Manual, Never};

        assert_eq!(release_step(Auto, false), ReleaseStep::Download);
        assert_eq!(release_step(Auto, true), ReleaseStep::Download);
        assert_eq!(release_step(Manual, false), ReleaseStep::Ask);
        assert_eq!(release_step(Manual, true), ReleaseStep::Ask);
        assert_eq!(release_step(Never, false), ReleaseStep::Skip);
        assert_eq!(release_step(Never, true), ReleaseStep::Ask);
    }

    #[test]
    fn scheduled_checks_stop_on_never_and_resume_when_it_is_left() {
        use UpdateDownloadPolicy::{Auto, Manual, Never};

        let stable = UpdateChannel::Stable;
        assert_eq!(first_scheduled_delay(Never, stable), None);
        assert_eq!(
            first_scheduled_delay(Manual, stable),
            Some(first_check_delay(stable))
        );

        let soon = Some(Duration::from_secs(3));
        assert_eq!(delay_after_policy_change(Auto, Never, stable, soon), None);
        assert_eq!(
            delay_after_policy_change(Never, Manual, stable, None),
            Some(first_check_delay(stable))
        );
        assert_eq!(delay_after_policy_change(Manual, Auto, stable, soon), soon);
        assert_eq!(delay_after_policy_change(Auto, Manual, stable, None), None);
    }

    #[test]
    fn manual_checks_coalesce_while_pending_or_running() {
        let mut requests = PendingUpdateRequests::default();

        assert!(requests.request_manual(false));
        assert!(!requests.request_manual(false));

        requests.manual = false;
        assert!(!requests.request_manual(true));
        assert!(requests.request_manual(false));
    }

    #[test]
    fn channel_switch_drops_what_the_other_channel_offered_and_keeps_an_install() {
        let nightly = version("0.6.1-nightly.4");
        for dropped in [
            UpdateStatus::Checking,
            UpdateStatus::UpToDate,
            UpdateStatus::Available {
                release: release("0.6.1-nightly.4"),
            },
            UpdateStatus::Downloading {
                release: release("0.6.1-nightly.4"),
                fraction: Some(0.4),
            },
            UpdateStatus::Ready {
                release: release("0.6.1-nightly.4"),
            },
            UpdateStatus::Failed(UpdateFailure::Check("offline".into())),
        ] {
            assert!(
                channel_switch_drops_pending_package(&dropped),
                "{dropped:?}"
            );
        }
        assert!(!channel_switch_drops_pending_package(
            &UpdateStatus::Installing { version: nightly }
        ));
        assert!(!channel_switch_drops_pending_package(
            &UpdateStatus::Installed {
                version: version("0.6.1"),
                executable: PathBuf::from("/Applications/FrameEver.app"),
            }
        ));
        assert!(!channel_switch_drops_pending_package(&UpdateStatus::Idle));
    }

    #[test]
    fn a_scheduled_check_keeps_an_offer_on_screen_while_it_runs() {
        assert!(shows_checking(true, &UpdateStatus::Idle));
        assert!(shows_checking(
            true,
            &UpdateStatus::Available {
                release: release("1.2.0")
            }
        ));
        assert!(shows_checking(false, &UpdateStatus::Idle));
        assert!(shows_checking(false, &UpdateStatus::UpToDate));
        assert!(!shows_checking(
            false,
            &UpdateStatus::Available {
                release: release("1.2.0")
            }
        ));
    }

    #[test]
    fn channel_changes_coalesce_without_losing_manual_checks() {
        let mut requests = PendingUpdateRequests::default();

        assert!(requests.request_manual(false));
        requests.request_channel(UpdateChannel::Nightly);
        requests.request_channel(UpdateChannel::Stable);

        assert_eq!(
            requests,
            PendingUpdateRequests {
                manual: true,
                channel: Some(UpdateChannel::Stable),
            }
        );
    }

    #[test]
    fn recording_exports_and_uploads_remain_update_blockers_until_finished() {
        use crate::editor_export::ExportPhase;

        for phase in [
            ExportPhase::Starting,
            ExportPhase::Rendering,
            ExportPhase::Copying,
        ] {
            assert!(phase.is_busy());
        }

        for phase in [ExportPhase::Idle, ExportPhase::Done, ExportPhase::Failed] {
            assert!(!phase.is_busy());
        }
    }
}
