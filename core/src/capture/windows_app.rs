//! Windows per-app capture using process loopback (Windows 10 2004+ / 11).

use super::{CaptureOptions, SampleSink, SourceInfo, SourceKind, Source};
use anyhow::anyhow;
use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use wasapi::{AudioClient, DeviceEnumerator, Direction, SampleType, SessionState, StreamMode, WaveFormat};

const RATE: u32 = 48_000;
const CH: usize = 2;
const CHUNK_FRAMES: usize = 240;

/// "chrome.exe" from a session identifier like "...|\Device\...\chrome.exe%b{...}".
fn exe_from_identifier(id: &str) -> Option<String> {
    let path = id.split('|').nth(1)?;
    let file = path.rsplit('\\').next()?;
    let exe = file.split('%').next()?;
    if exe.is_empty() {
        None
    } else {
        Some(exe.to_string())
    }
}

pub fn list_apps() -> Vec<SourceInfo> {
    let _ = wasapi::initialize_mta();
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let Ok(en) = DeviceEnumerator::new() else { return out };
    let Ok(devices) = en.get_device_collection(&Direction::Render) else { return out };
    for dev in &devices {
        let Ok(dev) = dev else { continue };
        let Ok(mgr) = dev.get_iaudiosessionmanager() else { continue };
        let Ok(sessions) = mgr.get_audiosessionenumerator() else { continue };
        for i in 0..sessions.get_count().unwrap_or(0) {
            let Ok(ctl) = sessions.get_session(i) else { continue };
            if ctl.get_state().ok() == Some(SessionState::Expired) {
                continue;
            }
            let Ok(pid) = ctl.get_process_id() else { continue };
            if pid == 0 || pid == std::process::id() {
                continue; // system sounds / ourselves
            }
            let exe = ctl
                .get_session_identifier()
                .ok()
                .and_then(|s| exe_from_identifier(&s))
                .unwrap_or_else(|| format!("pid {pid}"));
            if seen.insert(exe.to_lowercase()) {
                out.push(SourceInfo {
                    label: format!("แอป: {exe}"),
                    source: Source::App { key: format!("{exe}|{pid}") },
                    kind: SourceKind::App,
                });
            }
        }
    }
    out
}

pub fn run(key: &str, _opts: &CaptureOptions, mut sink: SampleSink, stop: &AtomicBool) -> anyhow::Result<()> {
    let pid: u32 = key
        .rsplit('|')
        .next()
        .and_then(|p| p.parse().ok())
        .ok_or_else(|| anyhow!("bad app key {key}"))?;
    let _ = wasapi::initialize_mta();
    let fmt = WaveFormat::new(32, 32, &SampleType::Float, RATE as usize, CH, None);
    let block = fmt.get_blockalign() as usize;
    let mut client = AudioClient::new_application_loopback_client(pid, true)
        .map_err(|e| anyhow!("cannot capture this app (needs Windows 10 2004 or newer): {e}"))?;
    let mode = StreamMode::EventsShared { autoconvert: true, buffer_duration_hns: 0 };
    client.initialize_client(&fmt, &Direction::Capture, &mode).map_err(|e| anyhow!("{e}"))?;
    let event = client.set_get_eventhandle().map_err(|e| anyhow!("{e}"))?;
    let capture = client.get_audiocaptureclient().map_err(|e| anyhow!("{e}"))?;
    client.start_stream().map_err(|e| anyhow!("{e}"))?;

    let mut queue: VecDeque<u8> = VecDeque::with_capacity(block * RATE as usize / 10);
    let mut samples = vec![0.0f32; CHUNK_FRAMES * CH];
    let chunk_bytes = CHUNK_FRAMES * block;
    while !stop.load(Ordering::Relaxed) {
        // Silence produces no events, so a timeout is normal; just re-check stop.
        let _ = event.wait_for_event(100);
        loop {
            match capture.get_next_packet_size() {
                Ok(Some(n)) if n > 0 => {
                    capture.read_from_device_to_deque(&mut queue).map_err(|e| anyhow!("{e}"))?;
                }
                Ok(_) => break,
                Err(e) => return Err(anyhow!("app capture stopped (app closed?): {e}")),
            }
        }
        while queue.len() >= chunk_bytes {
            for s in samples.iter_mut() {
                let b = [
                    queue.pop_front().unwrap_or(0),
                    queue.pop_front().unwrap_or(0),
                    queue.pop_front().unwrap_or(0),
                    queue.pop_front().unwrap_or(0),
                ];
                *s = f32::from_le_bytes(b);
            }
            sink(&samples, RATE, CH);
        }
    }
    let _ = client.stop_stream();
    Ok(())
}
