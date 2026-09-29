//! C functions behind the iOS app (see `include/ssnd.h`, used from Swift
//! through the bridging header).
//!
//! Every entry point catches panics so a bug in the engine can never take the
//! app down; failures come back as error strings or silence instead.
//! Strings returned to Swift are freed with `ssnd_free_string`.

use parking_lot::{Mutex, RwLock};
use ssnd_core::capture::CAPTURE_LATENCY_US;
use ssnd_core::engine::OUTPUT_LATENCY_US;
use ssnd_core::proto::DEFAULT_AUDIO_PORT;
use ssnd_core::{Engine, EngineConfig, Mode, Renderer, Source, Tap};
use std::ffi::{c_char, CStr, CString};
use std::net::{Ipv4Addr, SocketAddr, ToSocketAddrs};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

mod link;
mod sweep;

pub use link::*;

pub struct SsndHandle {
    engine: Arc<RwLock<Engine>>,
    renderer: Renderer,
    /// Mirrors `Engine::is_receiving` so the audio callback never touches the engine lock.
    receiving: AtomicBool,
    render_buf: Mutex<Vec<f32>>,
    capture_buf: Mutex<Vec<f32>>,
    sweep: Option<sweep::Sweep>,
}

fn handle<'a>(h: *const SsndHandle) -> Option<&'a SsndHandle> {
    // SAFETY: created by Box::into_raw in `ssnd_create`, freed only in `ssnd_destroy`.
    unsafe { h.as_ref() }
}

fn guard<T>(default: T, f: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(default)
}

fn cstr(s: *const c_char) -> String {
    if s.is_null() {
        return String::new();
    }
    // SAFETY: Swift passes NUL-terminated UTF-8 that outlives the call.
    unsafe { CStr::from_ptr(s) }.to_string_lossy().into_owned()
}

fn out_str(s: &str) -> *mut c_char {
    CString::new(s.replace('\0', " ")).map(CString::into_raw).unwrap_or(std::ptr::null_mut())
}

/// NULL for success, else an error message.
fn out_err(e: Option<String>) -> *mut c_char {
    match e {
        None => std::ptr::null_mut(),
        Some(e) => out_str(&e),
    }
}

pub(crate) fn parse_addrs(list: &str) -> Vec<SocketAddr> {
    list.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter_map(|s| {
            let full = if s.contains(':') { s.to_string() } else { format!("{s}:{DEFAULT_AUDIO_PORT}") };
            full.to_socket_addrs().ok()?.find(|a| a.is_ipv4())
        })
        .collect()
}

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

fn num(v: f64) -> String {
    if v.is_finite() {
        format!("{v:.1}")
    } else {
        "0".into()
    }
}

/// Start the engine. `mode` is "game", "balanced" or "music". `announce`
/// makes this device visible to others (the app); the screen-broadcast
/// extension only sends, so it passes false.
#[no_mangle]
pub extern "C" fn ssnd_create(name: *const c_char, mode: *const c_char, announce: bool) -> *mut SsndHandle {
    let name = cstr(name);
    let mode = Mode::parse(&cstr(mode)).unwrap_or_default();
    guard(std::ptr::null_mut(), || {
        let cfg = EngineConfig {
            name: if name.trim().is_empty() { "iPhone".into() } else { name },
            audio_port: DEFAULT_AUDIO_PORT,
            mode,
            discovery: announce,
            external_output: true,
        };
        let engine = Engine::new(cfg);
        let renderer = engine.renderer();
        let engine = Arc::new(RwLock::new(engine));
        let sweep = if announce { sweep::Sweep::start(engine.clone()) } else { None };
        let h = Box::new(SsndHandle {
            engine,
            renderer,
            receiving: AtomicBool::new(false),
            render_buf: Mutex::new(vec![0.0; 8192]),
            capture_buf: Mutex::new(vec![0.0; 8192]),
            sweep,
        });
        Box::into_raw(h)
    })
}

#[no_mangle]
pub extern "C" fn ssnd_destroy(h: *mut SsndHandle) {
    if !h.is_null() {
        guard((), || {
            // SAFETY: pointer came from `ssnd_create` and the app calls this once.
            let h = unsafe { Box::from_raw(h) };
            drop(h);
        })
    }
}

#[no_mangle]
pub extern "C" fn ssnd_free_string(s: *mut c_char) {
    if !s.is_null() {
        // SAFETY: every string we hand out comes from CString::into_raw.
        drop(unsafe { CString::from_raw(s) });
    }
}

