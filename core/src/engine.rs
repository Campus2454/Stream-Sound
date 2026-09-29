//! The engine ties everything together: a receiver (network -> jitter
//! buffers -> mixer -> speakers, with optional forwarding for daisy-chains)
//! and a sender (capture -> packets -> one or many destinations).

use crate::capture::{self, CaptureHandle, CaptureOptions, Source};
use crate::discovery::{Announce, Discovery, Peer};
use crate::jitter::{Mixer, StreamBuffer, StreamEntry, StreamStats};
use crate::proto::*;
use anyhow::Context;
#[cfg(not(target_os = "linux"))]
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
#[cfg(not(target_os = "linux"))]
use cpal::{BufferSize, SampleFormat, StreamConfig};
use parking_lot::{Mutex, RwLock};
use socket2::{Domain, Protocol, Socket, Type};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const STREAM_TIMEOUT: Duration = Duration::from_secs(2);

/// Frames the output device asked for in its most recent callback (diagnostics).
pub static OUTPUT_BLOCK_FRAMES: AtomicU64 = AtomicU64::new(0);
/// Output device sample rate (diagnostics).
pub static OUTPUT_RATE: AtomicU64 = AtomicU64::new(0);

pub struct EngineConfig {
    pub name: String,
    pub audio_port: u16,
    pub latency_ms: f64,
    pub discovery: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        EngineConfig {
            name: gethostname::gethostname().to_string_lossy().into_owned(),
            audio_port: DEFAULT_AUDIO_PORT,
            latency_ms: 20.0,
            discovery: true,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct SenderStats {
    pub active: bool,
    pub packets: u64,
    pub level: f32,
    pub error: Option<String>,
}

pub struct Engine {
    pub id: String,
    pub name: String,
    pub audio_port: u16,
    discovery: Option<Discovery>,
    receiving_flag: Arc<AtomicBool>,
    mixer: Arc<Mutex<Mixer>>,
    forward: Arc<RwLock<Vec<SocketAddr>>>,
    play_local: Arc<AtomicBool>,
    forwarded: Arc<AtomicU64>,
    receiver: Option<Receiver>,
    sender: Option<Sender>,
    send_dests: Arc<RwLock<Vec<SocketAddr>>>,
}

fn random_u32() -> u32 {
    let n = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let x = (n as u64) ^ ((std::process::id() as u64) << 32) ^ (&n as *const _ as u64);
    // splitmix64
    let mut z = x.wrapping_add(0x9E3779B97F4A7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    (z ^ (z >> 31)) as u32
}

impl Engine {
    pub fn new(cfg: EngineConfig) -> Engine {
        let id = format!("{:08x}", random_u32());
        let receiving_flag = Arc::new(AtomicBool::new(false));
        let discovery = if cfg.discovery {
            Discovery::start(Announce {
                id: id.clone(),
                name: cfg.name.clone(),
                audio_port: cfg.audio_port,
                receiving: receiving_flag.clone(),
            })
            .ok()
        } else {
            None
        };
        Engine {
            id,
            name: cfg.name,
            audio_port: cfg.audio_port,
            discovery,
            receiving_flag,
            mixer: Arc::new(Mutex::new(Mixer::new(cfg.latency_ms))),
            forward: Arc::default(),
            play_local: Arc::new(AtomicBool::new(true)),
            forwarded: Arc::default(),
            receiver: None,
            sender: None,
            send_dests: Arc::default(),
        }
    }

    pub fn peers(&self) -> Vec<Peer> {
        self.discovery.as_ref().map(|d| d.peers(&self.id)).unwrap_or_default()
    }

    // ---- receiving -------------------------------------------------------

    pub fn start_receiving(&mut self) -> anyhow::Result<()> {
        if self.receiver.is_some() {
            return Ok(());
        }
        self.receiver = Some(Receiver::start(
            self.audio_port,
            self.mixer.clone(),
            self.forward.clone(),
            self.play_local.clone(),
            self.forwarded.clone(),
        )?);
        self.receiving_flag.store(true, Ordering::Relaxed);
        Ok(())
    }

    pub fn stop_receiving(&mut self) {
        self.receiver = None;
        self.receiving_flag.store(false, Ordering::Relaxed);
        self.mixer.lock().streams.clear();
    }

    pub fn is_receiving(&self) -> bool {
        self.receiver.is_some()
    }

    pub fn receiver_error(&self) -> Option<String> {
        self.receiver.as_ref().and_then(|r| r.error.lock().clone())
    }

    /// Where incoming audio is passed on to (daisy-chain). Empty = no forwarding.
    pub fn set_forward(&self, dests: Vec<SocketAddr>) {
        *self.forward.write() = dests;
    }

    pub fn forward_targets(&self) -> Vec<SocketAddr> {
        self.forward.read().clone()
    }

    pub fn forwarded_packets(&self) -> u64 {
        self.forwarded.load(Ordering::Relaxed)
    }

    pub fn set_play_local(&self, on: bool) {
        self.play_local.store(on, Ordering::Relaxed);
        if !on {
            self.mixer.lock().streams.clear();
        }
    }

    pub fn play_local(&self) -> bool {
        self.play_local.load(Ordering::Relaxed)
    }

    pub fn set_volume(&self, v: f32) {
        self.mixer.lock().volume = v.clamp(0.0, 2.0);
    }

    pub fn set_latency_ms(&self, ms: f64) {
        self.mixer.lock().set_base_target(ms.clamp(5.0, 250.0));
    }

    pub fn streams(&self) -> Vec<StreamStats> {
        self.mixer.lock().stats()
    }

    // ---- sending ---------------------------------------------------------

    pub fn start_sending(&mut self, source: Source, dests: Vec<SocketAddr>, keep_local: bool) -> anyhow::Result<()> {
        self.stop_sending();
        *self.send_dests.write() = dests;
        let name = match &source {
            Source::App { key } => format!("{} · {}", self.name, key),
            Source::Input { name } => format!("{} · {}", self.name, name),
            _ => self.name.clone(),
        };
        self.sender = Some(Sender::start(source, name, self.send_dests.clone(), keep_local)?);
        Ok(())
    }

    pub fn set_send_dests(&self, dests: Vec<SocketAddr>) {
        *self.send_dests.write() = dests;
    }

    pub fn send_dests(&self) -> Vec<SocketAddr> {
        self.send_dests.read().clone()
    }

    pub fn stop_sending(&mut self) {
        self.sender = None;
    }

    pub fn sender_stats(&self) -> SenderStats {
        match &self.sender {
            None => SenderStats::default(),
            Some(s) => SenderStats {
                active: true,
                packets: s.packets.load(Ordering::Relaxed),
                level: f32::from_bits(s.level.load(Ordering::Relaxed) as u32),
                error: s.capture.last_error(),
            },
        }
    }
}

// ---- receiver ----------------------------------------------------------------

struct Receiver {
    stop: Arc<AtomicBool>,
    threads: Vec<thread::JoinHandle<()>>,
    error: Arc<Mutex<Option<String>>>,
}

impl Drop for Receiver {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}

fn udp_socket(port: u16) -> anyhow::Result<UdpSocket> {
    let s = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    let _ = s.set_recv_buffer_size(1 << 20);
    let _ = s.set_send_buffer_size(1 << 20);
    s.bind(&SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port)).into())
        .with_context(|| format!("port {port} is already in use (is another copy running?)"))?;
    Ok(s.into())
}

impl Receiver {
    fn start(
        port: u16,
        mixer: Arc<Mutex<Mixer>>,
        forward: Arc<RwLock<Vec<SocketAddr>>>,
        play_local: Arc<AtomicBool>,
        forwarded: Arc<AtomicU64>,
    ) -> anyhow::Result<Receiver> {
        let sock = udp_socket(port)?;
        sock.set_read_timeout(Some(Duration::from_millis(100)))?;
        let stop = Arc::new(AtomicBool::new(false));
        let error: Arc<Mutex<Option<String>>> = Arc::default();
        let mut threads = Vec::new();

        {
            let stop = stop.clone();
            let mixer = mixer.clone();
            threads.push(thread::Builder::new().name("ssnd-net-rx".into()).spawn(move || {
                // Restart the loop if anything inside ever panics.
                while !stop.load(Ordering::Relaxed) {
                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        net_loop(&sock, &mixer, &forward, &play_local, &forwarded, &stop)
                    }));
                }
            })?);
        }
        {
            let stop = stop.clone();
            let error = error.clone();
            threads.push(thread::Builder::new().name("ssnd-output".into()).spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    match run_output(&mixer, &stop) {
                        Ok(()) => *error.lock() = None,
                        Err(e) => *error.lock() = Some(format!("{e:#}")),
                    }
                    if !stop.load(Ordering::Relaxed) {
                        thread::sleep(Duration::from_millis(500));
                    }
                }
            })?);
        }
        Ok(Receiver { stop, threads, error })
    }
}

