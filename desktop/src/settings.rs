//! Remembered choices, stored as `key=value` lines in the user's config
//! folder (%APPDATA%\StreamSound or ~/.config/StreamSound). Lists repeat the
//! key once per item. Unknown keys are ignored, so older and newer builds
//! can share the file.

use ssnd_core::{Mode, Source};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Visual {
    Bars,
    Wave,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Settings {
    pub mode: Mode,
    pub volume: f32,
    pub muted: bool,
    /// Device name other devices see; `None` uses the computer's name.
    pub name: Option<String>,
    /// Typed-in addresses ("192.168.1.20" or "192.168.1.20:47800").
    pub manual: Vec<String>,
    /// Ticked destinations as "ip:port".
    pub send_to: Vec<String>,
    pub forward_to: Vec<String>,
    pub source: Source,
    pub keep_local: bool,
    pub play_local: bool,
    pub auto_receive: bool,
    pub visual: Visual,
    /// Also update to beta versions (vX.Y.Z), not only official ones.
    pub beta: bool,
    /// Volume for each sending device by its IP, 0..2; devices not listed play at 1.
    pub source_volume: Vec<(String, f32)>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            mode: Mode::default(),
            volume: 1.0,
            muted: false,
            name: None,
            manual: Vec::new(),
            send_to: Vec::new(),
            forward_to: Vec::new(),
            source: Source::System,
            keep_local: false,
            play_local: true,
            auto_receive: true,
            visual: Visual::Bars,
            beta: true,
            source_volume: Vec::new(),
        }
    }
}

impl Settings {
    /// Volume for everything from the device at `ip` (1 = as sent).
    pub fn source_volume(&self, ip: &str) -> f32 {
        self.source_volume.iter().find(|(k, _)| k == ip).map(|(_, v)| *v).unwrap_or(1.0)
    }

    pub fn set_source_volume(&mut self, ip: &str, v: f32) {
        self.source_volume.retain(|(k, _)| k != ip);
        if v != 1.0 {
            self.source_volume.push((ip.to_string(), v));
        }
    }
}

pub fn source_to_str(s: &Source) -> String {
    match s {
        Source::System | Source::External => "system".into(),
        Source::Tone => "tone".into(),
        Source::App { key } => format!("app:{key}"),
        Source::Input { name } => format!("input:{name}"),
    }
}

pub fn source_from_str(s: &str) -> Source {
    match s {
        "tone" => Source::Tone,
        _ if s.starts_with("app:") => Source::App { key: s[4..].to_string() },
        _ if s.starts_with("input:") => Source::Input { name: s[6..].to_string() },
        _ => Source::System,
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

fn flag(v: &str) -> bool {
    v.trim() == "1"
}

/// 0..2 (0–200 %).
fn volume(v: &str) -> Option<f32> {
    v.trim().parse::<f32>().ok().filter(|x| x.is_finite()).map(|x| x.clamp(0.0, 2.0))
}

pub fn parse(text: &str) -> Settings {
    let mut s = Settings::default();
    for line in text.lines() {
        let Some((k, v)) = line.split_once('=') else { continue };
        let v = v.trim_end_matches('\r');
        match k {
            "mode" => s.mode = Mode::parse(v).unwrap_or(s.mode),
            "volume" => s.volume = volume(v).unwrap_or(s.volume),
            "muted" => s.muted = flag(v),
            "name" if !v.trim().is_empty() => s.name = Some(v.trim().to_string()),
            "manual" if !v.trim().is_empty() => s.manual.push(v.trim().to_string()),
            "send_to" if !v.trim().is_empty() => s.send_to.push(v.trim().to_string()),
            "forward_to" if !v.trim().is_empty() => s.forward_to.push(v.trim().to_string()),
            "source" => s.source = source_from_str(v),
            "keep_local" => s.keep_local = flag(v),
            "play_local" => s.play_local = flag(v),
            "auto_receive" => s.auto_receive = flag(v),
            "visual" => s.visual = if v.trim() == "wave" { Visual::Wave } else { Visual::Bars },
            "beta" => s.beta = flag(v),
            "source_volume" => {
                // "<ip> <volume>"
                if let Some((ip, vol)) = v.trim().split_once(' ') {
                    if let (false, Some(vol)) = (ip.is_empty(), volume(vol)) {
                        s.set_source_volume(ip, vol);
                    }
                }
            }
            _ => {}
        }
    }
    s
}

pub fn format(s: &Settings) -> String {
    let one_line = |v: &str| v.replace(['\r', '\n'], " ");
    let mut o = String::new();
    o += &format!("mode={}\nvolume={}\nmuted={}\n", s.mode.as_str(), s.volume, s.muted as u8);
    if let Some(n) = &s.name {
        o += &format!("name={}\n", one_line(n));
    }
    for m in &s.manual {
        o += &format!("manual={}\n", one_line(m));
    }
    for a in &s.send_to {
        o += &format!("send_to={a}\n");
    }
    for a in &s.forward_to {
        o += &format!("forward_to={a}\n");
    }
    o += &format!("source={}\n", one_line(&source_to_str(&s.source)));
    o += &format!(
        "keep_local={}\nplay_local={}\nauto_receive={}\nvisual={}\nbeta={}\n",
        s.keep_local as u8,
        s.play_local as u8,
        s.auto_receive as u8,
        if s.visual == Visual::Wave { "wave" } else { "bars" },
        s.beta as u8
    );
    for (ip, v) in &s.source_volume {
        o += &format!("source_volume={} {v}\n", one_line(ip));
    }
    o
}

pub fn load() -> Settings {
    path().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| parse(&t)).unwrap_or_default()
}

/// Write next to the old file, then swap, so a crash never leaves it half written.
pub fn save(s: &Settings) {
    let Some(p) = path() else { return };
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = p.with_extension("tmp");
    if std::fs::write(&tmp, format(s)).is_ok() {
        let _ = std::fs::rename(&tmp, &p);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let s = Settings {
            mode: Mode::Game,
            volume: 0.75,
            muted: true,
            name: Some("PC ห้องนั่งเล่น".into()),
            manual: vec!["192.168.1.20".into(), "10.0.0.5:47800".into()],
            send_to: vec!["192.168.1.30:47800".into()],
            forward_to: vec![],
            source: Source::App { key: "chrome.exe".into() },
            keep_local: true,
            play_local: false,
            auto_receive: false,
            visual: Visual::Wave,
            beta: false,
            source_volume: vec![("192.168.1.30".into(), 0.4), ("10.0.0.5".into(), 1.8)],
        };
        assert_eq!(parse(&format(&s)), s);
    }

    #[test]
    fn old_file_still_loads() {
        let s = parse("mode=music\nvolume=0.5\n");
        assert_eq!(s.mode, Mode::Music);
        assert_eq!(s.volume, 0.5);
        assert!(s.auto_receive && s.play_local && s.beta);
    }

    #[test]
    fn junk_is_ignored() {
        let s = parse("volume=NaN\nmode=loud\n=x\nsend_to=\nwhat\nsource_volume=1.2.3.4\nsource_volume= 0.5\n");
        assert_eq!(s, Settings::default());
    }

    #[test]
    fn volumes_go_to_200_percent() {
        let s = parse("volume=1.9\nsource_volume=10.0.0.2 7\nsource_volume=10.0.0.3 1\n");
        assert_eq!(s.volume, 1.9);
        assert_eq!(s.source_volume("10.0.0.2"), 2.0);
        assert_eq!(s.source_volume("10.0.0.3"), 1.0);
        assert_eq!(s.source_volume.len(), 1, "1 is the default and is not stored");
    }
}
