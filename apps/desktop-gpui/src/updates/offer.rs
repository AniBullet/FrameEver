use std::path::PathBuf;

use semver::Version;
use serde::{Deserialize, Serialize};

use crate::{
    installer::Release,
    store::{UpdateChannel, UpdateDownloadPolicy},
};

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) enum UpdateStatus {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Available {
        release: Release,
    },
    Downloading {
        release: Release,
        fraction: Option<f32>,
    },
    Ready {
        release: Release,
    },
    Installing {
        version: Version,
    },
    Installed {
        version: Version,
        executable: PathBuf,
    },
    Failed(UpdateFailure),
}

impl UpdateStatus {
    pub(crate) fn accepts_check(&self) -> bool {
        matches!(
            self,
            Self::Idle | Self::UpToDate | Self::Available { .. } | Self::Failed(_)
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum UpdateFailure {
    Check(String),
    Download { release: Release, message: String },
}

impl UpdateFailure {
    pub(crate) fn message(&self) -> &str {
        match self {
            Self::Check(message) | Self::Download { message, .. } => message,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Indicator {
    Available(Version),
    Downloading {
        version: Version,
        percent: Option<u8>,
    },
    Ready(Version),
    Installing(Version),
    Failed,
}

impl Indicator {
    pub(crate) fn wants_attention(&self) -> bool {
        matches!(self, Self::Available(_) | Self::Ready(_) | Self::Failed)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Notice {
    Found(Version),
    Ready(Version),
}

impl Notice {
    pub(crate) fn version(&self) -> &Version {
        match self {
            Self::Found(version) | Self::Ready(version) => version,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateMemory {
    #[serde(default)]
    pub skipped_stable: Option<String>,
    #[serde(default)]
    pub skipped_nightly: Option<String>,
    #[serde(default)]
    pub notified: Option<String>,
}

impl UpdateMemory {
    pub(crate) fn skipped(&self, channel: UpdateChannel) -> Option<Version> {
        let skipped = match channel {
            UpdateChannel::Stable => &self.skipped_stable,
            UpdateChannel::Nightly => &self.skipped_nightly,
        };
        skipped
            .as_deref()
            .and_then(|value| Version::parse(value).ok())
    }

    pub(crate) fn skip(&mut self, channel: UpdateChannel, version: &Version) {
        let slot = match channel {
            UpdateChannel::Stable => &mut self.skipped_stable,
            UpdateChannel::Nightly => &mut self.skipped_nightly,
        };
        *slot = Some(version.to_string());
    }

    pub(crate) fn was_notified(&self, version: &Version) -> bool {
        self.notified.as_deref() == Some(version.to_string().as_str())
    }
}

pub(crate) fn offered(version: &Version, skipped: Option<&Version>, manual: bool) -> bool {
    manual || skipped != Some(version)
}

pub(crate) fn indicator(status: &UpdateStatus, dismissed: bool) -> Option<Indicator> {
    let indicator = match status {
        UpdateStatus::Downloading { release, fraction } => {
            return Some(Indicator::Downloading {
                version: release.version.clone(),
                percent: fraction.map(|fraction| (fraction.clamp(0., 1.) * 100.).round() as u8),
            });
        }
        UpdateStatus::Installing { version } => {
            return Some(Indicator::Installing(version.clone()));
        }
        UpdateStatus::Available { release } => Indicator::Available(release.version.clone()),
        UpdateStatus::Ready { release } => Indicator::Ready(release.version.clone()),
        UpdateStatus::Installed { version, .. } => Indicator::Ready(version.clone()),
        UpdateStatus::Failed(UpdateFailure::Download { .. }) => Indicator::Failed,
        UpdateStatus::Failed(UpdateFailure::Check(_)) => return None,
        UpdateStatus::Idle | UpdateStatus::Checking | UpdateStatus::UpToDate => return None,
    };
    (!dismissed).then_some(indicator)
}

pub(crate) fn notice(
    status: &UpdateStatus,
    policy: UpdateDownloadPolicy,
    memory: &UpdateMemory,
) -> Option<Notice> {
    let notice = match (status, policy) {
        (UpdateStatus::Ready { release }, UpdateDownloadPolicy::Auto) => {
            Notice::Ready(release.version.clone())
        }
        (
            UpdateStatus::Available { release },
            UpdateDownloadPolicy::Manual | UpdateDownloadPolicy::Never,
        ) => Notice::Found(release.version.clone()),
        _ => return None,
    };
    (!memory.was_notified(notice.version())).then_some(notice)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(value: &str) -> Version {
        Version::parse(value).unwrap()
    }

    #[test]
    fn a_scheduled_check_hides_the_skipped_version_and_a_manual_one_shows_it() {
        let skipped = v("1.2.0");
        assert!(!offered(&v("1.2.0"), Some(&skipped), false));
        assert!(offered(&v("1.2.0"), Some(&skipped), true));
        assert!(offered(&v("1.3.0"), Some(&skipped), false));
        assert!(offered(&v("1.2.0"), None, false));
    }

    #[test]
    fn skipped_versions_are_remembered_per_channel() {
        let mut memory = UpdateMemory::default();
        memory.skip(UpdateChannel::Stable, &v("1.2.0"));
        assert_eq!(memory.skipped(UpdateChannel::Stable), Some(v("1.2.0")));
        assert_eq!(memory.skipped(UpdateChannel::Nightly), None);
        memory.skip(UpdateChannel::Nightly, &v("1.3.0-beta.1"));
        assert_eq!(memory.skipped(UpdateChannel::Stable), Some(v("1.2.0")));
        assert_eq!(
            memory.skipped(UpdateChannel::Nightly),
            Some(v("1.3.0-beta.1"))
        );
    }

    fn release(value: &str) -> Release {
        Release {
            version: v(value),
            url: String::new(),
            signature: String::new(),
            notes: None,
        }
    }

    #[test]
    fn remind_me_later_hides_what_asks_for_attention_but_not_work_in_progress() {
        let version = v("1.2.0");
        let available = UpdateStatus::Available {
            release: release("1.2.0"),
        };
        let ready = UpdateStatus::Ready {
            release: release("1.2.0"),
        };
        let failed = UpdateStatus::Failed(UpdateFailure::Download {
            release: release("1.2.0"),
            message: "offline".into(),
        });
        let downloading = UpdateStatus::Downloading {
            release: release("1.2.0"),
            fraction: Some(0.416),
        };
        let installing = UpdateStatus::Installing {
            version: version.clone(),
        };

        assert_eq!(
            indicator(&available, false),
            Some(Indicator::Available(version.clone()))
        );
        assert_eq!(
            indicator(&ready, false),
            Some(Indicator::Ready(version.clone()))
        );
        assert_eq!(indicator(&failed, false), Some(Indicator::Failed));
        assert_eq!(indicator(&available, true), None);
        assert_eq!(indicator(&ready, true), None);
        assert_eq!(indicator(&failed, true), None);
        assert_eq!(
            indicator(&downloading, true),
            Some(Indicator::Downloading {
                version: version.clone(),
                percent: Some(42)
            })
        );
        assert_eq!(
            indicator(&installing, true),
            Some(Indicator::Installing(version))
        );
        for quiet in [
            UpdateStatus::Idle,
            UpdateStatus::Checking,
            UpdateStatus::UpToDate,
            UpdateStatus::Failed(UpdateFailure::Check("offline".into())),
        ] {
            assert_eq!(indicator(&quiet, false), None);
        }
    }

    #[test]
    fn only_available_ready_and_failed_put_a_dot_and_a_count_up() {
        let version = v("1.2.0");
        assert!(Indicator::Available(version.clone()).wants_attention());
        assert!(Indicator::Ready(version.clone()).wants_attention());
        assert!(Indicator::Failed.wants_attention());
        assert!(
            !Indicator::Downloading {
                version: version.clone(),
                percent: None
            }
            .wants_attention()
        );
        assert!(!Indicator::Installing(version).wants_attention());
    }

    #[test]
    fn each_policy_sends_one_notification_per_version() {
        use UpdateDownloadPolicy::{Auto, Manual, Never};
        let version = v("1.2.0");
        let available = UpdateStatus::Available {
            release: release("1.2.0"),
        };
        let ready = UpdateStatus::Ready {
            release: release("1.2.0"),
        };
        let fresh = UpdateMemory::default();

        assert_eq!(
            notice(&available, Manual, &fresh),
            Some(Notice::Found(version.clone()))
        );
        assert_eq!(
            notice(&available, Never, &fresh),
            Some(Notice::Found(version.clone()))
        );
        assert_eq!(notice(&available, Auto, &fresh), None);
        assert_eq!(notice(&ready, Auto, &fresh), Some(Notice::Ready(version)));
        assert_eq!(notice(&ready, Manual, &fresh), None);

        let notified = UpdateMemory {
            notified: Some("1.2.0".into()),
            ..UpdateMemory::default()
        };
        assert_eq!(notice(&available, Manual, &notified), None);
        assert_eq!(notice(&ready, Auto, &notified), None);
        assert_eq!(
            notice(
                &UpdateStatus::Available {
                    release: release("1.3.0")
                },
                Manual,
                &notified
            ),
            Some(Notice::Found(v("1.3.0")))
        );
    }

    #[test]
    fn only_settled_states_take_a_new_check() {
        assert!(UpdateStatus::Idle.accepts_check());
        assert!(UpdateStatus::UpToDate.accepts_check());
        assert!(
            UpdateStatus::Available {
                release: release("1.2.0")
            }
            .accepts_check()
        );
        assert!(UpdateStatus::Failed(UpdateFailure::Check("offline".into())).accepts_check());
        assert!(!UpdateStatus::Checking.accepts_check());
        assert!(
            !UpdateStatus::Downloading {
                release: release("1.2.0"),
                fraction: None
            }
            .accepts_check()
        );
        assert!(
            !UpdateStatus::Ready {
                release: release("1.2.0")
            }
            .accepts_check()
        );
    }
}
