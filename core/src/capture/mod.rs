//! Audio sources: whole-system audio, one app, an input device, or a test tone.
//! Every source delivers interleaved f32 samples to a callback.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(not(any(target_os = "linux", target_os = "android")))]
mod cpal_src;
#[cfg(windows)]
mod windows_app;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    /// Everything the computer is playing.
    System,
    /// One application. `key` is platform specific (process name / pid).
    App { key: String },
    /// A microphone or line-in by device name.
    Input { name: String },
    /// Built-in test tone, useful to check a connection.
    Tone,
    /// Samples pushed in by the host app (Android/iOS capture their own
    /// audio and hand it over with `Engine::push_capture`).
    External,
}

#[derive(Clone, Debug)]
pub struct SourceInfo {
    pub source: Source,
    pub label: String,
    pub kind: SourceKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceKind {
    System,
    App,
    Input,
    Tone,
}

/// Receives (interleaved samples, sample rate, channels).
pub type SampleSink = Box<dyn FnMut(&[f32], u32, usize) + Send + 'static>;

pub struct CaptureOptions {
    /// For per-app capture: keep playing the app on this computer too.
    pub keep_local: bool,
}

pub struct CaptureHandle {
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
    pub error: Arc<parking_lot::Mutex<Option<String>>>,
}

impl CaptureHandle {
    pub fn last_error(&self) -> Option<String> {
        self.error.lock().clone()
    }
}

impl Drop for CaptureHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

pub fn list_sources() -> Vec<SourceInfo> {
    let mut v = vec![SourceInfo { source: Source::System, label: "เสียงทั้งระบบ".into(), kind: SourceKind::System }];
    #[cfg(target_os = "linux")]
    v.extend(linux::list_apps_and_inputs());
    #[cfg(windows)]
    v.extend(windows_app::list_apps());
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    v.extend(cpal_src::list_inputs());
    v.push(SourceInfo { source: Source::Tone, label: "เสียงทดสอบ (โทน 440 Hz)".into(), kind: SourceKind::Tone });
    v
}

/// Start capturing on a background thread. The thread restarts the source
/// by itself if the device disappears or errors, until the handle is dropped.
pub fn start(source: Source, opts: CaptureOptions, sink: SampleSink) -> CaptureHandle {
    let stop = Arc::new(AtomicBool::new(false));
    let error: Arc<parking_lot::Mutex<Option<String>>> = Arc::default();
    if source == Source::External {
        drop(sink);
        return CaptureHandle { stop, thread: None, error };
    }
    let t = {
        let stop = stop.clone();
        let error = error.clone();
        thread::Builder::new()
            .name("ssnd-capture".into())
            .spawn(move || run(source, opts, sink, stop, error))
            .ok()
    };
    CaptureHandle { stop, thread: t, error }
}

fn run(
    source: Source,
    opts: CaptureOptions,
    sink: SampleSink,
    stop: Arc<AtomicBool>,
    error: Arc<parking_lot::Mutex<Option<String>>>,
) {
    let sink = Arc::new(parking_lot::Mutex::new(sink));
    while !stop.load(Ordering::Relaxed) {
        let s = sink.clone();
        let deliver = move |d: &[f32], r: u32, c: usize| (s.lock())(d, r, c);
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_once(&source, &opts, Box::new(deliver), &stop)
        }));
        match res {
            Ok(Ok(())) => {
                *error.lock() = None;
            }
            Ok(Err(e)) => *error.lock() = Some(e.to_string()),
            Err(_) => *error.lock() = Some("capture thread panicked; restarting".into()),
        }
        // Back off a little before retrying so a missing device doesn't spin.
        let until = Instant::now() + Duration::from_millis(700);
        while Instant::now() < until && !stop.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(50));
        }
    }
}

/// Runs until `stop` is set (returns Ok) or the source fails (returns Err).
fn run_once(source: &Source, opts: &CaptureOptions, sink: SampleSink, stop: &AtomicBool) -> anyhow::Result<()> {
    match source {
        Source::Tone => run_tone(sink, stop),
        Source::External => Ok(()),
        #[cfg(target_os = "linux")]
        _ => linux::run(source, opts, sink, stop),
        #[cfg(windows)]
        Source::App { key } => windows_app::run(key, opts, sink, stop),
        #[cfg(target_os = "android")]
        _ => {
            let _ = opts;
            Err(anyhow::anyhow!("on Android the app captures audio itself"))
        }
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        _ => {
            let _ = opts;
            cpal_src::run(source, sink, stop)
        }
    }
}

fn run_tone(mut sink: SampleSink, stop: &AtomicBool) -> anyhow::Result<()> {
    const RATE: u32 = 48_000;
    const CHUNK: usize = 240;
    let mut phase = 0u64;
    let mut buf = vec![0.0f32; CHUNK * 2];
    let start = Instant::now();
    let mut sent = 0u64;
    while !stop.load(Ordering::Relaxed) {
        for f in 0..CHUNK {
            let t = phase as f32 / RATE as f32;
            // A soft A4 with a slow pulse so dropouts are easy to hear.
            let env = 0.6 + 0.4 * (2.0 * std::f32::consts::PI * 0.5 * t).sin();
            let v = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.25 * env;
            buf[2 * f] = v;
            buf[2 * f + 1] = v;
            phase += 1;
        }
        sink(&buf, RATE, 2);
        sent += CHUNK as u64;
        let due = start + Duration::from_micros(sent * 1_000_000 / RATE as u64);
        let now = Instant::now();
        if due > now {
            thread::sleep(due - now);
        }
    }
    Ok(())
}
