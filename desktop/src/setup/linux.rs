//! Linux: the downloaded file installs itself. It copies itself into a
//! folder, adds the icon, an app-menu entry (with an "Uninstall" action) and
//! a desktop shortcut, and remembers what it did in `install.txt` next to
//! the settings so updates and uninstalling can find everything again.

use crate::os::linux::{config_dir, data_dir, desktop_entry, exec_arg, set_autostart_for, write_if_changed, APP_ID};
use anyhow::{anyhow, Context};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// The program's name inside the install folder.
pub const EXE_NAME: &str = "stream-sound";

/// What the installer did, as saved in `install.txt`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Record {
    /// The installed program; `None` when the app isn't installed.
    pub exe: Option<PathBuf>,
    pub desktop_shortcut: bool,
    pub menu: bool,
    /// Downloaded files the user chose to run without installing.
    pub portable: Vec<PathBuf>,
}

impl Record {
    fn parse(text: &str) -> Record {
        let mut r = Record::default();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            match k.trim() {
                "exe" if !v.is_empty() => r.exe = Some(PathBuf::from(v)),
                "desktop_shortcut" => r.desktop_shortcut = v.trim() == "1",
                "menu" => r.menu = v.trim() == "1",
                "portable" if !v.is_empty() => r.portable.push(PathBuf::from(v)),
                _ => {}
            }
        }
        r
    }

    fn text(&self) -> String {
        let mut s = String::new();
        if let Some(e) = &self.exe {
            s += &format!("exe={}\n", e.display());
        }
        s += &format!("desktop_shortcut={}\nmenu={}\n", self.desktop_shortcut as u8, self.menu as u8);
        for p in &self.portable {
            s += &format!("portable={}\n", p.display());
        }
        s
    }
}

fn settings_dir() -> Option<PathBuf> {
    Some(config_dir()?.join("StreamSound"))
}

fn record_file() -> Option<PathBuf> {
    Some(settings_dir()?.join("install.txt"))
}

pub fn load() -> Record {
    record_file().and_then(|p| fs::read_to_string(p).ok()).map(|t| Record::parse(&t)).unwrap_or_default()
}

fn save(r: &Record) -> anyhow::Result<()> {
    let p = record_file().ok_or_else(|| anyhow!("no home folder"))?;
    write_if_changed(&p, r.text().as_bytes())?;
    Ok(())
}

/// This program's file, with links resolved.
pub fn this_exe() -> Option<PathBuf> {
    let p = std::env::current_exe().ok()?;
    Some(fs::canonicalize(&p).unwrap_or(p))
}

fn same_file(a: &Path, b: &Path) -> bool {
    a == b || fs::canonicalize(a).ok().zip(fs::canonicalize(b).ok()).is_some_and(|(a, b)| a == b)
}

/// Whether this copy is the installed one.
pub fn installed_here() -> bool {
    let (Some(me), Some(installed)) = (this_exe(), load().exe) else { return false };
    same_file(&me, &installed)
}

/// Whether starting this file should open the installer: it isn't the
/// installed copy, and the user hasn't chosen to run it without installing.
/// Local builds (cargo run) never ask.
pub fn should_offer_install() -> bool {
    if cfg!(debug_assertions) || installed_here() {
        return false;
    }
    let Some(me) = this_exe() else { return false };
    !load().portable.iter().any(|p| same_file(p, &me))
}

/// Remember "run without installing" for this file.
pub fn remember_portable() {
    let (mut r, Some(me)) = (load(), this_exe()) else { return };
    if !r.portable.iter().any(|p| same_file(p, &me)) {
        r.portable.push(me);
        let _ = save(&r);
    }
}

/// Where a new install goes unless the user picks another folder.
pub fn default_dir() -> PathBuf {
    data_dir().map(|d| d.join(APP_ID)).unwrap_or_else(|| PathBuf::from("/tmp").join(APP_ID))
}

// ---- files the installer adds ---------------------------------------------------

fn icon_file() -> Option<PathBuf> {
    Some(data_dir()?.join("icons/hicolor/256x256/apps").join(format!("{APP_ID}.png")))
}

