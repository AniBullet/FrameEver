use std::path::PathBuf;

const PAYLOAD: &str = "FRAMEEVER_INSTALLER_PAYLOAD";
const INSTALLED_BYTES: &str = "FRAMEEVER_INSTALLER_INSTALLED_BYTES";

fn embed_payload() {
    println!("cargo:rerun-if-env-changed={PAYLOAD}");
    println!("cargo:rerun-if-env-changed={INSTALLED_BYTES}");
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR is set by cargo"));
    let embedded = out.join("payload.bin");
    let payload = std::env::var_os(PAYLOAD).filter(|value| !value.is_empty());
    match &payload {
        Some(source) => {
            println!("cargo:rerun-if-changed={}", PathBuf::from(source).display());
            let copied = std::fs::copy(source, &embedded)
                .expect("failed to copy the NSIS installer payload");
            if copied == 0 {
                panic!("{PAYLOAD} points at an empty file");
            }
        }
        None => std::fs::write(&embedded, []).expect("failed to write an empty payload"),
    }
    let installed: u64 = std::env::var(INSTALLED_BYTES)
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(0);
    if payload.is_some() && installed == 0 {
        panic!("{INSTALLED_BYTES} must be set with {PAYLOAD}");
    }
    std::fs::write(
        out.join("payload.rs"),
        format!("pub const INSTALLED_BYTES: u64 = {installed};\n"),
    )
    .expect("failed to write the payload constants");
}

fn main() {
    embed_payload();
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        for arg in [
            "/NODEFAULTLIB:vcruntime.lib",
            "/NODEFAULTLIB:vcruntimed.lib",
            "/NODEFAULTLIB:msvcrt.lib",
            "/NODEFAULTLIB:msvcrtd.lib",
            "/NODEFAULTLIB:libucrt.lib",
            "/DEFAULTLIB:libcmt.lib",
            "/DEFAULTLIB:libvcruntime.lib",
            "/DEFAULTLIB:ucrt.lib",
        ] {
            println!("cargo:rustc-link-arg-bin=frameever-setup={arg}");
        }
    }
    let icon = "../icons/icon.ico";
    println!("cargo:rerun-if-changed={icon}");
    tauri_winres::WindowsResource::new()
        .set_icon_with_id(icon, "1")
        .set("ProductName", "FrameEver")
        .set("FileDescription", "FrameEver Setup")
        .set("CompanyName", "aniBullet")
        .set("OriginalFilename", "frameever-setup.exe")
        .compile()
        .expect("failed to compile the Windows icon resource");
}
