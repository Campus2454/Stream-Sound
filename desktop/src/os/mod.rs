//! What the desktop app needs from the operating system: a tray icon to live
//! in after the window is closed, starting with the computer, keeping the
//! window clear of the taskbar, and running one copy at a time.

mod instance;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
use linux as imp;
#[cfg(windows)]
use windows as imp;

pub use instance::{claim, Claim};

use eframe::egui;
use raw_window_handle::RawWindowHandle;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Command-line flag the start-with-computer entry uses: open in the tray.
pub const MINIMIZED_FLAG: &str = "--minimized";
/// Flag an update passes to the new copy, which waits for the old one to go.
pub const AFTER_UPDATE_FLAG: &str = "--after-update";

/// Asks from the tray menu or from a second copy of the app, picked up by
/// the window on its next frame.
#[derive(Default)]
pub struct Requests {
    show: AtomicBool,
    quit: AtomicBool,
}

impl Requests {
    pub fn take_show(&self) -> bool {
        self.show.swap(false, Ordering::Relaxed)
    }

    pub fn take_quit(&self) -> bool {
        self.quit.swap(false, Ordering::Relaxed)
    }
}

/// Brings the window back; callable from any thread.
#[derive(Clone)]
pub struct Waker {
    ctx: egui::Context,
    req: Arc<Requests>,
    #[cfg(windows)]
    hwnd: Option<isize>,
}

impl Waker {
    pub fn new(ctx: &egui::Context, window: Option<RawWindowHandle>, req: Arc<Requests>) -> Waker {
        #[cfg(not(windows))]
        let _ = window;
        Waker {
            ctx: ctx.clone(),
            req,
            #[cfg(windows)]
            hwnd: window.and_then(imp::hwnd),
        }
    }

    pub fn show(&self) {
        // Windows doesn't run frames for a hidden window, so show it here;
        // the next frame then tells the window library it is visible again.
        #[cfg(windows)]
        if let Some(h) = self.hwnd {
            imp::show_native(h);
        }
        self.req.show.store(true, Ordering::Relaxed);
        self.ctx.request_repaint();
    }

    /// Exit from the tray menu.
    pub fn quit(&self) {
        // A hidden window gets no frames on Windows, so nothing would act
        // on the request: the settings are already saved and the system
        // frees the sound devices, so leave right away.
        #[cfg(windows)]
        if self.hwnd.is_some_and(|h| !imp::is_visible(h)) {
            imp::remove_tray_icon();
            std::process::exit(0);
        }
        self.req.quit.store(true, Ordering::Relaxed);
        self.ctx.request_repaint();
        // Linux keeps running frames while hidden, so this is only a
        // backstop if that ever stops working.
        #[cfg(not(windows))]
        let _ = std::thread::Builder::new().name("ssnd-quit".into()).spawn(|| {
            std::thread::sleep(std::time::Duration::from_secs(5));
            std::process::exit(0);
        });
    }
}

/// The tray icon: click to bring the window back, right-click for a menu
/// with Exit. Made before the window so Linux can pick a window system that
/// can hide windows; `connect` hands it the window once there is one.
pub struct Tray {
    #[cfg(any(windows, target_os = "linux"))]
    inner: imp::Tray,
}

impl Tray {
    /// `None` when this desktop has no tray (for example GNOME without the
    /// AppIndicator extension); closing the window then quits as before.
    pub fn new() -> Option<Tray> {
        #[cfg(any(windows, target_os = "linux"))]
        return imp::Tray::new().map(|inner| Tray { inner });
        #[cfg(not(any(windows, target_os = "linux")))]
        None
    }

    pub fn connect(&self, waker: Waker) {
        #[cfg(any(windows, target_os = "linux"))]
        self.inner.connect(waker);
        #[cfg(not(any(windows, target_os = "linux")))]
        let _ = waker;
    }

    /// Whether the icon is showing right now, so hiding the window is safe.
    pub fn available(&self) -> bool {
        #[cfg(any(windows, target_os = "linux"))]
        return self.inner.available();
        #[cfg(not(any(windows, target_os = "linux")))]
        false
    }
}

/// Whether the window can be hidden and shown again on this system
/// (Wayland has no way to hide a window, so there the app runs on X11).
pub fn can_hide_window() -> bool {
    #[cfg(target_os = "linux")]
    return std::env::var_os("DISPLAY").is_some();
    #[cfg(not(target_os = "linux"))]
    true
}

pub fn hide_window(ctx: &egui::Context) {
    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
}

pub fn show_window(ctx: &egui::Context) {
    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
}

/// Move and shrink the window so all of it is on screen and none of it is
/// under the taskbar. `Some(true)` when done, `Some(false)` to try again on
/// a later frame (not shown yet), `None` when this system can't tell where
/// the taskbar is and the caller should go by the monitor size.
pub fn fit_to_work_area(window: Option<RawWindowHandle>) -> Option<bool> {
    #[cfg(windows)]
    return window.and_then(imp::hwnd).map(imp::fit_to_work_area);
    #[cfg(not(windows))]
    {
        let _ = window;
        None
    }
}

/// Whether the app starts with the computer, as the system sees it now
/// (so turning it off in Task Manager or the desktop's autostart settings
/// shows here too).
pub fn autostart_enabled() -> bool {
    #[cfg(any(windows, target_os = "linux"))]
    return imp::autostart_enabled();
    #[cfg(not(any(windows, target_os = "linux")))]
    false
}

pub fn set_autostart(on: bool) -> anyhow::Result<()> {
    #[cfg(any(windows, target_os = "linux"))]
    return imp::set_autostart(on);
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = on;
        anyhow::bail!("not supported here")
    }
}

/// At start: point an existing start-with-computer entry at this copy of
/// the app (it may have been moved), and on Linux list the app and its
/// icon with the desktop's apps.
pub fn refresh_integration() {
    #[cfg(any(windows, target_os = "linux"))]
    imp::refresh_integration();
}

/// The app's own file, for start-with-computer entries.
#[allow(dead_code)]
fn exe_path() -> Option<std::path::PathBuf> {
    // An AppImage runs from a temporary mount; $APPIMAGE is the real file.
    if let Some(p) = std::env::var_os("APPIMAGE") {
        return Some(p.into());
    }
    std::env::current_exe().ok()
}