fn menu_file() -> Option<PathBuf> {
    Some(data_dir()?.join("applications").join(format!("{APP_ID}.desktop")))
}

/// The desktop folder, from ~/.config/user-dirs.dirs (it has a local name
/// in many languages), else ~/Desktop.
fn desktop_dir() -> Option<PathBuf> {
    let home = PathBuf::from(std::env::var_os("HOME")?);
    let dirs = config_dir().and_then(|c| fs::read_to_string(c.join("user-dirs.dirs")).ok()).unwrap_or_default();
    Some(desktop_dir_from(&dirs, &home))
}

fn desktop_dir_from(user_dirs: &str, home: &Path) -> PathBuf {
    for line in user_dirs.lines() {
        let Some(v) = line.trim().strip_prefix("XDG_DESKTOP_DIR=") else { continue };
        let v = v.trim().trim_matches('"');
        if let Some(rest) = v.strip_prefix("$HOME") {
            let rest = rest.trim_start_matches('/');
            // "$HOME/" alone means the home folder itself.
            return if rest.is_empty() { home.to_path_buf() } else { home.join(rest) };
        }
        if v.starts_with('/') {
            return PathBuf::from(v);
        }
    }
    home.join("Desktop")
}

fn shortcut_file() -> Option<PathBuf> {
    Some(desktop_dir()?.join(format!("{APP_ID}.desktop")))
}

/// The menu entry and desktop shortcut: open the app, and "Uninstall" on
/// right-click.
fn entry_text(exe: &Path) -> String {
    let e = exec_arg(&exe.to_string_lossy());
    desktop_entry(
        &e,
        &format!(
            "Actions=uninstall;\n\n[Desktop Action uninstall]\nName=ถอนการติดตั้ง Stream Sound\nExec={e} {}\n",
            super::UNINSTALL_FLAG
        ),
    )
}

fn write_shortcut(path: &Path, exe: &Path) -> anyhow::Result<()> {
    write_if_changed(path, entry_text(exe).as_bytes())?;
    // File managers only launch desktop files that are executable, and
    // GNOME's desktop also wants them marked trusted.
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    let _ = std::process::Command::new("gio")
        .args(["set", &path.to_string_lossy(), "metadata::trusted", "true"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    Ok(())
}

/// Remove `path` if it is one of our desktop files.
fn remove_entry(path: Option<PathBuf>) {
    let Some(p) = path else { return };
    if fs::read_to_string(&p).is_ok_and(|t| t.contains("Name=Stream Sound")) {
        let _ = fs::remove_file(p);
    }
}

/// Make the icon, menu entry and shortcut match `r`.
fn write_integration(r: &Record, exe: &Path) -> anyhow::Result<()> {
    if let Some(icon) = icon_file() {
        write_if_changed(&icon, crate::gui::ICON_PNG)?;
    }
    match (r.menu, menu_file()) {
        (true, Some(m)) => write_if_changed(&m, entry_text(exe).as_bytes())?,
        (false, m) => remove_entry(m),
        _ => {}
    }
    match (r.desktop_shortcut, shortcut_file()) {
        (true, Some(d)) => write_shortcut(&d, exe)?,
        (false, d) => remove_entry(d),
        _ => {}
    }
    Ok(())
}

/// At start of the installed copy: keep its files current (a new version's
/// icon or entry text). A shortcut the user deleted stays deleted.
pub fn refresh() {
    if !installed_here() {
        return;
    }
    let r = load();
    let Some(exe) = r.exe.clone() else { return };
    let shortcut_gone = shortcut_file().is_some_and(|d| !d.exists());
    let r = Record { desktop_shortcut: r.desktop_shortcut && !shortcut_gone, ..r };
    let _ = write_integration(&r, &exe);
}

// ---- install, update, uninstall -------------------------------------------------

pub struct Options {
    pub dir: PathBuf,
    pub desktop_shortcut: bool,
    pub menu: bool,
}

/// Copy this program to `to`, replacing whatever is there (a running
/// program can be replaced; it keeps running from the old file).
fn copy_self(to: &Path) -> anyhow::Result<()> {
    let me = this_exe().ok_or_else(|| anyhow!("can't find this program's file"))?;
    if same_file(&me, to) {
        return Ok(());
    }
    let dir = to.parent().ok_or_else(|| anyhow!("bad path {}", to.display()))?;
    fs::create_dir_all(dir).with_context(|| format!("สร้างโฟลเดอร์ {} ไม่ได้", dir.display()))?;
    let tmp = dir.join(format!(".{EXE_NAME}.new"));
    fs::copy(&me, &tmp).with_context(|| format!("เขียนลง {} ไม่ได้", dir.display()))?;
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755))?;
    fs::File::open(&tmp)?.sync_all()?;
    fs::rename(&tmp, to).with_context(|| format!("แทนที่ {} ไม่ได้", to.display()))?;
    Ok(())
}

