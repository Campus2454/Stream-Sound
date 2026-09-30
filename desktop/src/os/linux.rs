//! Linux: a StatusNotifierItem tray icon (KDE, Xfce, Cinnamon, and GNOME
//! with the AppIndicator extension), XDG autostart, and the desktop-file
//! helpers the installer (setup::linux) uses.

use super::Waker;
use ksni::blocking::TrayMethods;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

pub(crate) const APP_ID: &str = "stream-sound";

struct Item {
    waker: Arc<OnceLock<Waker>>,
    online: Arc<AtomicBool>,
    icon: Vec<ksni::Icon>,
}

impl ksni::Tray for Item {
    fn id(&self) -> String {
        APP_ID.into()
    }

    fn title(&self) -> String {
        "Stream Sound".into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        self.icon.clone()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip { title: "Stream Sound".into(), ..Default::default() }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        if let Some(w) = self.waker.get() {
            w.show();
        }
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::StandardItem;
        vec![
            StandardItem {
                label: "เปิด Stream Sound".into(),
                activate: Box::new(|this: &mut Self| {
                    if let Some(w) = this.waker.get() {
                        w.show();
                    }
                }),
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
            StandardItem {
                label: "ออก".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|this: &mut Self| {
                    if let Some(w) = this.waker.get() {
                        w.quit();
                    }
                }),
                ..Default::default()
            }
            .into(),
        ]
    }

    fn watcher_online(&self) {
        self.online.store(true, Ordering::Relaxed);
    }

    fn watcher_offline(&self, _reason: ksni::OfflineReason) -> bool {
        // The panel went away (or restarted): until it is back, closing the
        // window quits rather than hiding it where nobody can reach it.
        self.online.store(false, Ordering::Relaxed);
        true
    }
}

pub struct Tray {
    waker: Arc<OnceLock<Waker>>,
    online: Arc<AtomicBool>,
    handle: ksni::blocking::Handle<Item>,
}

impl Tray {
    pub fn new() -> Option<Tray> {
        let waker = Arc::new(OnceLock::new());
        let online = Arc::new(AtomicBool::new(true));
        let item = Item { waker: waker.clone(), online: online.clone(), icon: tray_icons() };
        let handle = item.spawn().ok()?;
        Some(Tray { waker, online, handle })
    }

    pub fn connect(&self, waker: Waker) {
        let _ = self.waker.set(waker);
    }

    pub fn available(&self) -> bool {
        self.online.load(Ordering::Relaxed)
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        let _ = self.handle.shutdown();
    }
}

/// The app icon as ARGB (big-endian), at a few sizes for the panel to pick.
fn tray_icons() -> Vec<ksni::Icon> {
    let img = crate::gui::app_icon();
    let (w, h) = (img.width as usize, img.height as usize);
    [64usize, 32, 22]
        .iter()
        .map(|&n| {
            let mut data = Vec::with_capacity(n * n * 4);
            for y in 0..n {
                for x in 0..n {
                    // Box filter from the 256 px source.
                    let (x0, x1) = (x * w / n, ((x + 1) * w / n).max(x * w / n + 1));
                    let (y0, y1) = (y * h / n, ((y + 1) * h / n).max(y * h / n + 1));
                    let mut acc = [0u32; 4];
                    for sy in y0..y1 {
                        for sx in x0..x1 {
                            let p = &img.rgba[(sy * w + sx) * 4..][..4];
                            let a = p[3] as u32;
                            acc[0] += a;
                            acc[1] += p[0] as u32 * a;
                            acc[2] += p[1] as u32 * a;
                            acc[3] += p[2] as u32 * a;
                        }
                    }
                    let count = ((x1 - x0) * (y1 - y0)) as u32;
                    let a = acc[0] / count;
                    let c = |v: u32| if acc[0] == 0 { 0 } else { (v / acc[0]) as u8 };
                    data.extend_from_slice(&[a as u8, c(acc[1]), c(acc[2]), c(acc[3])]);
                }
            }
            ksni::Icon { width: n as i32, height: n as i32, data }
        })
        .collect()
}

// ---- files ------------------------------------------------------------------

/// An XDG folder variable, or `fallback` under $HOME. Empty or relative
/// values count as unset, as the XDG spec says.
pub(crate) fn xdg_dir(var: &str, fallback: &str) -> Option<PathBuf> {
    let absolute = |p: PathBuf| p.is_absolute().then_some(p);
    std::env::var_os(var)
        .map(PathBuf::from)
        .and_then(absolute)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(fallback)).and_then(absolute))
}

