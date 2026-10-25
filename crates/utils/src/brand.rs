#[macro_export]
macro_rules! app_name {
    () => {
        "FrameEver"
    };
}

pub const APP_NAME: &str = app_name!();
pub const APP_ID: &str = "io.github.anibullet.frameever";
pub const URL_SCHEME: &str = "frameever";
pub const CLI_COMMAND: &str = "frameever";
pub const RELEASES_REPO: &str = "AniBullet/FrameEver";
pub const PROJECT_EXTENSION: &str = "ever";
pub const RECORDINGS_DIR_NAME: &str = "recordings";
pub const SCREENSHOTS_DIR_NAME: &str = "screenshots";

pub fn is_app_url(raw: &str) -> bool {
    raw.strip_prefix(URL_SCHEME)
        .is_some_and(|rest| rest.starts_with("://"))
}

pub fn action_url(value: &str) -> String {
    format!("{URL_SCHEME}://action?value={value}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_our_own_scheme_counts_as_an_app_url() {
        assert!(is_app_url("frameever://action?value=%22stop_recording%22"));
        assert!(is_app_url(&action_url("%22check_for_updates%22")));
        assert!(!is_app_url("frameever-evil://action"));
        assert!(!is_app_url("cap-desktop://action"));
        assert!(!is_app_url("cap://action"));
        assert_ne!(APP_ID, "so.cap.desktop");
    }
}