/// Wait for a running copy to close on its own (an update: the old copy is
/// quitting), then ask it to.
fn close_running(wait_first: Duration) -> anyhow::Result<()> {
    let until = Instant::now() + wait_first;
    while crate::os::running() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(100));
    }
    if !crate::os::ask_to_quit(Duration::from_secs(10)) {
        anyhow::bail!("Stream Sound ที่เปิดอยู่ไม่ยอมปิด ปิดแอปก่อน (คลิกขวาที่ไอคอนในถาด แล้วเลือก \"ออก\") แล้วลองใหม่");
    }
    Ok(())
}

/// Install this file. `step` gets progress (0..1) and what is happening.
/// Returns the installed program.
pub fn install(o: &Options, step: &dyn Fn(f32, &str)) -> anyhow::Result<PathBuf> {
    let dir = if o.dir.is_absolute() { o.dir.clone() } else { anyhow::bail!("ใส่ที่อยู่โฟลเดอร์แบบเต็ม เช่น {}", default_dir().display()) };
    let exe = dir.join(EXE_NAME);
    step(0.1, "กำลังปิด Stream Sound ที่เปิดอยู่");
    close_running(Duration::ZERO)?;
    step(0.3, "กำลังคัดลอกโปรแกรม");
    copy_self(&exe)?;
    step(0.6, "กำลังเพิ่มไอคอนและทางลัด");
    let old = load();
    let r = Record { exe: Some(exe.clone()), desktop_shortcut: o.desktop_shortcut, menu: o.menu, portable: Vec::new() };
    write_integration(&r, &exe)?;
    // Moved to another folder: take the old copy away.
    if let Some(prev) = old.exe.as_ref().filter(|p| !same_file(p, &exe)) {
        if prev.file_name().is_some_and(|n| n == EXE_NAME) {
            let _ = fs::remove_file(prev);
            if let Some(d) = prev.parent() {
                let _ = fs::remove_dir(d);
            }
        }
    }
    if crate::os::autostart_enabled() {
        set_autostart_for(&exe, true)?;
    }
    step(0.9, "กำลังบันทึก");
    save(&r)?;
    step(1.0, "ติดตั้งเสร็จแล้ว");
    Ok(exe)
}

/// Replace `target` (the program an update was started from) with this
/// file, once that copy has closed.
pub fn update(target: &Path, step: &dyn Fn(f32, &str)) -> anyhow::Result<()> {
    step(0.1, "กำลังรอให้เวอร์ชันเดิมปิด");
    close_running(Duration::from_secs(15))?;
    step(0.4, "กำลังติดตั้งเวอร์ชันใหม่");
    copy_self(target)?;
    let r = load();
    if r.exe.as_ref().is_some_and(|e| same_file(e, target)) {
        step(0.8, "กำลังอัปเดตไอคอนและทางลัด");
        let shortcut_gone = shortcut_file().is_some_and(|d| !d.exists());
        let _ = write_integration(&Record { desktop_shortcut: r.desktop_shortcut && !shortcut_gone, ..r }, target);
    }
    step(1.0, "อัปเดตเสร็จแล้ว");
    Ok(())
}