fn net_loop(
    sock: &UdpSocket,
    mixer: &Mutex<Mixer>,
    forward: &RwLock<Vec<SocketAddr>>,
    play_local: &AtomicBool,
    forwarded: &AtomicU64,
    stop: &AtomicBool,
) {
    let mut buf = [0u8; MAX_PACKET];
    let mut samples: Vec<f32> = Vec::with_capacity(4096);
    let mut last_gc = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        if last_gc.elapsed() > Duration::from_millis(500) {
            last_gc = Instant::now();
            let stale = mixer.lock().take_stale(STREAM_TIMEOUT);
            drop(stale); // freed here, not in the audio callback
        }
        let (n, from) = match sock.recv_from(&mut buf) {
            Ok(v) => v,
            Err(_) => continue, // timeout
        };
        let pkt = &mut buf[..n];
        let Some(h) = Header::parse(pkt) else { continue };

        // Daisy-chain: pass the packet on untouched (minus one hop) before
        // doing anything else, so each hop adds almost no delay.
        if h.ttl > 1 {
            let fw = forward.read();
            if !fw.is_empty() {
                pkt[4] = h.ttl - 1;
                for d in fw.iter() {
                    if *d != from && sock.send_to(pkt, d).is_ok() {
                        forwarded.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }
        if !play_local.load(Ordering::Relaxed) {
            continue;
        }

        match h.kind {
            KIND_AUDIO => {
                samples.clear();
                let len = h.frames as usize * h.channels as usize * 2;
                decode_pcm16(&pkt[HEADER_LEN..HEADER_LEN + len], &mut samples);
                let mut m = mixer.lock();
                let base = m.base_target_ms;
                let entry = m.streams.entry(h.stream_id).or_insert_with(|| StreamEntry {
                    buf: StreamBuffer::new(h.sample_rate, h.channels as usize, base),
                    name: from.ip().to_string(),
                    from: from.ip().to_string(),
                });
                if entry.buf.format() != (h.sample_rate, h.channels as usize) {
                    entry.buf = StreamBuffer::new(h.sample_rate, h.channels as usize, base);
                }
                entry.buf.push(h.seq, &samples);
            }
            KIND_INFO => {
                let name = String::from_utf8_lossy(&pkt[HEADER_LEN..]).into_owned();
                if let Some(e) = mixer.lock().streams.get_mut(&h.stream_id) {
                    if e.name != name {
                        e.name = name;
                    }
                }
            }
            _ => {}
        }
    }
}

/// Plays the mixer until stopped or the device fails (then the caller rebuilds it).
#[cfg(target_os = "linux")]
fn run_output(mixer: &Arc<Mutex<Mixer>>, stop: &AtomicBool) -> anyhow::Result<()> {
    // On Linux talk to PulseAudio/PipeWire directly: it paces us precisely and
    // avoids the ALSA compatibility layer, which can glitch at small buffers.
    use libpulse_binding::def::BufferAttr;
    use libpulse_binding::sample::{Format, Spec};
    use libpulse_binding::stream::Direction;
    use libpulse_simple_binding::Simple;
    const RATE: u32 = 48_000;
    const CH: usize = 2;
    const CHUNK: usize = 240; // 5 ms
    let spec = Spec { format: Format::FLOAT32NE, channels: CH as u8, rate: RATE };
    let bytes_per_ms = RATE as u32 * CH as u32 * 4 / 1000;
    let attr = BufferAttr {
        maxlength: u32::MAX,
        tlength: bytes_per_ms * 15,
        prebuf: u32::MAX,
        minreq: bytes_per_ms * 5,
        fragsize: u32::MAX,
    };
    let s = Simple::new(None, "Stream Sound", Direction::Playback, None, "playback", &spec, None, Some(&attr))
        .map_err(|e| anyhow::anyhow!("cannot open speakers: {e}"))?;
    OUTPUT_RATE.store(RATE as u64, Ordering::Relaxed);
    OUTPUT_BLOCK_FRAMES.store(CHUNK as u64, Ordering::Relaxed);
    let mut samples = vec![0.0f32; CHUNK * CH];
    let mut bytes = vec![0u8; CHUNK * CH * 4];
    while !stop.load(Ordering::Relaxed) {
        mixer.lock().render(&mut samples, RATE, CH);
        for (b, v) in bytes.chunks_exact_mut(4).zip(samples.iter()) {
            b.copy_from_slice(&v.to_ne_bytes());
        }
        s.write(&bytes).map_err(|e| anyhow::anyhow!("speaker write failed: {e}"))?;
    }
    Ok(())
}

/// Plays the mixer on the default output device until stopped or the
/// device fails (then the caller rebuilds it).
#[cfg(not(target_os = "linux"))]
fn run_output(mixer: &Arc<Mutex<Mixer>>, stop: &AtomicBool) -> anyhow::Result<()> {
    let host = cpal::default_host();
    let device = host.default_output_device().context("no speaker/output device")?;
    let supported = device.default_output_config()?;
    let rate = supported.sample_rate().0;
    let ch = supported.channels() as usize;
    let format = supported.sample_format();
    let failed = Arc::new(AtomicBool::new(false));
    OUTPUT_RATE.store(rate as u64, Ordering::Relaxed);

    // Try a small fixed buffer for low latency, fall back to the device default.
    let mut last_err = None;
    for size in [Some(256u32), None] {
        let config = StreamConfig {
            channels: ch as u16,
            sample_rate: cpal::SampleRate(rate),
            buffer_size: size.map(BufferSize::Fixed).unwrap_or(BufferSize::Default),
        };
        let beat = Arc::new(AtomicU64::new(0));
        match build_output(&device, &config, format, mixer.clone(), rate, ch, failed.clone(), beat.clone()) {
            Ok(stream) => {
                stream.play()?;
                let mut last_beat = 0;
                let mut last_change = Instant::now();
                while !stop.load(Ordering::Relaxed) {
                    if failed.load(Ordering::Relaxed) {
                        anyhow::bail!("output device changed or was unplugged; reconnecting");
                    }
                    // Watchdog: a stream that stops calling us is dead.
                    let b = beat.load(Ordering::Relaxed);
                    if b != last_beat {
                        last_beat = b;
                        last_change = Instant::now();
                    } else if last_change.elapsed() > Duration::from_millis(1500) {
                        anyhow::bail!("output stalled; reconnecting");
                    }
                    thread::sleep(Duration::from_millis(100));
                }
                return Ok(());
            }
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("could not open output")))
}

#[cfg(not(target_os = "linux"))]
#[allow(clippy::too_many_arguments)]
fn build_output(
    device: &cpal::Device,
    config: &StreamConfig,
    format: SampleFormat,
    mixer: Arc<Mutex<Mixer>>,
    rate: u32,
    ch: usize,
    failed: Arc<AtomicBool>,
    beat: Arc<AtomicU64>,
) -> anyhow::Result<cpal::Stream> {
    // Only a lost device is fatal; glitches (xruns) are recovered by the backend.
    let err_fn = move |e: cpal::StreamError| {
        if matches!(e, cpal::StreamError::DeviceNotAvailable) {
            failed.store(true, Ordering::Relaxed)
        }
    };
    let mut scratch: Vec<f32> = vec![0.0; 8192];
    let stream = match format {
        SampleFormat::F32 => device.build_output_stream(
            config,
            move |out: &mut [f32], _| {
                OUTPUT_BLOCK_FRAMES.store((out.len() / ch.max(1)) as u64, Ordering::Relaxed);
                beat.fetch_add(1, Ordering::Relaxed);
                mixer.lock().render(out, rate, ch)
            },
            err_fn,
            None,
        )?,
        SampleFormat::I16 => device.build_output_stream(
            config,
            move |out: &mut [i16], _| {
                if scratch.len() < out.len() {
                    scratch.resize(out.len(), 0.0);
                }
                let s = &mut scratch[..out.len()];
                OUTPUT_BLOCK_FRAMES.store((out.len() / ch.max(1)) as u64, Ordering::Relaxed);
                beat.fetch_add(1, Ordering::Relaxed);
                mixer.lock().render(s, rate, ch);
                for (o, v) in out.iter_mut().zip(s.iter()) {
                    *o = (v.clamp(-1.0, 1.0) * 32767.0) as i16;
                }
            },
            err_fn,
            None,
        )?,
        SampleFormat::I32 => device.build_output_stream(
            config,
            move |out: &mut [i32], _| {
                if scratch.len() < out.len() {
                    scratch.resize(out.len(), 0.0);
                }
                let s = &mut scratch[..out.len()];
                OUTPUT_BLOCK_FRAMES.store((out.len() / ch.max(1)) as u64, Ordering::Relaxed);
                beat.fetch_add(1, Ordering::Relaxed);
                mixer.lock().render(s, rate, ch);
                for (o, v) in out.iter_mut().zip(s.iter()) {
                    *o = (*v as f64 * 2147483647.0).clamp(-2147483648.0, 2147483647.0) as i32;
                }
            },
            err_fn,
            None,
        )?,
        f => anyhow::bail!("unsupported output format {f:?}"),
    };
    Ok(stream)
}

// ---- sender ------------------------------------------------------------------

struct Sender {
    capture: CaptureHandle,
    packets: Arc<AtomicU64>,
    level: Arc<AtomicU64>,
}

impl Sender {
    fn start(source: Source, name: String, dests: Arc<RwLock<Vec<SocketAddr>>>, keep_local: bool) -> anyhow::Result<Sender> {
        let sock = udp_socket(0)?;
        sock.set_nonblocking(true)?;
        let packets = Arc::new(AtomicU64::new(0));
        let level = Arc::new(AtomicU64::new(0));
        let mut p = Packetizer {
            sock,
            dests,
            stream_id: random_u32(),
            seq: 0,
            pending: Vec::with_capacity(4096),
            buf: vec![0u8; MAX_PACKET],
            name,
            packets: packets.clone(),
            level: level.clone(),
            peak: 0.0,
        };
        let capture = capture::start(source, CaptureOptions { keep_local }, Box::new(move |d, r, c| p.push(d, r, c)));
        Ok(Sender { capture, packets, level })
    }
}

struct Packetizer {
    sock: UdpSocket,
    dests: Arc<RwLock<Vec<SocketAddr>>>,
    stream_id: u32,
    seq: u32,
    /// Stereo (or mono) samples waiting to fill a packet.
    pending: Vec<f32>,
    buf: Vec<u8>,
    name: String,
    packets: Arc<AtomicU64>,
    level: Arc<AtomicU64>,
    peak: f32,
}

impl Packetizer {
    fn push(&mut self, data: &[f32], rate: u32, ch: usize) {
        if ch == 0 || rate == 0 {
            return;
        }
        let out_ch = ch.min(2);
        // Keep first two channels (front L/R) for surround sources.
        for frame in data.chunks_exact(ch) {
            self.pending.extend_from_slice(&frame[..out_ch]);
            self.peak = self.peak.max(frame[0].abs());
        }
        // 5 ms packets: small enough for low latency, big enough for Wi-Fi.
        let frames = (rate / 200).max(1) as usize;
        let per = frames * out_ch;
        while self.pending.len() >= per {
            self.send(&self.pending[..per].to_vec(), frames, out_ch, rate);
            self.pending.drain(..per);
        }
    }

    fn send(&mut self, samples: &[f32], frames: usize, ch: usize, rate: u32) {
        let dests = self.dests.read();
        let h = Header {
            kind: KIND_AUDIO,
            ttl: DEFAULT_TTL,
            channels: ch as u8,
            frames: frames as u16,
            stream_id: self.stream_id,
            seq: self.seq,
            sample_rate: rate,
            codec: CODEC_PCM16,
        };
        let len = HEADER_LEN + samples.len() * 2;
        if len > self.buf.len() {
            return;
        }
        h.write(&mut self.buf);
        encode_pcm16(samples, &mut self.buf[HEADER_LEN..len]);
        for d in dests.iter() {
            let _ = self.sock.send_to(&self.buf[..len], d);
        }
        // Tell receivers our name about once a second.
        if self.seq % 200 == 0 {
            let name = self.name.as_bytes();
            let n = name.len().min(200);
            let info = Header { kind: KIND_INFO, frames: 0, ..h };
            let mut ib = vec![0u8; HEADER_LEN + n];
            info.write(&mut ib);
            ib[HEADER_LEN..].copy_from_slice(&name[..n]);
            for d in dests.iter() {
                let _ = self.sock.send_to(&ib, d);
            }
        }
        self.seq = self.seq.wrapping_add(1);
        self.packets.fetch_add(1, Ordering::Relaxed);
        self.level.store(self.peak.to_bits() as u64, Ordering::Relaxed);
        self.peak *= 0.8;
    }
}
