//! The engine ties everything together: a receiver (network -> jitter
//! buffers -> mixer -> speakers, with optional forwarding for daisy-chains)
//! and a sender (capture -> packets -> one or many destinations).

use crate::capture::{self, CaptureHandle, CaptureOptions, Source};
use crate::discovery::{Announce, Discovery, Peer};
use crate::jitter::{Mixer, Mode, StreamBuffer, StreamEntry, StreamStats};
use crate::proto::*;
use crate::scope::{Snapshot as ScopeSnapshot, Tap};
use anyhow::Context;
#[cfg(not(any(target_os = "linux", target_os = "android")))]
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
#[cfg(not(any(target_os = "linux", target_os = "android")))]
use cpal::{BufferSize, SampleFormat, StreamConfig};
use parking_lot::{Mutex, RwLock};
use socket2::{Domain, Protocol, Socket, Type};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const STREAM_TIMEOUT: Duration = Duration::from_secs(3);
/// No captured audio for this long means the source went quiet (nothing
/// playing); the next packet tells receivers not to count the gap as a glitch.
const SENDER_PAUSE: Duration = Duration::from_millis(80);
/// Largest UDP payload that fits a standard 1500-byte Ethernet/Wi-Fi frame.
const MAX_DATAGRAM: usize = 1472;

/// Frames the output device asked for in its most recent callback (diagnostics).
pub static OUTPUT_BLOCK_FRAMES: AtomicU64 = AtomicU64::new(0);
/// Output device sample rate (diagnostics).
pub static OUTPUT_RATE: AtomicU64 = AtomicU64::new(0);
/// Delay from handing audio to the speaker until it is heard, microseconds (0 = unknown).
pub static OUTPUT_LATENCY_US: AtomicU64 = AtomicU64::new(0);

fn mode_to_u8(m: Mode) -> u8 {
    m as u8
}

fn mode_from_u8(v: u8) -> Mode {
    Mode::ALL.into_iter().find(|m| *m as u8 == v).unwrap_or_default()
}