/// Remove the installed program, its icon, menu entry, shortcut and
/// start-with-computer entry; the settings too if `settings`.
pub fn uninstall(settings: bool, step: &dyn Fn(f32, &str)) -> anyhow::Result<()> {
    step(0.1, "กำลังปิด Stream Sound");
    // This may be the installed program itself (the menu's Uninstall
    // action); it doesn't hold the port, so only another copy is closed.
    close_running(Duration::ZERO)?;
    step(0.4, "กำลังลบโปรแกรม");
    let r = load();
    if let Some(exe) = &r.exe {
        if exe.exists() {
            fs::remove_file(exe).with_context(|| format!("ลบ {} ไม่ได้", exe.display()))?;
        }
        if let Some(d) = exe.parent() {
            let _ = fs::remove_dir(d);
        }
    }
    step(0.6, "กำลังลบไอคอนและทางลัด");
    remove_entry(menu_file());
    remove_entry(shortcut_file());
    if let Some(i) = icon_file() {
        let _ = fs::remove_file(i);
    }
    if r.exe.is_some() {
        let _ = set_autostart_for(Path::new(""), false);
    }
    step(0.8, "กำลังลบข้อมูลการติดตั้ง");
    if settings {
        if let Some(d) = settings_dir() {
            let _ = fs::remove_dir_all(d);
        }
    } else if let Some(f) = record_file() {
        let _ = fs::remove_file(f);
    }
    step(1.0, "ถอนการติดตั้งแล้ว");
    Ok(())
}

/// Start `exe` on its own (it outlives this process).
pub fn launch(exe: &Path, args: &[&str]) -> anyhow::Result<()> {
    use std::process::{Command, Stdio};
    Command::new(exe)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("เปิด {} ไม่ได้", exe.display()))?;
    Ok(())
}

/// A folder picker from the desktop, if one is installed (zenity on GNOME
/// and most others, kdialog on KDE). `None` if neither is there.
pub fn folder_picker() -> Option<&'static str> {
    ["kdialog", "zenity"].into_iter().find(|t| {
        std::env::var_os("PATH").is_some_and(|path| std::env::split_paths(&path).any(|d| d.join(t).is_file()))
    })
}

/// Ask with `tool` for a folder, starting at `start`.
pub fn pick_folder(tool: &str, start: &Path) -> Option<PathBuf> {
    let start = start.to_string_lossy().into_owned();
    let out = match tool {
        "kdialog" => std::process::Command::new("kdialog").args(["--getexistingdirectory", &start]).output(),
        _ => std::process::Command::new("zenity")
            .args(["--file-selection", "--directory", "--title=เลือกโฟลเดอร์ที่จะติดตั้ง", &format!("--filename={start}/")])
            .output(),
    }
    .ok()?;
    let p = String::from_utf8(out.stdout).ok()?;
    let p = p.trim();
    (out.status.success() && !p.is_empty()).then(|| PathBuf::from(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_round_trips() {
        let r = Record {
            exe: Some("/home/me/.local/share/stream-sound/stream-sound".into()),
            desktop_shortcut: true,
            menu: false,
            portable: vec!["/home/me/Downloads/StreamSound-linux-x86_64".into()],
        };
        assert_eq!(Record::parse(&r.text()), r);
        assert_eq!(Record::parse(""), Record::default());
    }

    #[test]
    fn finds_the_desktop_folder() {
        let home = Path::new("/home/me");
        assert_eq!(desktop_dir_from("", home), home.join("Desktop"));
        let dirs = "# comment\nXDG_DOWNLOAD_DIR=\"$HOME/ดาวน์โหลด\"\nXDG_DESKTOP_DIR=\"$HOME/เดสก์ท็อป\"\n";
        assert_eq!(desktop_dir_from(dirs, home), home.join("เดสก์ท็อป"));
        assert_eq!(desktop_dir_from("XDG_DESKTOP_DIR=\"/data/desk\"", home), PathBuf::from("/data/desk"));
        assert_eq!(desktop_dir_from("XDG_DESKTOP_DIR=\"$HOME/\"", home), home.to_path_buf());
    }

    #[test]
    fn entries_offer_uninstall() {
        let t = entry_text(Path::new("/opt/My Apps/stream-sound"));
        assert!(t.contains("Exec=\"/opt/My Apps/stream-sound\"\n"), "{t}");
        assert!(t.contains("Actions=uninstall;"), "{t}");
        assert!(t.contains("Exec=\"/opt/My Apps/stream-sound\" --uninstall\n"), "{t}");
    }
}