pub(crate) fn config_dir() -> Option<PathBuf> {
    xdg_dir("XDG_CONFIG_HOME", ".config")
}

pub(crate) fn data_dir() -> Option<PathBuf> {
    xdg_dir("XDG_DATA_HOME", ".local/share")
}

fn autostart_file() -> Option<PathBuf> {
    Some(config_dir()?.join("autostart").join(format!("{APP_ID}.desktop")))
}

/// Quote a path for a desktop file's Exec line (Desktop Entry spec).
pub(crate) fn exec_arg(path: &str) -> String {
    let mut q = String::from("\"");
    for c in path.chars() {
        match c {
            '"' | '`' | '$' | '\\' => {
                q.push('\\');
                q.push(c);
            }
            '%' => q.push_str("%%"),
            _ => q.push(c),
        }
    }
    q.push('"');
    // Then escape backslashes again for the file's own string syntax.
    q.replace('\\', "\\\\")
}

pub(crate) fn desktop_entry(exec: &str, extra: &str) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName=Stream Sound\nComment=ส่งและรับเสียงระหว่างเครื่องในวง LAN\n\
         Exec={exec}\nIcon={APP_ID}\nTerminal=false\nCategories=AudioVideo;Audio;\nStartupWMClass={APP_ID}\n{extra}"
    )
}

fn exe() -> Option<PathBuf> {
    super::exe_path()
}

/// Write only when different, so the desktop isn't told about a change
/// on every start.
pub(crate) fn write_if_changed(path: &std::path::Path, data: &[u8]) -> std::io::Result<()> {
    if std::fs::read(path).ok().as_deref() == Some(data) {
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, data)
}

fn autostart_text(exe: &std::path::Path) -> String {
    let exec = format!("{} {}", exec_arg(&exe.to_string_lossy()), super::MINIMIZED_FLAG);
    desktop_entry(&exec, "X-GNOME-Autostart-enabled=true\n")
}

pub fn autostart_enabled() -> bool {
    let Some(text) = autostart_file().and_then(|p| std::fs::read_to_string(p).ok()) else { return false };
    // Desktops turn an entry off without deleting it.
    !text.lines().any(|l| {
        let l = l.trim().replace(' ', "");
        l.eq_ignore_ascii_case("Hidden=true") || l.eq_ignore_ascii_case("X-GNOME-Autostart-enabled=false")
    })
}

pub fn set_autostart(on: bool) -> anyhow::Result<()> {
    let exe = exe().ok_or_else(|| anyhow::anyhow!("can't find the app's file"))?;
    set_autostart_for(&exe, on)
}

/// Start `exe` with the computer, or stop doing so.
pub(crate) fn set_autostart_for(exe: &std::path::Path, on: bool) -> anyhow::Result<()> {
    let path = autostart_file().ok_or_else(|| anyhow::anyhow!("no home folder"))?;
    if on {
        write_if_changed(&path, autostart_text(exe).as_bytes())?;
    } else if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

pub fn refresh_integration() {
    // An installed copy keeps its menu entry and icon current (the files
    // are written by the installer; see setup::linux).
    crate::setup::linux::refresh();
    if autostart_enabled() {
        if let Some(exe) = exe() {
            let _ = set_autostart_for(&exe, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autostart_follows_the_desktop_file() {
        // The only test that touches XDG_CONFIG_HOME.
        let dir = std::env::temp_dir().join(format!("ssnd-autostart-{}", std::process::id()));
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        assert!(!autostart_enabled());
        set_autostart(true).unwrap();
        let file = dir.join("autostart/stream-sound.desktop");
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.contains("\" --minimized\n"), "{text}");
        assert!(autostart_enabled());
        // Turned off in the desktop's settings without deleting the file.
        std::fs::write(&file, text.replace("X-GNOME-Autostart-enabled=true", "X-GNOME-Autostart-enabled=false")).unwrap();
        assert!(!autostart_enabled());
        std::fs::write(&file, format!("{text}Hidden=true\n")).unwrap();
        assert!(!autostart_enabled());
        set_autostart(false).unwrap();
        assert!(!file.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn exec_paths_are_quoted() {
        assert_eq!(exec_arg("/home/me/StreamSound"), "\"/home/me/StreamSound\"");
        assert_eq!(exec_arg("/home/me/My Apps/a$b"), "\"/home/me/My Apps/a\\\\$b\"");
        assert_eq!(exec_arg("/x/100%"), "\"/x/100%%\"");
    }
}
