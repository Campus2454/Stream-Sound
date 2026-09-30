//! Installing, updating and removing the desktop app.
//!
//! Windows has a standard installer (installer/windows.nsi): it asks where
//! to install and which shortcuts to add, registers an uninstaller with
//! Settings > Apps, and runs with `/UPDATE` for updates, showing only its
//! progress. On Linux the single downloaded file installs itself
//! (`linux`, with the steps shown by gui::setup).

#[cfg(target_os = "linux")]
pub mod linux;

/// Linux: open the installer (also what a plain start does for a file that
/// isn't installed yet).
#[cfg(target_os = "linux")]
pub const INSTALL_FLAG: &str = "--install";
/// Linux: `--update <program>` replaces that program with this file.
#[cfg(target_os = "linux")]
pub const UPDATE_FLAG: &str = "--update";
/// Linux: remove the installed app.
#[cfg(target_os = "linux")]
pub const UNINSTALL_FLAG: &str = "--uninstall";
/// Windows: the uninstaller's name in the install folder.
#[cfg(windows)]
pub const WINDOWS_UNINSTALLER: &str = "uninstall.exe";

/// Whether this copy was installed (rather than run from a downloaded file).
pub fn installed() -> bool {
    #[cfg(windows)]
    return std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join(WINDOWS_UNINSTALLER)))
        .is_some_and(|u| u.is_file());
    #[cfg(target_os = "linux")]
    return linux::installed_here();
    #[cfg(not(any(windows, target_os = "linux")))]
    false
}
