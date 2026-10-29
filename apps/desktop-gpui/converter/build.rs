fn main() {
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
            println!("cargo:rustc-link-arg-bin=frameever-converter={arg}");
        }
    }
    let icon = "../icons/icon.ico";
    println!("cargo:rerun-if-changed={icon}");
    tauri_winres::WindowsResource::new()
        .set_icon_with_id(icon, "1")
        .set("ProductName", "FrameEver Converter")
        .set("FileDescription", "FrameEver Converter")
        .set("CompanyName", "aniBullet")
        .set("OriginalFilename", "FrameEver-Converter.exe")
        .compile()
        .expect("failed to compile the Windows icon resource");
}
