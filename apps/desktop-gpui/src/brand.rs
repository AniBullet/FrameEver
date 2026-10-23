pub(crate) use cap_utils::app_name;
pub use cap_utils::brand::{
    APP_ID, APP_NAME, CLI_COMMAND, LEGACY_APP_ID, RELEASES_REPO, URL_SCHEME, is_app_url,
};

pub const USER_AGENT: &str = concat!(app_name!(), "/", env!("CARGO_PKG_VERSION"));
