#[path = "../../scripts/diagnostic-build.rs"]
mod diagnostic_build;

use std::path::Path;

const EMBEDDED_SKILL: &str = "skill/frameever-demo";

fn main() {
    diagnostic_build::emit();
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set");
    let node_modules = Path::new(&manifest_dir)
        .join(EMBEDDED_SKILL)
        .join("node_modules");

    println!("cargo:rerun-if-changed={EMBEDDED_SKILL}");

    if node_modules.exists() {
        panic!(
            "\n\n{EMBEDDED_SKILL}/node_modules exists at build time.\n\
             include_dir! in src/agents.rs embeds the whole {EMBEDDED_SKILL} folder into the \
             frameever binary and ignores .gitignore, so this build would ship node_modules.\n\n\
             Fix: rm -rf apps/cli/{EMBEDDED_SKILL}/node_modules\n\n"
        );
    }
}