pub struct EngineConfig {
    pub name: String,
    pub audio_port: u16,
    pub mode: Mode,
    pub discovery: bool,
    /// The host app pulls audio with `Engine::render` instead of the engine
    /// opening the speaker itself (Android, iOS).
    pub external_output: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        EngineConfig {
            name: gethostname::gethostname().to_string_lossy().into_owned(),
            audio_port: DEFAULT_AUDIO_PORT,
            mode: Mode::default(),
            discovery: true,
            external_output: cfg!(target_os = "android"),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct SenderStats {
    pub active: bool,
    pub packets: u64,
    pub level: f32,
    pub error: Option<String>,
    /// Capture delay on this device, ms (0 = unknown).
    pub capture_ms: f64,
}

/// See [`Engine::renderer`].
#[derive(Clone)]
pub struct Renderer(Arc<Mutex<Mixer>>);

impl Renderer {
    /// Fill `out` (interleaved, `ch` channels at `rate`); silence when nothing is received.
    pub fn render(&self, out: &mut [f32], rate: u32, ch: usize) {
        self.0.lock().render(out, rate, ch);
    }

    pub fn mode(&self) -> Mode {
        self.0.lock().mode
    }
}

pub struct Engine {
    pub id: String,
    name: Arc<RwLock<String>>,
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
    external_output: bool,
    mode: Arc<AtomicU8>,
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
        let name = Arc::new(RwLock::new(cfg.name));
        let discovery = if cfg.discovery {
            Discovery::start(Announce {
                id: id.clone(),
                name: name.clone(),
                audio_port: cfg.audio_port,
                receiving: receiving_flag.clone(),
            })
            .ok()
        } else {
            None
        };
        Engine {
            id,
            name,
            audio_port: cfg.audio_port,
            discovery,
            receiving_flag,
            mixer: Arc::new(Mutex::new(Mixer::new(cfg.mode))),
            forward: Arc::default(),
            play_local: Arc::new(AtomicBool::new(true)),
            forwarded: Arc::default(),
            receiver: None,
            sender: None,
            send_dests: Arc::default(),
            external_output: cfg.external_output,
            mode: Arc::new(AtomicU8::new(mode_to_u8(cfg.mode))),
        }
    }

    /// This device's LAN addresses, the one other devices should use first.
    pub fn local_ips(&self) -> Vec<Ipv4Addr> {
        crate::discovery::local_ips()
    }

    /// Latency/stability trade-off for both receiving and sending.
    pub fn set_mode(&self, mode: Mode) {
        self.mode.store(mode_to_u8(mode), Ordering::Relaxed);
        self.mixer.lock().set_mode(mode);
    }

    pub fn mode(&self) -> Mode {
        mode_from_u8(self.mode.load(Ordering::Relaxed))
    }

    /// Speaker delay on this device, ms (0 = unknown).
    pub fn output_latency_ms(&self) -> f64 {
        OUTPUT_LATENCY_US.load(Ordering::Relaxed) as f64 / 1000.0
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
            !self.external_output,
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

    /// This device's name as other devices see it.
    pub fn name(&self) -> String {
        self.name.read().clone()
    }

    /// Rename this device; others see the new name within a second.
    pub fn set_name(&self, name: &str) {
        let name = name.trim();
        if !name.is_empty() {
            *self.name.write() = name.chars().take(60).collect();
        }
    }

    /// Recent audio on one side for drawing a waveform or frequency bars;
    /// `None` while that side is off.
    pub fn scope(&self, tap: Tap) -> Option<ScopeSnapshot> {
        match tap {
            Tap::Send => self.sender.as_ref().map(|s| s.packetizer.lock().scope.snapshot()),
            Tap::Receive => self.receiver.as_ref().map(|_| self.mixer.lock().scope.snapshot()),
        }
    }

    /// Something an audio callback the app owns (Android's AAudio player)
    /// can pull mixed audio from without going through the engine.
    pub fn renderer(&self) -> Renderer {
        Renderer(self.mixer.clone())
    }

    pub fn streams(&self) -> Vec<StreamStats> {
        self.mixer.lock().stats()
    }

    /// Pull mixed audio for the speaker when `external_output` is set.
    /// Fills `out` (interleaved, `ch` channels at `rate`) with silence when
    /// nothing is being received.
    pub fn render(&self, out: &mut [f32], rate: u32, ch: usize) {
        if self.receiver.is_some() {
            self.mixer.lock().render(out, rate, ch);
        } else {
            out.fill(0.0);
        }
    }

    /// Feed captured audio when sending from `Source::External`.
    pub fn push_capture(&self, data: &[f32], rate: u32, ch: usize) {
        if let Some(s) = &self.sender {
            s.packetizer.lock().push(data, rate, ch);
        }
    }

    // ---- sending ---------------------------------------------------------

    pub fn start_sending(&mut self, source: Source, dests: Vec<SocketAddr>, keep_local: bool) -> anyhow::Result<()> {
        self.stop_sending();
        *self.send_dests.write() = dests;
        let me = self.name();
        let name = match &source {
            Source::App { key } => format!("{me} · {key}"),
            Source::Input { name } => format!("{me} · {name}"),
            _ => me,
        };
        self.sender = Some(Sender::start(source, name, self.send_dests.clone(), keep_local, self.mode.clone())?);
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
                capture_ms: capture::CAPTURE_LATENCY_US.load(Ordering::Relaxed) as f64 / 1000.0,
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
        own_output: bool,
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
                crate::rt::boost_current_thread(crate::rt::Priority::Network);
                // Restart the loop if anything inside ever panics.
                while !stop.load(Ordering::Relaxed) {
                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        net_loop(&sock, &mixer, &forward, &play_local, &forwarded, &stop)
                    }));
                }
            })?);
        }
        if own_output {
            let stop = stop.clone();
            let error = error.clone();
            threads.push(thread::Builder::new().name("ssnd-output".into()).spawn(move || {
                crate::rt::boost_current_thread(crate::rt::Priority::Audio);
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
                let mode = m.mode;
                let entry = m.streams.entry(h.stream_id).or_insert_with(|| StreamEntry {
                    buf: StreamBuffer::new(h.sample_rate, h.channels as usize, mode),
                    name: from.ip().to_string(),
                    from: from.ip().to_string(),
                });
                if entry.buf.format() != (h.sample_rate, h.channels as usize) {
                    entry.buf = StreamBuffer::new(h.sample_rate, h.channels as usize, mode);
                }
                entry.buf.capture_ms = h.capture_delay as f64 / 10.0;
                entry.buf.push(h.seq, h.flags, &samples);
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
    use crate::pulse::{BufferAttr, Direction, Stream};
    const RATE: u32 = 48_000;
    const CH: usize = 2;
    const CHUNK: usize = 240; // 5 ms
    let bytes_per_ms = RATE * CH as u32 * 4 / 1000;
    let attr = BufferAttr { tlength: bytes_per_ms * 15, minreq: bytes_per_ms * 5, ..BufferAttr::default_all() };
    let s = Stream::open(Direction::Playback, None, "playback", RATE, CH as u8, &attr)
        .map_err(|e| anyhow::anyhow!("cannot open speakers: {e}"))?;
    OUTPUT_RATE.store(RATE as u64, Ordering::Relaxed);
    OUTPUT_BLOCK_FRAMES.store(CHUNK as u64, Ordering::Relaxed);
    let mut samples = vec![0.0f32; CHUNK * CH];
    let mut n = 0u32;
    while !stop.load(Ordering::Relaxed) {
        mixer.lock().render(&mut samples, RATE, CH);
        s.write(&samples).map_err(|e| anyhow::anyhow!("speaker write failed: {e}"))?;
        n = n.wrapping_add(1);
        if n % 100 == 1 {
            if let Some(us) = s.latency_us() {
                OUTPUT_LATENCY_US.store(us, Ordering::Relaxed);
            }
        }
    }
    Ok(())
}

#[cfg(target_os = "android")]
fn run_output(_mixer: &Arc<Mutex<Mixer>>, _stop: &AtomicBool) -> anyhow::Result<()> {
    anyhow::bail!("on Android the app plays audio itself via Engine::render")
}

/// Plays the mixer on the default output device until stopped or the
/// device fails (then the caller rebuilds it).
#[cfg(not(any(target_os = "linux", target_os = "android")))]
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

/// Speaker delay as the driver reports it: when this block will be heard.
#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn note_output(info: &cpal::OutputCallbackInfo) {
    let ts = info.timestamp();
    if let Some(d) = ts.playback.duration_since(&ts.callback) {
        OUTPUT_LATENCY_US.store(d.as_micros() as u64, Ordering::Relaxed);
    }
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
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
            move |out: &mut [f32], info: &cpal::OutputCallbackInfo| {
                note_output(info);
                OUTPUT_BLOCK_FRAMES.store((out.len() / ch.max(1)) as u64, Ordering::Relaxed);
                beat.fetch_add(1, Ordering::Relaxed);
                mixer.lock().render(out, rate, ch)
            },
            err_fn,
            None,
        )?,
        SampleFormat::I16 => device.build_output_stream(
            config,
            move |out: &mut [i16], info: &cpal::OutputCallbackInfo| {
                note_output(info);
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
            move |out: &mut [i32], info: &cpal::OutputCallbackInfo| {
                note_output(info);
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
    // Dropped first so capture stops before the packetizer goes away.
    capture: CaptureHandle,
    packetizer: Arc<Mutex<Packetizer>>,
    packets: Arc<AtomicU64>,
    level: Arc<AtomicU64>,
}

impl Sender {
    fn start(
        source: Source,
        name: String,
        dests: Arc<RwLock<Vec<SocketAddr>>>,
        keep_local: bool,
        mode: Arc<AtomicU8>,
    ) -> anyhow::Result<Sender> {
        let sock = udp_socket(0)?;
        sock.set_nonblocking(true)?;
        let packets = Arc::new(AtomicU64::new(0));
        let level = Arc::new(AtomicU64::new(0));
        let packetizer = Arc::new(Mutex::new(Packetizer {
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
            scope: Default::default(),
            mode,
            last_push: None,
            resume: true,
        }));
        let p = packetizer.clone();
        let capture = capture::start(source, CaptureOptions { keep_local }, Box::new(move |d, r, c| p.lock().push(d, r, c)));
        Ok(Sender { capture, packetizer, packets, level })
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
    /// What is being sent, for the visualizers.
    scope: crate::scope::Scope,
    mode: Arc<AtomicU8>,
    last_push: Option<Instant>,
    /// Mark the next packet as the first after a pause.
    resume: bool,
}

impl Packetizer {
    fn push(&mut self, data: &[f32], rate: u32, ch: usize) {
        if ch == 0 || rate == 0 {
            return;
        }
        let now = Instant::now();
        if self.last_push.is_some_and(|t| now.duration_since(t) > SENDER_PAUSE) {
            // The source went quiet (nothing playing) and came back.
            self.pending.clear();
            self.resume = true;
        }
        self.last_push = Some(now);
        self.scope.push(data, rate, ch);
        let out_ch = ch.min(2);
        // Keep first two channels (front L/R) for surround sources.
        for frame in data.chunks_exact(ch) {
            self.pending.extend_from_slice(&frame[..out_ch]);
            self.peak = self.peak.max(frame[0].abs());
        }
        // Packet length from the mode, but never bigger than one network frame.
        let ms = mode_from_u8(self.mode.load(Ordering::Relaxed)).packet_ms();
        let max_frames = (MAX_DATAGRAM - HEADER_LEN) / (out_ch * 2);
        let frames = ((rate as f64 * ms / 1000.0).round() as usize).clamp(1, max_frames);
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
            flags: if self.resume { FLAG_RESUME } else { 0 },
            capture_delay: (capture::CAPTURE_LATENCY_US.load(Ordering::Relaxed) / 100).min(u16::MAX as u64) as u16,
        };
        self.resume = false;
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The mobile path: the app pushes captured audio in and pulls mixed
    /// audio out, with the network in between.
    #[test]
    fn external_capture_to_external_render() {
        let cfg = |port| EngineConfig {
            name: "test".into(),
            audio_port: port,
            mode: Mode::Game,
            discovery: false,
            external_output: true,
        };
        let mut rx = Engine::new(cfg(47893));
        rx.start_receiving().unwrap();
        let mut tx = Engine::new(cfg(47894));
        tx.start_sending(Source::External, vec!["127.0.0.1:47893".parse().unwrap()], false).unwrap();

        let chunk: Vec<f32> = (0..240)
            .flat_map(|i| {
                let v = (i as f32 * 0.1).sin() * 0.5;
                [v, v]
            })
            .collect();
        for _ in 0..40 {
            tx.push_capture(&chunk, 48_000, 2);
            thread::sleep(Duration::from_millis(5));
        }
        thread::sleep(Duration::from_millis(50));

        let streams = rx.streams();
        assert_eq!(streams.len(), 1, "one incoming stream");
        assert_eq!(streams[0].lost, 0);
        let mut out = vec![0.0f32; 480 * 2];
        rx.render(&mut out, 48_000, 2);
        assert!(out.iter().any(|v| v.abs() > 0.2), "audio came out of render");
        assert!(tx.sender_stats().packets >= 40);
    }
}

#[cfg(test)]
mod delay_tests {
    use super::*;

    /// Real UDP in real time with Game-mode packets: a click must come out of
    /// render soon after its packet was sent.
    #[test]
    fn game_mode_click_delay_is_low() {
        let frames = 120usize;
        let mut rx = Engine::new(EngineConfig {
            name: "rx".into(),
            audio_port: 47895,
            mode: Mode::Game,
            discovery: false,
            external_output: true,
        });
        rx.start_receiving().unwrap();
        let sock = UdpSocket::bind("127.0.0.1:0").unwrap();
        let start = Instant::now();
        let mut sent_frames = 0usize;
        let mut rendered = 0usize;
        let mut seq = 0u32;
        let mut out = vec![0.0f32; 240 * 2];
        let mut delays = Vec::new();
        let mut last_click_sent: Option<Instant> = None;
        let mut buf = vec![0u8; MAX_PACKET];
        while start.elapsed() < Duration::from_secs(4) {
            let now_frames = (start.elapsed().as_secs_f64() * 48_000.0) as usize;
            while sent_frames + frames <= now_frames {
                let samples: Vec<f32> = (0..frames)
                    .flat_map(|i| {
                        let v = if (sent_frames + i) % 19_200 < 48 { 0.6 } else { 0.0 };
                        [v, v]
                    })
                    .collect();
                if samples.iter().any(|v| *v > 0.0) {
                    last_click_sent = Some(Instant::now());
                }
                let h = Header {
                    kind: KIND_AUDIO,
                    ttl: 1,
                    channels: 2,
                    frames: frames as u16,
                    stream_id: 7,
                    seq,
                    sample_rate: 48_000,
                    codec: CODEC_PCM16,
                    flags: 0,
                    capture_delay: 0,
                };
                h.write(&mut buf);
                let len = HEADER_LEN + samples.len() * 2;
                encode_pcm16(&samples, &mut buf[HEADER_LEN..len]);
                sock.send_to(&buf[..len], "127.0.0.1:47895").unwrap();
                seq += 1;
                sent_frames += frames;
            }
            while rendered + 240 <= now_frames {
                rx.render(&mut out, 48_000, 2);
                rendered += 240;
                if out.iter().any(|v| v.abs() > 0.3) {
                    if let Some(t) = last_click_sent.take() {
                        delays.push(t.elapsed().as_secs_f64() * 1000.0);
                    }
                }
            }
            thread::sleep(Duration::from_micros(200));
        }
        assert!(delays.len() >= 5, "clicks came through: {delays:?}");
        delays.sort_by(|a, b| a.total_cmp(b));
        let median = delays[delays.len() / 2];
        assert!(median < 60.0, "median click delay {median:.1} ms: {delays:?}");
    }
}
