use serde::Serialize;

use crate::{OutputFormat, write_json};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateReport {
    started: bool,
    handled_by: &'static str,
}

pub fn run(format: OutputFormat) -> Result<(), String> {
    crate::automation::open_path_or_url(&frameever_utils::brand::action_url(
        "%22check_for_updates%22",
    ))
    .map_err(|error| format!("Could not reach the desktop app to check for updates: {error}"))?;
    match format {
        OutputFormat::Json => write_json(&UpdateReport {
            started: true,
            handled_by: "desktop",
        }),
        OutputFormat::Text => {
            println!(
                "Asked the desktop app to check for updates. It downloads and verifies the signed release, then updates the bundled CLI with it."
            );
            Ok(())
        }
    }
}
