use super::InputKind;
use crate::i18n::t;

pub(super) trait SwitchSteps {
    type Device;
    type Attached;

    async fn detach(&mut self) -> anyhow::Result<()>;
    async fn attach(&mut self, device: &Self::Device) -> anyhow::Result<Self::Attached>;
    fn is_current(&mut self) -> bool;
}

#[derive(Debug)]
pub(super) enum Switched<T> {
    Attached(T),
    Restored { attached: T, cause: anyhow::Error },
    Unchanged { cause: anyhow::Error },
    LeftOff { cause: anyhow::Error },
    Abandoned,
}

impl<T> Switched<T> {
    pub(super) fn ended_with<'a, D>(&self, requested: &'a D, previous: &'a D) -> &'a D {
        match self {
            Self::Attached(_) => requested,
            _ => previous,
        }
    }
}

pub(super) async fn switch_or_restore<S: SwitchSteps>(
    steps: &mut S,
    requested: &S::Device,
    previous: &S::Device,
) -> Switched<S::Attached> {
    if let Err(cause) = steps.detach().await {
        return Switched::Unchanged { cause };
    }
    if !steps.is_current() {
        return Switched::Abandoned;
    }
    let cause = match steps.attach(requested).await {
        Ok(attached) => return Switched::Attached(attached),
        Err(cause) => cause,
    };
    if !steps.is_current() {
        return Switched::Abandoned;
    }
    match steps.attach(previous).await {
        Ok(attached) => Switched::Restored { attached, cause },
        Err(error) => {
            tracing::error!("restoring the previous recording input failed: {error:#}");
            Switched::LeftOff { cause }
        }
    }
}

#[derive(Debug)]
pub(crate) struct InputFailure {
    pub(crate) device: String,
    pub(crate) cause: String,
    pub(crate) off: bool,
    pub(crate) attempt: u64,
}

impl InputFailure {
    fn message(&self, kind: InputKind, paused: Option<bool>) -> String {
        let mut message: String = match paused {
            Some(true) => t!(
                "Couldn't switch to {device}. The recording stays paused.",
                device = self.device
            ),
            Some(false) => t!(
                "Couldn't switch to {device}. The recording continues.",
                device = self.device
            ),
            None => t!("Couldn't switch to {device}.", device = self.device),
        };
        if self.off {
            message.push('\n');
            message.push_str(match kind {
                InputKind::Microphone => {
                    t!("The microphone is off. Choose one in Recording options.")
                }
                InputKind::Camera => t!("The camera is off. Choose one in Recording options."),
            });
        }
        message
    }
}

#[derive(Default)]
pub(crate) struct InputFailures {
    microphone: Option<InputFailure>,
    camera: Option<InputFailure>,
}

impl InputFailures {
    fn get(&self, kind: InputKind) -> Option<&InputFailure> {
        match kind {
            InputKind::Microphone => self.microphone.as_ref(),
            InputKind::Camera => self.camera.as_ref(),
        }
    }

    pub(crate) fn set(&mut self, kind: InputKind, failure: Option<InputFailure>) {
        match kind {
            InputKind::Microphone => self.microphone = failure,
            InputKind::Camera => self.camera = failure,
        }
    }

    pub(crate) fn is_off(&self, kind: InputKind) -> bool {
        self.get(kind).is_some_and(|failure| failure.off)
    }

    pub(crate) fn messages(&self, paused: Option<bool>) -> Vec<String> {
        [InputKind::Microphone, InputKind::Camera]
            .into_iter()
            .filter_map(|kind| Some(self.get(kind)?.message(kind, paused)))
            .collect()
    }

    pub(crate) fn attempts(&self) -> Vec<u64> {
        [&self.microphone, &self.camera]
            .into_iter()
            .flatten()
            .map(|failure| failure.attempt)
            .collect()
    }