#[no_mangle]
pub extern "C" fn ssnd_start_receiving(h: *const SsndHandle) -> *mut c_char {
    out_err(guard(Some("internal error".into()), || match handle(h) {
        Some(x) => {
            let r = x.engine.write().start_receiving().err().map(|e| format!("{e:#}"));
            x.receiving.store(r.is_none(), Ordering::Release);
            r
        }
        None => Some("engine not started".into()),
    }))
}

#[no_mangle]
pub extern "C" fn ssnd_stop_receiving(h: *const SsndHandle) {
    guard((), || {
        if let Some(x) = handle(h) {
            x.receiving.store(false, Ordering::Release);
            x.engine.write().stop_receiving();
        }
    })
}

/// Fill `frames` frames of mixed received audio into two channel buffers
/// (`right` may be NULL for mono). Called from the audio render thread: it
/// never waits on the engine lock and never allocates once warmed up.
#[no_mangle]
pub extern "C" fn ssnd_render_planar(h: *const SsndHandle, left: *mut f32, right: *mut f32, frames: u32, rate: u32) {
    if left.is_null() {
        return;
    }
    let n = frames as usize;
    // SAFETY: Swift passes buffers of at least `frames` floats.
    let l = unsafe { std::slice::from_raw_parts_mut(left, n) };
    let mut r = if right.is_null() { None } else { Some(unsafe { std::slice::from_raw_parts_mut(right, n) }) };
    let ok = guard(false, || {
        let Some(x) = handle(h) else { return false };
        if !x.receiving.load(Ordering::Acquire) {
            return false;
        }
        let Some(mut buf) = x.render_buf.try_lock() else { return false };
        if buf.len() < n * 2 {
            buf.resize(n * 2, 0.0);
        }
        let inter = &mut buf[..n * 2];
        x.renderer.render(inter, rate.max(8000), 2);
        match r.as_deref_mut() {
            Some(r) => {
                for i in 0..n {
                    l[i] = inter[2 * i];
                    r[i] = inter[2 * i + 1];
                }
            }
            None => {
                for i in 0..n {
                    l[i] = 0.5 * (inter[2 * i] + inter[2 * i + 1]);
                }
            }
        }
        true
    });
    if !ok {
        l.fill(0.0);
        if let Some(r) = r {
            r.fill(0.0);
        }
    }
}

/// Speaker delay (AVAudioSession output latency + IO buffer), ms.
#[no_mangle]
pub extern "C" fn ssnd_set_output_latency(ms: f32) {
    OUTPUT_LATENCY_US.store((ms.max(0.0) * 1000.0) as u64, Ordering::Relaxed);
}

/// Capture delay (input latency + IO buffer, or ReplayKit's delivery delay), ms.
#[no_mangle]
pub extern "C" fn ssnd_set_capture_latency(ms: f32) {
    CAPTURE_LATENCY_US.store((ms.max(0.0) * 1000.0) as u64, Ordering::Relaxed);
}

/// Start sending audio the app pushes with `ssnd_push_capture*`.
/// `dests` is a comma-separated list of "ip" or "ip:port".
#[no_mangle]
pub extern "C" fn ssnd_start_sending(h: *const SsndHandle, dests: *const c_char) -> *mut c_char {
    let dests = cstr(dests);
    out_err(guard(Some("internal error".into()), || match handle(h) {
        Some(x) => x
            .engine
            .write()
            .start_sending(Source::External, parse_addrs(&dests), false)
            .err()
            .map(|e| format!("{e:#}")),
        None => Some("engine not started".into()),
    }))
}

#[no_mangle]
pub extern "C" fn ssnd_stop_sending(h: *const SsndHandle) {
    guard((), || {
        if let Some(x) = handle(h) {
            x.engine.write().stop_sending();
        }
    })
}

/// Interleaved captured audio: `len` samples (frames × `ch`).
#[no_mangle]
pub extern "C" fn ssnd_push_capture(h: *const SsndHandle, data: *const f32, len: u32, rate: u32, ch: u32) {
    if data.is_null() || len == 0 {
        return;
    }
    guard((), || {
        let Some(x) = handle(h) else { return };
        // SAFETY: Swift passes `len` valid floats.
        let s = unsafe { std::slice::from_raw_parts(data, len as usize) };
        x.engine.read().push_capture(s, rate.max(8000), ch.max(1) as usize);
    })
}

