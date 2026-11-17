#[path = "../../src/app_icon.rs"]
pub mod app_icon;
pub mod command_line;
pub mod field;
pub mod flow;
pub mod i18n;
#[path = "../../src/instance_control.rs"]
pub mod instance_control;
pub mod location;
pub mod measure;
#[path = "../../src/theme/motion.rs"]
pub mod motion;
pub mod settings;
pub mod theme;
pub mod version;

pub mod payload {
    include!(concat!(env!("OUT_DIR"), "/payload.rs"));

    pub const NSIS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/payload.bin"));
}
