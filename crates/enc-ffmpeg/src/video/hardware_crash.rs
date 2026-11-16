use std::sync::{OnceLock, PoisonError, RwLock};

static DISABLED: RwLock<Vec<String>> = RwLock::new(Vec::new());
static OBSERVER: OnceLock<fn(&str, bool)> = OnceLock::new();

pub fn set_disabled_hardware_encoders(names: Vec<String>) {
    *DISABLED.write().unwrap_or_else(PoisonError::into_inner) = names;
}

pub fn hardware_encoder_disabled(name: &str) -> bool {
    DISABLED
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .any(|disabled| disabled == name)
}

pub fn set_hardware_encoder_observer(observer: fn(&str, bool)) {
    if OBSERVER.set(observer).is_err() {
        tracing::debug!("hardware encoder observer already registered");
    }
}

pub struct HardwareEncoderSession {
    name: String,
}

impl HardwareEncoderSession {
    pub fn begin(name: &str) -> Self {
        if let Some(observer) = OBSERVER.get() {
            observer(name, true);
        }
        Self {
            name: name.to_owned(),
        }
    }
}

impl Drop for HardwareEncoderSession {
    fn drop(&mut self) {
        if let Some(observer) = OBSERVER.get() {
            observer(&self.name, false);
        }
    }
}
