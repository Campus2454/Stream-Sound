//! Linux capture through PulseAudio (also works on PipeWire via pipewire-pulse).
//!
//! System audio records the default sink's monitor. Per-app audio creates a
//! private null sink, moves the app's streams onto it and records its monitor.

use super::{CaptureOptions, SampleSink, Source, SourceInfo, SourceKind};
use anyhow::{anyhow, Context};
use libpulse_binding::def::BufferAttr;
use libpulse_binding::sample::{Format, Spec};
use libpulse_binding::stream::Direction;
use libpulse_simple_binding::Simple;
use serde_json::Value;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

const RATE: u32 = 48_000;
const CH: usize = 2;
const CHUNK_FRAMES: usize = 240; // 5 ms

fn pactl(args: &[&str]) -> anyhow::Result<String> {
    let out = Command::new("pactl").args(args).output().context("pactl not found (install pulseaudio-utils)")?;
    if !out.status.success() {
        return Err(anyhow!("pactl {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn pactl_json(what: &str) -> Vec<Value> {
    pactl(&["-f", "json", "list", what])
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
}

fn prop<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get("properties")?.get(key)?.as_str()
}

/// Stable key for an app: its binary name, falling back to its display name.
fn app_key(v: &Value) -> Option<String> {
    let pid = std::process::id().to_string();
    if prop(v, "application.process.id") == Some(pid.as_str()) {
        return None; // our own streams
    }
    if prop(v, "media.name").map(|m| m.starts_with("Loopback")).unwrap_or(false) {
        return None;
    }
    prop(v, "application.process.binary")
        .or_else(|| prop(v, "application.name"))
        .map(|s| s.to_string())
}

pub fn list_apps_and_inputs() -> Vec<SourceInfo> {
    let mut v = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for si in pactl_json("sink-inputs") {
        if let Some(key) = app_key(&si) {
            if seen.insert(key.clone()) {
                let name = prop(&si, "application.name").unwrap_or(&key).to_string();
                v.push(SourceInfo { label: format!("แอป: {name}"), source: Source::App { key }, kind: SourceKind::App });
            }
        }
    }
    for s in pactl_json("sources") {
        let name = s.get("name").and_then(|n| n.as_str()).unwrap_or_default();
        if name.is_empty() || name.ends_with(".monitor") || name.starts_with("ssnd_app_") {
            continue;
        }
        let desc = s.get("description").and_then(|n| n.as_str()).unwrap_or(name);
        v.push(SourceInfo {
            label: format!("อินพุต: {desc}"),
            source: Source::Input { name: name.to_string() },
            kind: SourceKind::Input,
        });
    }
    v
}

pub fn run(source: &Source, opts: &CaptureOptions, sink: SampleSink, stop: &AtomicBool) -> anyhow::Result<()> {
    match source {
        Source::System => record("@DEFAULT_MONITOR@", sink, stop, &mut || {}),
        Source::Input { name } => record(name, sink, stop, &mut || {}),
        Source::App { key } => {
            let route = AppRoute::new(key, opts.keep_local)?;
            let monitor = format!("{}.monitor", route.sink_name);
            let mut last = Instant::now() - Duration::from_secs(5);
            let mut tick = || {
                // Apps open new streams (next song, new tab); keep catching them.
                if last.elapsed() > Duration::from_millis(500) {
                    last = Instant::now();
                    route.capture_matching();
                }
            };
            record(&monitor, sink, stop, &mut tick)
        }
        Source::Tone => unreachable!("handled by caller"),
    }
}

fn record(device: &str, mut sink: SampleSink, stop: &AtomicBool, tick: &mut dyn FnMut()) -> anyhow::Result<()> {
    let spec = Spec { format: Format::FLOAT32NE, channels: CH as u8, rate: RATE };
    let frag = (CHUNK_FRAMES * CH * 4) as u32;
    let attr = BufferAttr { maxlength: u32::MAX, tlength: u32::MAX, prebuf: u32::MAX, minreq: u32::MAX, fragsize: frag };
    let s = Simple::new(None, "Stream Sound", Direction::Record, Some(device), "capture", &spec, None, Some(&attr))
        .map_err(|e| anyhow!("cannot record from {device}: {e}"))?;
    let mut bytes = vec![0u8; CHUNK_FRAMES * CH * 4];
    let mut samples = vec![0.0f32; CHUNK_FRAMES * CH];
    tick();
    while !stop.load(Ordering::Relaxed) {
        s.read(&mut bytes).map_err(|e| anyhow!("read from {device} failed: {e}"))?;
        for (o, b) in samples.iter_mut().zip(bytes.chunks_exact(4)) {
            *o = f32::from_ne_bytes([b[0], b[1], b[2], b[3]]);
        }
        sink(&samples, RATE, CH);
        tick();
    }
    Ok(())
}

struct AppRoute {
    key: String,
    sink_name: String,
    sink_index: Option<u64>,
    default_sink: String,
    modules: Vec<String>,
}

impl AppRoute {
    fn new(key: &str, keep_local: bool) -> anyhow::Result<AppRoute> {
        cleanup_stale_modules();
        let default_sink = pactl(&["get-default-sink"])?;
        let sink_name = format!("ssnd_app_{}", std::process::id());
        let mut modules = vec![pactl(&[
            "load-module",
            "module-null-sink",
            &format!("sink_name={sink_name}"),
            "sink_properties=device.description=StreamSound-capture",
        ])?];
        if keep_local {
            if let Ok(m) = pactl(&[
                "load-module",
                "module-loopback",
                &format!("source={sink_name}.monitor"),
                &format!("sink={default_sink}"),
                "latency_msec=5",
            ]) {
                modules.push(m);
            }
        }
        let sink_index = pactl_json("sinks")
            .iter()
            .find(|s| s.get("name").and_then(|n| n.as_str()) == Some(sink_name.as_str()))
            .and_then(|s| s.get("index").and_then(|i| i.as_u64()));
        // A new sink can become the default or pull other streams onto it
        // (switch-on-connect). Put everything that isn't the chosen app back.
        if pactl(&["get-default-sink"]).ok().as_deref() != Some(default_sink.as_str()) {
            let _ = pactl(&["set-default-sink", &default_sink]);
        }
        let r = AppRoute { key: key.to_string(), sink_name, sink_index, default_sink, modules };
        r.capture_matching();
        Ok(r)
    }

    fn capture_matching(&self) {
        for si in pactl_json("sink-inputs") {
            let Some(idx) = si.get("index").and_then(|i| i.as_u64()) else { continue };
            let on_ours = self.sink_index.is_some() && si.get("sink").and_then(|s| s.as_u64()) == self.sink_index;
            let is_loopback = prop(&si, "media.name").map(|m| m.starts_with("Loopback")).unwrap_or(false);
            let matches = app_key(&si).as_deref() == Some(self.key.as_str());
            if matches && !on_ours {
                let _ = pactl(&["move-sink-input", &idx.to_string(), &self.sink_name]);
            } else if !matches && on_ours && !is_loopback {
                // Something else landed on our capture sink; send it back.
                let _ = pactl(&["move-sink-input", &idx.to_string(), &self.default_sink]);
            }
        }
    }
}

impl Drop for AppRoute {
    fn drop(&mut self) {
        for si in pactl_json("sink-inputs") {
            let on_ours = self.sink_index.is_some() && si.get("sink").and_then(|s| s.as_u64()) == self.sink_index;
            if on_ours {
                if let Some(idx) = si.get("index").and_then(|i| i.as_u64()) {
                    let _ = pactl(&["move-sink-input", &idx.to_string(), &self.default_sink]);
                }
            }
        }
        for m in self.modules.iter().rev() {
            let _ = pactl(&["unload-module", m]);
        }
    }
}

/// Remove capture sinks left behind by a copy of the app that was killed.
fn cleanup_stale_modules() {
    let Ok(list) = pactl(&["list", "short", "modules"]) else { return };
    let mine = format!("ssnd_app_{}", std::process::id());
    for line in list.lines() {
        let mut cols = line.split('\t');
        let (Some(id), Some(_name), Some(args)) = (cols.next(), cols.next(), cols.next()) else { continue };
        if args.contains("ssnd_app_") && !args.contains(&mine) {
            // Only remove sinks whose owner process is gone.
            let owner = args
                .split("ssnd_app_")
                .nth(1)
                .and_then(|s| s.split(|c: char| !c.is_ascii_digit()).next())
                .and_then(|p| p.parse::<u32>().ok());
            if let Some(pid) = owner {
                if !std::path::Path::new(&format!("/proc/{pid}")).exists() {
                    let _ = pactl(&["unload-module", id]);
                }
            }
        }
    }
}
