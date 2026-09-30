//! Windows: put the app icon and version details into StreamSound.exe, so
//! Explorer, the taskbar and the tray show them.

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-env-changed=SSND_VERSION");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let version = std::env::var("SSND_VERSION").unwrap_or_else(|_| "dev".into());
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/icon.ico")
        .set("ProductName", "Stream Sound")
        .set("FileDescription", "Stream Sound")
        .set("OriginalFilename", "StreamSound.exe")
        .set("InternalName", "StreamSound")
        .set("LegalCopyright", "Campus2454")
        .set("ProductVersion", &version)
        .set("FileVersion", &version)
        .set_version_info(winresource::VersionInfo::PRODUCTVERSION, numeric(&version))
        .set_version_info(winresource::VersionInfo::FILEVERSION, numeric(&version));
    if let Err(e) = res.compile() {
        // Release builds must have it; a local build can do without.
        if std::env::var_os("CI").is_some() {
            panic!("could not add the Windows icon: {e}");
        }
        println!("cargo:warning=could not add the Windows icon: {e}");
    }
}

/// "0.2.5" as the four 16-bit parts Windows shows (0.2.5.0); "dev" is 0.0.0.0.
fn numeric(version: &str) -> u64 {
    let mut parts = version.split('.').map(|p| p.parse::<u64>().unwrap_or(0).min(0xFFFF));
    let mut v = 0u64;
    for _ in 0..4 {
        v = (v << 16) | parts.next().unwrap_or(0);
    }
    v
}
