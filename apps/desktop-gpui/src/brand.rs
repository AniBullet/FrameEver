pub(crate) use frameever_utils::app_name;
pub use frameever_utils::brand::{
    APP_ID, APP_NAME, CLI_COMMAND, RELEASES_REPO, URL_SCHEME, is_app_url,
};

pub const USER_AGENT: &str = concat!(app_name!(), "/", env!("CARGO_PKG_VERSION"));