/// Captured audio as separate channel buffers (`right` may be NULL for mono).
#[no_mangle]
pub extern "C" fn ssnd_push_capture_planar(h: *const SsndHandle, left: *const f32, right: *const f32, frames: u32, rate: u32) {
    if left.is_null() || frames == 0 {
        return;
    }
    guard((), || {
        let Some(x) = handle(h) else { return };
        let n = frames as usize;
        // SAFETY: Swift passes `frames` valid floats per channel.
        let l = unsafe { std::slice::from_raw_parts(left, n) };
        if right.is_null() {
            x.engine.read().push_capture(l, rate.max(8000), 1);
            return;
        }
        let r = unsafe { std::slice::from_raw_parts(right, n) };
        let mut buf = x.capture_buf.lock();
        if buf.len() < n * 2 {
            buf.resize(n * 2, 0.0);
        }
        for i in 0..n {
            buf[2 * i] = l[i];
            buf[2 * i + 1] = r[i];
        }
        x.engine.read().push_capture(&buf[..n * 2], rate.max(8000), 2);
    })
}

#[no_mangle]
pub extern "C" fn ssnd_set_send_dests(h: *const SsndHandle, dests: *const c_char) {
    let dests = cstr(dests);
    guard((), || {
        if let Some(x) = handle(h) {
            x.engine.read().set_send_dests(parse_addrs(&dests));
        }
    })
}

/// Daisy-chain: pass received audio on to these devices.
#[no_mangle]
pub extern "C" fn ssnd_set_forward(h: *const SsndHandle, dests: *const c_char) {
    let dests = cstr(dests);
    guard((), || {
        if let Some(x) = handle(h) {
            x.engine.read().set_forward(parse_addrs(&dests));
        }
    })
}

#[no_mangle]
pub extern "C" fn ssnd_set_play_local(h: *const SsndHandle, on: bool) {
    guard((), || {
        if let Some(x) = handle(h) {
            x.engine.read().set_play_local(on);
        }
    })
}

/// 0.0 ..= 1.5 (150 %).
#[no_mangle]
pub extern "C" fn ssnd_set_volume(h: *const SsndHandle, v: f32) {
    guard((), || {
        if let Some(x) = handle(h) {
            x.engine.read().set_volume(if v.is_finite() { v } else { 1.0 });
        }
    })
}

/// "game", "balanced" or "music".
#[no_mangle]
pub extern "C" fn ssnd_set_mode(h: *const SsndHandle, mode: *const c_char) {
    let mode = cstr(mode);
    guard((), || {
        if let (Some(x), Some(m)) = (handle(h), Mode::parse(&mode)) {
            x.engine.read().set_mode(m);
        }
    })
}

#[no_mangle]
pub extern "C" fn ssnd_set_name(h: *const SsndHandle, name: *const c_char) {
    let name = cstr(name);
    guard((), || {
        if let Some(x) = handle(h) {
            x.engine.read().set_name(&name);
        }
    })
}

/// Devices added by IP in Settings: they get this device's hello directly,
/// so they list the iPhone even across subnets.
#[no_mangle]
pub extern "C" fn ssnd_set_manual_peers(h: *const SsndHandle, ips: *const c_char) {
    let ips = cstr(ips);
    guard((), || {
        if let Some(s) = handle(h).and_then(|x| x.sweep.as_ref()) {
            let list: Vec<Ipv4Addr> = parse_addrs(&ips)
                .into_iter()
                .filter_map(|a| match a {
                    SocketAddr::V4(v) => Some(*v.ip()),
                    _ => None,
                })
                .collect();
            s.set_manual(list);
        }
    })
}

/// Visualizer data: `tap` 0 = send, 1 = receive; `kind` 0 = spectrum
/// (`n` bands), 1 = waveform (`n` columns). Writes `n` values in 0..1 to
/// `out`; returns false (and writes zeros) while that side is off.
#[no_mangle]
pub extern "C" fn ssnd_scope(h: *const SsndHandle, tap: i32, kind: i32, n: u32, out: *mut f32) -> bool {
    if out.is_null() || n == 0 {
        return false;
    }
    let n = n as usize;
    // SAFETY: Swift passes a buffer of `n` floats.
    let out = unsafe { std::slice::from_raw_parts_mut(out, n) };
    let vals = guard(None, || {
        let x = handle(h)?;
        let snap = x.engine.read().scope(if tap == 0 { Tap::Send } else { Tap::Receive })?;
        Some(if kind == 0 { snap.spectrum(n) } else { snap.waveform(n) })
    });
    match vals {
        Some(v) => {
            for (o, v) in out.iter_mut().zip(v.iter().chain(std::iter::repeat(&0.0))) {
                *o = if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 };
            }
            true
        }
        None => {
            out.fill(0.0);
            false
        }
    }
}

