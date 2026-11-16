use gpui::App;

use crate::session::Phase;

pub(crate) fn holds_off_idle_sleep(phase: Phase) -> bool {
    phase != Phase::Idle
}

pub fn install(cx: &mut App) {
    #[cfg(windows)]
    windows::install(cx);
    #[cfg(not(windows))]
    let _ = cx;
}

#[cfg(windows)]
mod windows {
    use gpui::{App, Global};
    use windows_sys::Win32::System::Power::{
        ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED, EXECUTION_STATE,
        SetThreadExecutionState,
    };

    use super::holds_off_idle_sleep;
    use crate::session::RecordingSession;

    pub(super) fn execution_state(hold: bool) -> EXECUTION_STATE {
        if hold {
            ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED
        } else {
            ES_CONTINUOUS
        }
    }

    #[derive(Default)]
    struct IdleSleepHold(bool);

    impl Global for IdleSleepHold {}

    impl IdleSleepHold {
        fn hold(&mut self, wanted: bool) {
            if self.0 == wanted {
                return;
            }
            if unsafe { SetThreadExecutionState(execution_state(wanted)) } == 0 {
                tracing::warn!(wanted, "could not change the idle sleep hold");
                return;
            }
            tracing::info!(wanted, "idle sleep hold changed");
            self.0 = wanted;
        }
    }

    pub(super) fn install(cx: &mut App) {
        let session = RecordingSession::global(cx);
        cx.set_global(IdleSleepHold::default());
        cx.observe(&session, |session, cx| {
            let wanted = holds_off_idle_sleep(session.read(cx).phase);
            cx.global_mut::<IdleSleepHold>().hold(wanted);
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_sleep_is_held_off_for_the_whole_recording_lifecycle() {
        assert!(!holds_off_idle_sleep(Phase::Idle));
        for phase in [
            Phase::Starting,
            Phase::Recording { paused: false },
            Phase::Recording { paused: true },
            Phase::Stopping,
        ] {
            assert!(holds_off_idle_sleep(phase), "{phase:?}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn a_held_recording_keeps_the_system_and_display_awake_until_released() {
        assert_eq!(windows::execution_state(true), 0x8000_0003);
        assert_eq!(windows::execution_state(false), 0x8000_0000);
    }
}
