use std::path::PathBuf;

const PAYLOAD: &str = "FRAMEEVER_INSTALLER_PAYLOAD";
const INSTALLED_BYTES: &str = "FRAMEEVER_INSTALLER_INSTALLED_BYTES";
const MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
    </application>
  </compatibility>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/pm</dpiAware>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
      <activeCodePage xmlns="http://schemas.microsoft.com/SMI/2019/WindowsSettings">UTF-8</activeCodePage>
    </windowsSettings>
  </application>
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/>
    </dependentAssembly>
  </dependency>
</assembly>
"#;

fn embed_payload() {
    println!("cargo:rerun-if-env-changed={PAYLOAD}");
    println!("cargo:rerun-if-env-changed={INSTALLED_BYTES}");
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR is set by cargo"));
    let embedded = out.join("payload.bin");
    let payload = std::env::var_os(PAYLOAD).filter(|value| !value.is_empty());
    match &payload {
        Some(source) => {
            println!("cargo:rerun-if-changed={}", PathBuf::from(source).display());
            std::fs::copy(source, &embedded).expect("failed to copy the NSIS installer payload");
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
        .set_manifest(MANIFEST)
        .set("ProductName", "FrameEver")
        .set("FileDescription", "FrameEver Setup")
        .set("CompanyName", "aniBullet")
        .set("OriginalFilename", "frameever-setup.exe")
        .compile()
        .expect("failed to compile the Windows icon resource");
}
