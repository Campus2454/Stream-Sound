//! A few remembered choices, stored as `key=value` lines in the user's config folder.

use ssnd_core::Mode;
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq)]
pub struct Settings {
    pub mode: Mode,
    pub volume: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { mode: Mode::default(), volume: 1.0 }
    }
}

fn path() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    }?;
    Some(base.join("StreamSound").join("settings.txt"))
}

pub fn load() -> Settings {
    let mut s = Settings::default();
    let Some(text) = path().and_then(|p| std::fs::read_to_string(p).ok()) else { return s };
    for line in text.lines() {
        match line.split_once('=') {
            Some(("mode", v)) => s.mode = Mode::parse(v).unwrap_or(s.mode),
            Some(("volume", v)) => s.volume = v.trim().parse().unwrap_or(s.volume),
            _ => {}
        }
    }
    s
}

pub fn save(s: &Settings) {
    let Some(p) = path() else { return };
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(p, format!("mode={}\nvolume={}\n", s.mode.as_str(), s.volume));
}