    pub(crate) fn causes(&self) -> Vec<&str> {
        [&self.microphone, &self.camera]
            .into_iter()
            .flatten()
            .map(|failure| failure.cause.as_str())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failed(off: bool) -> InputFailure {
        InputFailure {
            device: "USB Microphone".into(),
            cause: "device busy".into(),
            off,
            attempt: 1,
        }
    }

    #[test]
    fn a_failure_names_the_device_and_follows_the_recording_state() {
        let message = |paused| failed(false).message(InputKind::Microphone, paused);
        assert_eq!(
            message(Some(false)),
            "Couldn't switch to USB Microphone. The recording continues."
        );
        assert_eq!(
            message(Some(true)),
            "Couldn't switch to USB Microphone. The recording stays paused."
        );
        assert_eq!(message(None), "Couldn't switch to USB Microphone.");
    }

    #[test]
    fn an_input_left_off_says_so_until_that_input_is_switched() {
        let mut failures = InputFailures::default();
        failures.set(InputKind::Microphone, Some(failed(true)));
        assert!(failures.is_off(InputKind::Microphone));
        assert!(!failures.is_off(InputKind::Camera));
        assert!(failures.messages(None)[0].contains("The microphone is off."));
        failures.set(InputKind::Camera, None);
        assert!(failures.is_off(InputKind::Microphone));
        failures.set(InputKind::Microphone, None);
        assert!(failures.messages(None).is_empty());
    }

    #[derive(Default)]
    struct Bench {
        unplugged: Vec<&'static str>,
        recorded: Option<&'static str>,
        opened: Vec<&'static str>,
        detach_fails: bool,
        stops_after_detach: bool,
        stops_after_attach: bool,
        stopped: bool,
    }

    impl SwitchSteps for Bench {
        type Device = &'static str;
        type Attached = &'static str;

        async fn detach(&mut self) -> anyhow::Result<()> {
            anyhow::ensure!(!self.detach_fails, "detach failed");
            self.recorded = None;
            self.stopped |= self.stops_after_detach;
            Ok(())
        }

        async fn attach(&mut self, device: &&'static str) -> anyhow::Result<&'static str> {
            self.opened.push(*device);
            self.stopped |= self.stops_after_attach;
            anyhow::ensure!(!self.unplugged.contains(device), "{device} is unplugged");
            self.recorded = Some(*device);
            Ok(*device)
        }

        fn is_current(&mut self) -> bool {
            !self.stopped
        }
    }

    fn recording_old() -> Bench {
        Bench {
            recorded: Some("old"),
            ..Default::default()
        }
    }

    async fn switch_to_new(bench: &mut Bench) -> Switched<&'static str> {
        switch_or_restore(bench, &"new", &"old").await
    }

    #[tokio::test]
    async fn a_device_that_opens_is_the_one_recorded() {
        let mut bench = recording_old();
        let switched = switch_to_new(&mut bench).await;
        assert!(matches!(switched, Switched::Attached("new")));
        assert_eq!(bench.recorded, Some("new"));
    }

    #[tokio::test]
    async fn a_device_that_fails_hands_the_recording_back_the_one_it_had() {
        let mut bench = recording_old();
        bench.unplugged = vec!["new"];
        let switched = switch_to_new(&mut bench).await;
        assert_eq!(bench.recorded, Some("old"));
        let Switched::Restored { attached, cause } = switched else {
            panic!("{switched:?}");
        };
        assert_eq!(attached, "old");
        assert_eq!(cause.to_string(), "new is unplugged");
    }

    #[tokio::test]
    async fn the_input_is_off_only_when_neither_device_opens() {
        let mut bench = recording_old();
        bench.unplugged = vec!["new", "old"];
        let switched = switch_to_new(&mut bench).await;
        assert_eq!(bench.opened, ["new", "old"]);
        assert_eq!(bench.recorded, None);
        let Switched::LeftOff { cause } = switched else {
            panic!("{switched:?}");
        };
        assert_eq!(cause.to_string(), "new is unplugged");
    }

    #[tokio::test]
    async fn a_device_that_cannot_be_released_stays_recorded() {
        let mut bench = recording_old();
        bench.detach_fails = true;
        let switched = switch_to_new(&mut bench).await;
        assert!(matches!(switched, Switched::Unchanged { .. }));
        assert_eq!(bench.recorded, Some("old"));
        assert!(bench.opened.is_empty());
    }

    #[tokio::test]
    async fn stop_during_release_opens_no_device() {
        let mut bench = recording_old();
        bench.stops_after_detach = true;
        let switched = switch_to_new(&mut bench).await;
        assert!(matches!(switched, Switched::Abandoned));
        assert!(bench.opened.is_empty());
    }

    #[tokio::test]
    async fn a_switch_ended_by_stop_leaves_the_app_on_the_device_the_recording_ended_with() {
        let mut bench = recording_old();
        bench.stops_after_attach = true;
        let switched = switch_to_new(&mut bench).await;
        assert_eq!(bench.recorded, Some("new"));
        assert_eq!(*switched.ended_with(&"new", &"old"), "new");

        let mut bench = recording_old();
        bench.unplugged = vec!["new"];
        bench.stops_after_attach = true;
        let switched = switch_to_new(&mut bench).await;
        assert_eq!(*switched.ended_with(&"new", &"old"), "old");

        let mut bench = recording_old();
        bench.stops_after_detach = true;
        let switched = switch_to_new(&mut bench).await;
        assert_eq!(*switched.ended_with(&"new", &"old"), "old");
    }

    #[tokio::test]
    async fn stop_during_a_failing_open_does_not_reopen_the_old_device() {
        let mut bench = recording_old();
        bench.unplugged = vec!["new"];
        bench.stops_after_attach = true;
        let switched = switch_to_new(&mut bench).await;
        assert!(matches!(switched, Switched::Abandoned));
        assert_eq!(bench.opened, ["new"]);
    }
}