/// Everything the UI shows, as one JSON object.
#[no_mangle]
pub extern "C" fn ssnd_state_json(h: *const SsndHandle) -> *mut c_char {
    let json = guard("{}".to_string(), || {
        let Some(x) = handle(h) else { return "{}".to_string() };
        let e = x.engine.read();
        let peers: Vec<String> = e
            .peers()
            .iter()
            .map(|p| {
                format!(
                    r#"{{"id":"{}","name":"{}","addr":"{}","receiving":{}}}"#,
                    esc(&p.id),
                    esc(&p.name),
                    p.addr(),
                    p.receiving
                )
            })
            .collect();
        let streams: Vec<String> = e
            .streams()
            .iter()
            .map(|s| {
                format!(
                    r#"{{"id":{},"name":"{}","from":"{}","bufferMs":{},"targetMs":{},"captureMs":{},"lost":{},"late":{},"underruns":{},"level":{:.3},"rate":{}}}"#,
                    s.id,
                    esc(&s.name),
                    esc(&s.from),
                    num(s.buffer_ms),
                    num(s.target_ms),
                    num(s.capture_ms),
                    s.lost,
                    s.late,
                    s.underruns,
                    if s.level.is_finite() { s.level } else { 0.0 },
                    s.sample_rate
                )
            })
            .collect();
        let st = e.sender_stats();
        let ips: Vec<String> = e.local_ips().iter().map(|i| format!(r#""{i}""#)).collect();
        format!(
            r#"{{"id":"{}","name":"{}","ips":[{}],"mode":"{}","outputMs":{},"captureMs":{},"receiving":{},"receiverError":"{}","playLocal":{},"forwarded":{},"sending":{},"sentPackets":{},"sendLevel":{:.3},"sendError":"{}","peers":[{}],"streams":[{}]}}"#,
            esc(&e.id),
            esc(&e.name()),
            ips.join(","),
            e.mode().as_str(),
            num(e.output_latency_ms()),
            num(st.capture_ms),
            e.is_receiving(),
            esc(&e.receiver_error().unwrap_or_default()),
            e.play_local(),
            e.forwarded_packets(),
            st.active,
            st.packets,
            if st.level.is_finite() { st.level } else { 0.0 },
            esc(&st.error.unwrap_or_default()),
            peers.join(","),
            streams.join(",")
        )
    });
    out_str(&json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn round_trip_through_the_c_api() {
        let name = CString::new("ทดสอบ \"iPhone\"").unwrap();
        let mode = CString::new("game").unwrap();
        let h = ssnd_create(name.as_ptr(), mode.as_ptr(), false);
        assert!(!h.is_null());
        let err = ssnd_start_receiving(h);
        assert!(err.is_null(), "{}", cstr(err));

        let mut l = vec![1.0f32; 256];
        let mut r = vec![1.0f32; 256];
        ssnd_render_planar(h, l.as_mut_ptr(), r.as_mut_ptr(), 256, 48_000);
        assert!(l.iter().chain(r.iter()).all(|v| *v == 0.0), "silence when nothing arrives");

        let mut bars = vec![9.0f32; 32];
        assert!(ssnd_scope(h, 1, 0, 32, bars.as_mut_ptr()));
        assert!(bars.iter().all(|v| (0.0..=1.0).contains(v)));
        assert!(!ssnd_scope(h, 0, 1, 32, bars.as_mut_ptr()), "send side is off");

        let js = ssnd_state_json(h);
        let s = cstr(js);
        ssnd_free_string(js);
        assert!(s.contains(r#""mode":"game""#), "{s}");
        assert!(s.contains(r#"\"iPhone\""#), "{s}");
        assert!(s.contains(r#""receiving":true"#), "{s}");

        ssnd_stop_receiving(h);
        ssnd_destroy(h);
    }

    #[test]
    fn null_handles_are_harmless() {
        let p = std::ptr::null();
        ssnd_stop_receiving(p);
        ssnd_set_volume(p, 1.0);
        let mut l = vec![1.0f32; 16];
        ssnd_render_planar(p, l.as_mut_ptr(), std::ptr::null_mut(), 16, 48_000);
        assert!(l.iter().all(|v| *v == 0.0));
        let e = ssnd_start_receiving(p);
        assert!(!e.is_null());
        ssnd_free_string(e);
    }

    #[test]
    fn parses_destinations() {
        let a = parse_addrs("192.168.1.5, 10.0.0.2:5000,,");
        assert_eq!(a.len(), 2);
        assert_eq!(a[0].port(), DEFAULT_AUDIO_PORT);
        assert_eq!(a[1].port(), 5000);
    }
}
