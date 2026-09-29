//! Speaker output through AAudio, the lowest-latency audio path Android has.
//!
//! The stream asks for the low-latency path (MMAP where the phone supports
//! it) and starts with the smallest buffer that plays cleanly, growing it one
//! burst at a time whenever the speaker runs dry. The app falls back to its
//! AudioTrack loop when AAudio can't be opened at all.

use ndk::audio::{
    AudioCallbackResult, AudioContentType, AudioDirection, AudioFormat, AudioPerformanceMode,
    AudioSharingMode, AudioStream, AudioStreamBuilder, AudioUsage, Clockid,
};
use ssnd_core::engine::{OUTPUT_BLOCK_FRAMES, OUTPUT_LATENCY_US, OUTPUT_RATE};
use ssnd_core::{Mode, Renderer};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::JoinHandle;
use std::time::Duration;

/// Reopen attempts in a row before giving up and letting the app fall back.
const MAX_FAILURES: u32 = 10;

pub struct Player {
    stop: Arc<AtomicBool>,
    failed: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Player {
    /// Opens the speaker and keeps it playing (reopening it after headphones,
    /// Bluetooth or mode changes) until dropped.
    pub fn start(renderer: Renderer) -> Result<Player, String> {
        let stop = Arc::new(AtomicBool::new(false));
        let failed = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let (st, fl) = (stop.clone(), failed.clone());
        let thread = std::thread::Builder::new()
            .name("ssnd-aaudio".into())
            .spawn(move || {
                let _ = catch_unwind(AssertUnwindSafe(|| supervise(&renderer, &st, tx)));
                fl.store(true, Ordering::Relaxed);
            })
            .map_err(|e| e.to_string())?;
        match rx.recv_timeout(Duration::from_secs(3)) {
            Ok(Ok(())) => Ok(Player { stop, failed, thread: Some(thread) }),
            Ok(Err(e)) => {
                let _ = thread.join();
                Err(e)
            }
            Err(_) => {
                stop.store(true, Ordering::Relaxed);
                Err("AAudio did not start".into())
            }
        }
    }

    /// True once the speaker could not be reopened; the app should switch to AudioTrack.
    pub fn failed(&self) -> bool {
        self.failed.load(Ordering::Relaxed)
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn supervise(renderer: &Renderer, stop: &AtomicBool, ready: mpsc::Sender<Result<(), String>>) {
    let mut ready = Some(ready);
    let mut failures = 0;
    while !stop.load(Ordering::Relaxed) {
        let exclusive = renderer.mode() == Mode::Game;
        // Opening, running and closing all stay on this thread; a panic in
        // one round (the ndk crate unwraps close errors) only costs a reopen.
        let round = catch_unwind(AssertUnwindSafe(|| match Output::open(renderer, exclusive) {
            Ok(out) => {
                if let Some(r) = ready.take() {
                    let _ = r.send(Ok(()));
                }
                out.run(renderer, stop, exclusive);
                Ok(())
            }
            Err(e) => Err(e),
        }));
        match round {
            Ok(Ok(())) => failures = 0,
            Ok(Err(e)) => {
                if let Some(r) = ready.take() {
                    let _ = r.send(Err(e));
                    return;
                }
                failures += 1;
            }
            Err(_) => failures += 1,
        }
        if failures >= MAX_FAILURES {
            return;
        }
        if failures > 0 {
            std::thread::sleep(Duration::from_millis(300));
        }
    }
}

struct Output {
    stream: AudioStream,
    disconnected: Arc<AtomicBool>,
}

impl Output {
    fn open(renderer: &Renderer, exclusive: bool) -> Result<Output, String> {
        let mut last = String::new();
        for format in [AudioFormat::PCM_Float, AudioFormat::PCM_I16] {
            match Self::open_with(renderer, exclusive, format) {
                Ok(o) => return Ok(o),
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    fn open_with(renderer: &Renderer, exclusive: bool, format: AudioFormat) -> Result<Output, String> {
        let disconnected = Arc::new(AtomicBool::new(false));
        let flag = disconnected.clone();
        let r = renderer.clone();
        let mut scratch: Vec<f32> = vec![0.0; 8192];
        let builder = AudioStreamBuilder::new()
            .map_err(|e| format!("AAudio: {e}"))?
            .direction(AudioDirection::Output)
            .performance_mode(AudioPerformanceMode::LowLatency)
            .sharing_mode(if exclusive { AudioSharingMode::Exclusive } else { AudioSharingMode::Shared })
            .usage(AudioUsage::Media)
            .content_type(AudioContentType::Music)
            .format(format)
            .channel_count(2)
            .data_callback(Box::new(move |stream, data, frames| {
                let ch = stream.channel_count().max(1) as usize;
                let rate = stream.sample_rate().max(8000) as u32;
                let n = frames.max(0) as usize * ch;
                let ok = catch_unwind(AssertUnwindSafe(|| {
                    if format == AudioFormat::PCM_Float {
                        // SAFETY: AAudio hands us `frames` frames of `ch` floats.
                        let out = unsafe { std::slice::from_raw_parts_mut(data as *mut f32, n) };
                        r.render(out, rate, ch);
                    } else {
                        if scratch.len() < n {
                            scratch.resize(n, 0.0);
                        }
                        r.render(&mut scratch[..n], rate, ch);
                        // SAFETY: as above, 16-bit samples.
                        let out = unsafe { std::slice::from_raw_parts_mut(data as *mut i16, n) };
                        for (o, s) in out.iter_mut().zip(&scratch[..n]) {
                            *o = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
                        }
                    }
                }));
                if ok.is_err() {
                    let bytes = n * if format == AudioFormat::PCM_Float { 4 } else { 2 };
                    // SAFETY: the buffer is `bytes` long; silence rather than garbage.
                    unsafe { std::ptr::write_bytes(data as *mut u8, 0, bytes) };
                }
                AudioCallbackResult::Continue
            }))
            .error_callback(Box::new(move |_stream, _err| {
                // Device changed or went away: the supervisor reopens it.
                flag.store(true, Ordering::Relaxed);
            }));
        let stream = builder.open_stream().map_err(|e| format!("AAudio: {e}"))?;
        let burst = stream.frames_per_burst().max(16);
        let bursts = if renderer.mode() == Mode::Music { 4 } else { 2 };
        let _ = stream.set_buffer_size_in_frames(burst * bursts);
        stream.request_start().map_err(|e| format!("AAudio start: {e}"))?;
        OUTPUT_RATE.store(stream.sample_rate().max(0) as u64, Ordering::Relaxed);
        OUTPUT_BLOCK_FRAMES.store(burst as u64, Ordering::Relaxed);
        Ok(Output { stream, disconnected })
    }

    /// Plays until stopped, disconnected or the wanted sharing mode changes.
    fn run(self, renderer: &Renderer, stop: &AtomicBool, exclusive: bool) {
        let s = &self.stream;
        let burst = s.frames_per_burst().max(16);
        let capacity = s.buffer_capacity_in_frames().max(burst);
        let rate = s.sample_rate().max(8000) as f64;
        let mut xruns = s.x_run_count();
        let mut tick = 0u32;
        while !stop.load(Ordering::Relaxed) && !self.disconnected.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(50));
            if (renderer.mode() == Mode::Game) != exclusive {
                break;
            }
            // Ran dry: one more burst of headroom, up to what the device allows.
            let now = s.x_run_count();
            if now > xruns {
                xruns = now;
                let size = s.buffer_size_in_frames();
                if size < capacity {
                    let _ = s.set_buffer_size_in_frames((size + burst).min(capacity));
                }
            }
            tick = tick.wrapping_add(1);
            if tick % 4 == 0 {
                if let Some(us) = latency_us(s, rate) {
                    OUTPUT_LATENCY_US.store(us, Ordering::Relaxed);
                }
            }
        }
        let _ = s.request_stop();
    }
}

/// Time from writing a frame to it leaving the speaker, from AAudio's timestamp.
fn latency_us(s: &AudioStream, rate: f64) -> Option<u64> {
    let ts = s.timestamp(Clockid::Monotonic).ok()?;
    let now = monotonic_ns();
    let written = s.frames_written() as f64;
    let playing_now = ts.frame_position as f64 + (now - ts.time_nanoseconds) as f64 * rate / 1e9;
    let frames = written - playing_now;
    (frames > 0.0).then(|| (frames * 1e6 / rate) as u64)
}

fn monotonic_ns() -> i64 {
    let mut t = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: plain syscall into a local struct.
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut t) };
    t.tv_sec as i64 * 1_000_000_000 + t.tv_nsec as i64
}
