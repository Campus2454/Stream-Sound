//! Per-source jitter buffer with adaptive delay, loss concealment,
//! clock-drift correction and resampling, plus the mixer that sums every
//! incoming source into the output device.
//!
//! How much audio to keep queued is measured, not guessed: after every
//! render the buffer notes how much was left over. Network jitter, senders
//! that deliver in bursts and sound cards that pull in big blocks all show
//! up as dips in that number. The buffer keeps just enough queued to ride
//! out the dips seen recently, plus a safety margin set by the [`Mode`].

use crate::proto::FLAG_RESUME;
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

/// Latency versus stability trade-off, chosen per device.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
pub enum Mode {
    /// Lowest delay. Audio that arrives too late is dropped so sound stays
    /// in sync with the picture; rare Wi-Fi hiccups are heard as short glitches.
    Game,
    #[default]
    Balanced,
    /// Largest safety margin, slow and gentle corrections, never drops audio.
    Music,
}

impl Mode {
    pub const ALL: [Mode; 3] = [Mode::Game, Mode::Balanced, Mode::Music];

    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Game => "game",
            Mode::Balanced => "balanced",
            Mode::Music => "music",
        }
    }

    pub fn parse(s: &str) -> Option<Mode> {
        Mode::ALL.into_iter().find(|m| m.as_str() == s.trim())
    }

    /// Audio per network packet when this device sends.
    pub fn packet_ms(self) -> f64 {
        match self {
            Mode::Game => 2.5,
            Mode::Balanced => 5.0,
            Mode::Music => 5.0,
        }
    }

    fn tuning(self) -> Tuning {
        match self {
            Mode::Game => Tuning {
                margin_ms: 2.0,
                start_ms: 10.0,
                history: 6,
                ignore_worst: 1,
                max_speed: 0.02,
                skip_over_ms: 5.0,
                drop_late: true,
            },
            Mode::Balanced => Tuning {
                margin_ms: 5.0,
                start_ms: 25.0,
                history: 20,
                ignore_worst: 0,
                max_speed: 0.01,
                skip_over_ms: 20.0,
                drop_late: false,
            },
            Mode::Music => Tuning {
                margin_ms: 15.0,
                start_ms: 60.0,
                history: 60,
                ignore_worst: 0,
                max_speed: 0.004,
                skip_over_ms: 80.0,
                drop_late: false,
            },
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Tuning {
    /// Audio kept in reserve below the deepest recent dip.
    margin_ms: f64,
    /// Target before anything has been measured.
    start_ms: f64,
    /// Seconds of dips remembered.
    history: usize,
    /// How many of the worst seconds to ignore (accept a rare glitch).
    ignore_worst: usize,
    /// Largest playback speed change used to drift back to target.
    max_speed: f64,
    /// Above this much extra, cut the extra out with a crossfade.
    skip_over_ms: f64,
    /// After a stall, throw away the audio that arrived too late.
    drop_late: bool,
}

const WINDOW_SECS: f64 = 1.0;
const MAX_TARGET_MS: f64 = 400.0;
/// Missing packets filled in with concealment; bigger gaps are skipped.
const MAX_CONCEAL_MS: f64 = 60.0;
/// Starved this long: the sender went quiet, start over when it returns.
const PAUSE_SECS: f64 = 1.0;
const FADE_MS: f64 = 5.0;
const XFADE_MS: f64 = 2.5;
/// Concealment fades out with this time constant.
const PLC_TAU_MS: f64 = 10.0;
/// Output audio remembered for concealment.
const PLC_MS: f64 = 5.0;
/// Averaging time for the buffer level the speed control steers.
const EMA_SECS: f64 = 0.5;
/// How fast a remembered low point is forgotten.
const MIN_FORGET_MS_PER_SEC: f64 = 2.0;

enum State {
    /// Waiting for enough audio to start.
    Prebuffer,
    Playing,
    /// Ran dry; `owed` source frames of time have passed without audio.
    Starving { owed: f64 },
}

/// Leftover statistics for the current measuring window.
struct Window {
    min: f64,
    sum: f64,
    n: u32,
    secs: f64,
    adjust0: f64,
}

impl Window {
    fn new(adjust: f64) -> Window {
        Window { min: f64::MAX, sum: 0.0, n: 0, secs: 0.0, adjust0: adjust }
    }
}

struct Skip {
    frames: f64,
    done: usize,
    len: usize,
}

/// Plays the last few milliseconds back and forth, fading out, to cover a gap.
struct Plc {
    ring: VecDeque<f32>,
    ch: usize,
    seg: Vec<f32>,
    i: isize,
    dir: isize,
    gain: f32,
}

impl Plc {
    fn new() -> Plc {
        Plc { ring: VecDeque::new(), ch: 0, seg: Vec::new(), i: 0, dir: -1, gain: 0.0 }
    }

    fn remember(&mut self, frame: &[f32], max_frames: usize) {
        if self.ch != frame.len() {
            self.ring.clear();
            self.ch = frame.len();
        }
        self.ring.extend(frame.iter().copied());
        while self.ring.len() > max_frames * self.ch {
            for _ in 0..self.ch {
                self.ring.pop_front();
            }
        }
    }

    fn start(&mut self) {
        self.seg.clear();
        self.seg.extend(self.ring.iter().copied());
        let frames = self.seg.len() / self.ch.max(1);
        self.i = frames as isize - 2;
        self.dir = -1;
        self.gain = if frames >= 4 { 1.0 } else { 0.0 };
    }

    fn active(&self) -> bool {
        self.gain > 0.001
    }

    /// Next concealment frame into `out` (length = channels); returns false once silent.
    fn next(&mut self, out: &mut [f32], decay: f32) -> bool {
        let frames = (self.seg.len() / self.ch.max(1)) as isize;
        if !self.active() || frames < 4 || out.len() != self.ch {
            out.fill(0.0);
            self.gain = 0.0;
            return false;
        }
        let base = self.i as usize * self.ch;
        for (c, o) in out.iter_mut().enumerate() {
            *o = self.seg[base + c] * self.gain;
        }
        self.i += self.dir;
        if self.i <= 0 || self.i >= frames - 1 {
            self.dir = -self.dir;
            self.i = self.i.clamp(0, frames - 1);
        }
        self.gain *= decay;
        true
    }
}

pub struct StreamBuffer {
    src_rate: u32,
    src_ch: usize,
    tune: Tuning,
    /// Interleaved source samples. Frame 0 is one frame of history for interpolation.
    buf: VecDeque<f32>,
    /// Fractional read position in frames, always >= 1.
    pos: f64,
    next_seq: Option<u32>,
    /// Packets that arrived ahead of a missing one, waiting for it (seq, samples).
    pending: Vec<(u32, Vec<f32>)>,
    last_packet: Vec<f32>,
    state: State,
    /// Wanted average leftover after each render, in source frames.
    target: f64,
    leftover_ema: f64,
    /// Lowest recent leftover, slowly forgetting; guards catch-up skips.
    recent_min: f64,
    last_need: f64,
    win: Window,
    /// Recent per-second dip depths (average minus lowest leftover), in frames.
    spreads: VecDeque<f64>,
    /// Frames our own corrections removed (+) or added (-), so measuring
    /// can tell them apart from jitter.
    adjust: f64,
    /// Seconds of audio rendered, and when the last window closed / we last ran dry.
    clock: f64,
    window_closed_at: f64,
    starved_at: f64,
    skip: Option<Skip>,
    plc: Plc,
    ramp: f32,
    frame_scratch: Vec<f32>,
    plc_scratch: Vec<f32>,
    pub last_seen: Instant,
    pub lost: u64,
    pub late: u64,
    pub underruns: u64,
    pub peak: f32,
    /// Capture delay reported by the sender, ms (0 = unknown).
    pub capture_ms: f64,
}

impl StreamBuffer {
    pub fn new(src_rate: u32, src_ch: usize, mode: Mode) -> Self {
        let tune = mode.tuning();
        let mut buf = VecDeque::with_capacity(src_rate as usize * src_ch / 2);
        buf.extend(std::iter::repeat(0.0).take(src_ch)); // history frame
        let mut b = StreamBuffer {
            src_rate,
            src_ch,
            tune,
            buf,
            pos: 1.0,
            next_seq: None,
            pending: Vec::new(),
            last_packet: Vec::new(),
            state: State::Prebuffer,
            target: 0.0,
            leftover_ema: 0.0,
            recent_min: 0.0,
            last_need: 0.0,
            win: Window::new(0.0),
            spreads: VecDeque::new(),
            adjust: 0.0,
            clock: 0.0,
            window_closed_at: f64::MIN,
            starved_at: 0.0,
            skip: None,
            plc: Plc::new(),
            ramp: 0.0,
            frame_scratch: Vec::new(),
            plc_scratch: Vec::new(),
            last_seen: Instant::now(),
            lost: 0,
            late: 0,
            underruns: 0,
            peak: 0.0,
            capture_ms: 0.0,
        };
        b.reset_history();
        b
    }

    fn ms_to_frames(&self, ms: f64) -> f64 {
        ms * self.src_rate as f64 / 1000.0
    }

    fn frames_to_ms(&self, frames: f64) -> f64 {
        frames * 1000.0 / self.src_rate as f64
    }

    fn reset_history(&mut self) {
        let start = self.ms_to_frames(self.tune.start_ms - self.tune.margin_ms).max(0.0);
        self.spreads.clear();
        self.spreads.extend(std::iter::repeat(start).take(self.tune.history));
        self.update_target();
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.tune = mode.tuning();
        self.reset_history();
    }

    pub fn format(&self) -> (u32, usize) {
        (self.src_rate, self.src_ch)
    }

    fn frames(&self) -> usize {
        self.buf.len() / self.src_ch
    }

    /// Frames buffered ahead of the read position.
    pub fn fill_frames(&self) -> f64 {
        (self.frames() as f64 - self.pos - 2.0).max(0.0)
    }

    /// Frames buffered including packets waiting behind a missing one (which
    /// will be concealed): the real amount of time the buffer covers.
    fn fill_with_pending(&self) -> f64 {
        let (Some(next), Some((last, p))) = (self.next_seq, self.pending.last()) else {
            return self.fill_frames();
        };
        let per = (p.len() / self.src_ch) as f64;
        self.fill_frames() + (last.wrapping_sub(next) as f64 + 1.0) * per
    }

    /// Average audio queued, ms.
    pub fn buffer_ms(&self) -> f64 {
        match self.state {
            State::Playing => self.frames_to_ms((self.leftover_ema + self.last_need).max(0.0)),
            _ => self.frames_to_ms(self.fill_frames()),
        }
    }

    /// Wanted average audio queued, ms.
    pub fn target_ms(&self) -> f64 {
        self.frames_to_ms(self.target + self.last_need)
    }

    fn update_target(&mut self) {
        let mut v: Vec<f64> = self.spreads.iter().copied().collect();
        v.sort_by(|a, b| b.total_cmp(a));
        let pick = v.get(self.tune.ignore_worst.min(v.len().saturating_sub(1))).copied().unwrap_or(0.0);
        let max = self.ms_to_frames(MAX_TARGET_MS);
        self.target = (pick + self.ms_to_frames(self.tune.margin_ms)).clamp(1.0, max);
    }

    fn record(&mut self, leftover: f64) {
        let v = leftover + (self.adjust - self.win.adjust0);
        self.win.min = self.win.min.min(v);
        self.win.sum += v;
        self.win.n += 1;
    }

    fn finish_window(&mut self) {
        if self.win.n >= 5 {
            let spread = (self.win.sum / self.win.n as f64 - self.win.min).max(0.0);
            self.spreads.push_back(spread);
            while self.spreads.len() > self.tune.history.max(1) {
                self.spreads.pop_front();
            }
            self.update_target();
            self.window_closed_at = self.clock;
        }
        self.win = Window::new(self.adjust);
    }

    /// The sender went quiet. The buffer draining just before we noticed
    /// was not jitter: forget a window that closed while it drained.
    fn forget_pause(&mut self) {
        if self.starved_at - self.window_closed_at < 0.5 && self.spreads.len() > 1 {
            self.spreads.pop_back();
            self.spreads.push_front(self.spreads.front().copied().unwrap_or(0.0));
            self.update_target();
        }
        self.window_closed_at = f64::MIN;
        self.win = Window::new(self.adjust);
    }

    /// The sender started over (or went quiet for long): play from `seq` after prebuffering.
    fn restart(&mut self, seq: u32) {
        if matches!(self.state, State::Starving { .. }) {
            self.forget_pause();
        }
        let keep = self.src_ch;
        self.buf.drain(..self.buf.len().saturating_sub(keep));
        self.pos = 1.0;
        // Keep packets from after the restart that overtook this one.
        self.pending.retain(|(s, _)| (1..1000).contains(&(s.wrapping_sub(seq) as i32)));
        self.next_seq = Some(seq);
        self.skip = None;
        if matches!(self.state, State::Playing) {
            self.plc.start();
        }
        self.state = State::Prebuffer;
        self.win = Window::new(self.adjust);
    }

    /// Add one packet of interleaved samples.
    pub fn push(&mut self, seq: u32, flags: u8, samples: &[f32]) {
        self.last_seen = Instant::now();
        if samples.is_empty() || samples.len() % self.src_ch != 0 {
            return;
        }
        if flags & FLAG_RESUME != 0 {
            match self.state {
                // Still playing what came before the pause: carry on, but the
                // dip the pause caused is not network jitter.
                State::Playing if self.next_seq == Some(seq) => self.win = Window::new(self.adjust),
                _ => self.restart(seq),
            }
        }
        let Some(next) = self.next_seq else {
            self.next_seq = Some(seq.wrapping_add(1));
            self.append(samples, None);
            return;
        };
        let diff = seq.wrapping_sub(next) as i32;
        if diff < 0 {
            self.late += 1;
            return;
        }
        if diff == 0 {
            self.append(samples, None);
            self.next_seq = Some(seq.wrapping_add(1));
            self.drain_pending();
        } else if diff > 1000 {
            // Far ahead: the sender restarted without saying so. Resync.
            self.lost += diff as u64;
            self.pending.clear();
            self.append(samples, None);
            self.next_seq = Some(seq.wrapping_add(1));
        } else if !self.pending.iter().any(|(s, _)| *s == seq) {
            // A packet is missing. Hold this one until the gap is due, in
            // case the missing one is only late.
            self.pending.push((seq, samples.to_vec()));
            self.pending.sort_by_key(|(s, _)| s.wrapping_sub(next));
            if self.pending.len() > 64 {
                self.fill_gaps();
            }
        }
        // Keep memory bounded if nothing is playing us.
        let cap = self.src_rate as f64 * 2.0;
        if self.fill_frames() > cap {
            let drop = (self.fill_frames() - self.target - self.last_need).max(0.0) as usize;
            self.discard(drop);
        }
    }

    /// Append pending packets that are now next in line.
    fn drain_pending(&mut self) {
        while let Some(next) = self.next_seq {
            let Some(i) = self.pending.iter().position(|(s, _)| *s == next) else { break };
            let (_, p) = self.pending.remove(i);
            self.append(&p, None);
            self.next_seq = Some(next.wrapping_add(1));
        }
    }

    /// Give up on missing packets: conceal them and play what came after.
    fn fill_gaps(&mut self) {
        while !self.pending.is_empty() {
            let Some(next) = self.next_seq else { return };
            let (seq, p) = self.pending.remove(0);
            let missing = seq.wrapping_sub(next);
            self.lost += missing as u64;
            let per = (p.len() / self.src_ch).max(1);
            let conceal_frames = missing as usize * per;
            let cont = if missing > 0 && self.frames_to_ms(conceal_frames as f64) <= MAX_CONCEAL_MS {
                Some(self.conceal(conceal_frames))
            } else {
                None
            };
            self.append(&p, cont.as_deref());
            self.next_seq = Some(seq.wrapping_add(1));
            self.drain_pending();
        }
    }

    /// Append `frames` of concealment made from the last packet, played
    /// back and forth and fading. Returns a short continuation to crossfade
    /// into the packet that follows.
    fn conceal(&mut self, frames: usize) -> Vec<f32> {
        let ch = self.src_ch;
        let xf = (self.ms_to_frames(1.0) as usize).max(1);
        let src = std::mem::take(&mut self.last_packet);
        let n = src.len() / ch;
        let mut cont = Vec::with_capacity(xf * ch);
        if n < 4 {
            self.buf.extend(std::iter::repeat(0.0).take(frames * ch));
            cont.resize(xf * ch, 0.0);
        } else {
            let decay = (0.7f64.powf(1.0 / n as f64)) as f32;
            let mut g = 1.0f32;
            let mut i = n as isize - 2;
            let mut dir = -1isize;
            for k in 0..frames + xf {
                for c in 0..ch {
                    let v = src[i as usize * ch + c] * g;
                    if k < frames {
                        self.buf.push_back(v);
                    } else {
                        cont.push(v);
                    }
                }
                i += dir;
                if i <= 0 || i >= n as isize - 1 {
                    dir = -dir;
                    i = i.clamp(0, n as isize - 1);
                }
                g *= decay;
            }
        }
        self.last_packet = src;
        cont
    }

    fn append(&mut self, samples: &[f32], xfade_from: Option<&[f32]>) {
        match xfade_from {
            Some(cont) if !cont.is_empty() => {
                let ch = self.src_ch;
                let xf = (cont.len() / ch).min(samples.len() / ch);
                for f in 0..samples.len() / ch {
                    for c in 0..ch {
                        let mut v = samples[f * ch + c];
                        if f < xf {
                            let w = (f + 1) as f32 / (xf + 1) as f32;
                            v = v * w + cont[f * ch + c] * (1.0 - w);
                        }
                        self.buf.push_back(v);
                    }
                }
            }
            _ => self.buf.extend(samples.iter().copied()),
        }
        self.last_packet.clear();
        self.last_packet.extend_from_slice(samples);
    }

    /// Drop `n` frames right after the read position.
    fn discard(&mut self, n: usize) {
        let n = n.min(self.fill_frames() as usize);
        if n == 0 {
            return;
        }
        let start = (self.pos.floor() as usize + 1) * self.src_ch;
        let end = (start + n * self.src_ch).min(self.buf.len());
        self.buf.drain(start..end);
        self.adjust += n as f64;
    }

    #[inline]
    fn sample(&self, frame: usize, ch: usize) -> f32 {
        let ch = ch.min(self.src_ch - 1);
        self.buf.get(frame * self.src_ch + ch).copied().unwrap_or(0.0)
    }

    /// Source sample for output channel `c` at fractional frame position `p`.
    #[inline]
    fn value(&self, p: f64, c: usize, out_ch: usize) -> f32 {
        let i = p.floor() as usize;
        let t = (p - i as f64) as f32;
        match (self.src_ch, c) {
            (1, 0) | (1, 1) => self.cubic(i, 0, t),
            (2, 0) | (2, 1) if out_ch >= 2 => self.cubic(i, c, t),
            (2, 0) => 0.5 * (self.cubic(i, 0, t) + self.cubic(i, 1, t)),
            _ => 0.0,
        }
    }

    /// Mix this stream into `out` (interleaved, `out_ch` channels at `out_rate`).
    pub fn render_add(&mut self, out: &mut [f32], out_rate: u32, out_ch: usize, gain: f32) {
        let frames_out = out.len() / out_ch.max(1);
        if frames_out == 0 {
            return;
        }
        let secs = frames_out as f64 / out_rate as f64;
        self.clock += secs;
        let base_step = self.src_rate as f64 / out_rate as f64;
        let nominal = frames_out as f64 * base_step;
        let need = frames_out as f64 * base_step * (1.0 + self.tune.max_speed) + 3.0;
        self.last_need = nominal;
        let plc_decay = (-1.0 / (PLC_TAU_MS / 1000.0 * out_rate as f64)).exp() as f32;
        let plc_frames = (PLC_MS / 1000.0 * out_rate as f64) as usize;

        // A missing packet is only given up on when it is actually due.
        let prebuffering = matches!(self.state, State::Prebuffer);
        if !self.pending.is_empty() && !prebuffering && self.fill_frames() < need {
            self.fill_gaps();
        }
        let mut avail = self.fill_frames();

        match self.state {
            State::Prebuffer => {
                let waiting: usize = self.pending.iter().map(|(_, p)| p.len() / self.src_ch).sum();
                if avail + waiting as f64 >= need + self.target && !self.pending.is_empty() {
                    self.fill_gaps();
                    avail = self.fill_frames();
                }
                if avail >= need + self.target {
                    self.state = State::Playing;
                    self.ramp = 0.0;
                    self.leftover_ema = avail - need;
                    self.recent_min = avail - need;
                    self.win = Window::new(self.adjust);
                } else {
                    self.conceal_out(out, out_ch, gain, plc_decay);
                    return;
                }
            }
            State::Starving { owed } => {
                let resume_at = need + self.target * 0.5;
                if self.tune.drop_late && avail > resume_at {
                    // Audio that should already have played: drop it to stay in sync.
                    let d = owed.min(avail - resume_at).max(0.0);
                    self.discard(d as usize);
                    let per = (self.last_packet.len() / self.src_ch).max(1) as f64;
                    self.late += (d / per).round() as u64;
                    avail = self.fill_frames();
                }
                if avail >= resume_at {
                    self.underruns += 1;
                    self.state = State::Playing;
                    self.ramp = 0.0;
                    // Make room for a dip like this one right away.
                    if self.win.n > 0 {
                        let dip = self.win.sum / self.win.n as f64 - self.win.min;
                        let max = self.ms_to_frames(MAX_TARGET_MS);
                        self.target = self.target.max(dip + self.ms_to_frames(self.tune.margin_ms)).min(max);
                    }
                } else {
                    let owed = owed + nominal;
                    self.adjust -= nominal;
                    self.record(avail - need);
                    self.recent_min = self.recent_min.min(avail - need - owed);
                    self.win.secs += secs;
                    if owed > self.src_rate as f64 * PAUSE_SECS {
                        // Not a hiccup: the sender stopped. Start over when it returns.
                        self.forget_pause();
                        self.state = State::Prebuffer;
                    } else {
                        self.state = State::Starving { owed };
                    }
                    self.conceal_out(out, out_ch, gain, plc_decay);
                    return;
                }
            }
            State::Playing => {}
        }

        // Measure against everything received, not only what is contiguous:
        // a lost packet must not look like the buffer running low.
        let leftover = self.fill_with_pending() - need;
        self.record(leftover);
        let alpha = 1.0 - (-secs / EMA_SECS).exp();
        self.leftover_ema += (leftover - self.leftover_ema) * alpha;
        self.recent_min = leftover.min(self.recent_min + self.ms_to_frames(MIN_FORGET_MS_PER_SEC) * secs);

        // Back toward target: cut out a big excess, drift the speed for small ones.
        let err = self.leftover_ema - self.target;
        let xf_out = ((XFADE_MS / 1000.0 * out_rate as f64) as usize).max(8);
        if self.skip.is_none() && err > self.ms_to_frames(self.tune.skip_over_ms) {
            let n = err.min(self.ms_to_frames(20.0)).floor();
            let spare = self.recent_min - n - self.ms_to_frames(self.tune.margin_ms);
            if spare > 0.0 && leftover > n + xf_out as f64 * base_step * 1.1 + 4.0 {
                self.skip = Some(Skip { frames: n, done: 0, len: xf_out });
                self.leftover_ema -= n;
                self.recent_min -= n;
            }
        }
        let adj = (err / (self.src_rate as f64 * 2.0)).clamp(-self.tune.max_speed, self.tune.max_speed);
        let step = base_step * (1.0 + adj);
        let ramp_step = (1000.0 / (FADE_MS * out_rate as f64)) as f32;

        let mut frame = std::mem::take(&mut self.frame_scratch);
        frame.resize(out_ch, 0.0);
        let mut plc_frame = std::mem::take(&mut self.plc_scratch);
        plc_frame.resize(out_ch, 0.0);
        let mut peak = self.peak * 0.9;
        let mut starved_at = None;
        let start_pos = self.pos;
        for f in 0..frames_out {
            let skip_off = self.skip.as_ref().map(|s| s.frames).unwrap_or(0.0);
            if self.pos.floor() as usize + skip_off as usize + 3 > self.frames() {
                starved_at = Some(f);
                break;
            }
            for (c, v) in frame.iter_mut().enumerate() {
                *v = self.value(self.pos, c, out_ch);
            }
            if let Some((skip_frames, done, len)) = self.skip.as_ref().map(|s| (s.frames, s.done, s.len)) {
                // Crossfade from here to `skip_frames` ahead, then jump there.
                let w = (done + 1) as f32 / (len + 1) as f32;
                for (c, v) in frame.iter_mut().enumerate() {
                    *v = *v * (1.0 - w) + self.value(self.pos + skip_frames, c, out_ch) * w;
                }
                if done + 1 >= len {
                    self.pos += skip_frames;
                    self.skip = None;
                } else if let Some(s) = &mut self.skip {
                    s.done += 1;
                }
            }
            if self.ramp < 1.0 {
                self.ramp = (self.ramp + ramp_step).min(1.0);
                if self.plc.active() {
                    self.plc.next(&mut plc_frame, plc_decay);
                    for (v, p) in frame.iter_mut().zip(&plc_frame) {
                        *v = *v * self.ramp + *p * (1.0 - self.ramp);
                    }
                } else {
                    for v in frame.iter_mut() {
                        *v *= self.ramp;
                    }
                }
            }
            self.plc.remember(&frame, plc_frames);
            for (c, v) in frame.iter().enumerate() {
                let o = v * gain;
                peak = peak.max(o.abs());
                out[f * out_ch + c] += o;
            }
            self.pos += step;
        }
        self.frame_scratch = frame;
        self.plc_scratch = plc_frame;
        self.peak = peak;
        // Consumed more than real time (speed-up, skip) or less (stall).
        self.adjust += (self.pos - start_pos) - nominal;

        if let Some(f) = starved_at {
            // Ran dry mid-block: cover the rest and wait for more.
            self.skip = None;
            self.state = State::Starving { owed: 0.0 };
            self.starved_at = self.clock;
            self.plc.start();
            let rest = &mut out[f * out_ch..];
            self.conceal_out(rest, out_ch, gain, plc_decay);
        }

        // Discard consumed frames, keeping one frame of history.
        let consumed = (self.pos.floor() as usize).saturating_sub(1).min(self.frames());
        if consumed > 0 {
            self.buf.drain(..consumed * self.src_ch);
            self.pos -= consumed as f64;
        }

        self.win.secs += secs;
        if self.win.secs >= WINDOW_SECS && matches!(self.state, State::Playing) {
            self.finish_window();
        }
    }

    /// Fill `out` with fading concealment (or nothing once it has faded).
    fn conceal_out(&mut self, out: &mut [f32], out_ch: usize, gain: f32, decay: f32) {
        if !self.plc.active() {
            return;
        }
        let mut frame = std::mem::take(&mut self.plc_scratch);
        frame.resize(out_ch, 0.0);
        for chunk in out.chunks_exact_mut(out_ch) {
            if !self.plc.next(&mut frame, decay) {
                break;
            }
            for (o, v) in chunk.iter_mut().zip(&frame) {
                *o += v * gain;
            }
        }
        self.plc_scratch = frame;
    }

    /// Catmull-Rom interpolation between frames i and i+1.
    #[inline]
    fn cubic(&self, i: usize, ch: usize, t: f32) -> f32 {
        let y0 = self.sample(i.saturating_sub(1), ch);
        let y1 = self.sample(i, ch);
        let y2 = self.sample(i + 1, ch);
        let y3 = self.sample(i + 2, ch);
        let a = -0.5 * y0 + 1.5 * y1 - 1.5 * y2 + 0.5 * y3;
        let b = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
        let c = -0.5 * y0 + 0.5 * y2;
        ((a * t + b) * t + c) * t + y1
    }
}

pub struct StreamEntry {
    pub buf: StreamBuffer,
    pub name: String,
    pub from: String,
}

#[derive(Clone, Debug, Default)]
pub struct StreamStats {
    pub id: u32,
    pub name: String,
    pub from: String,
    pub sample_rate: u32,
    pub channels: usize,
    /// Average audio queued, ms.
    pub buffer_ms: f64,
    pub target_ms: f64,
    pub lost: u64,
    pub late: u64,
    pub underruns: u64,
    pub level: f32,
    /// Capture delay reported by the sender, ms (0 = unknown).
    pub capture_ms: f64,
}

/// Sums every incoming stream. Shared between the network thread (push)
/// and the audio callback (render), guarded by a short mutex.
pub struct Mixer {
    pub streams: HashMap<u32, StreamEntry>,
    pub mode: Mode,
    pub volume: f32,
    pub muted: bool,
    /// What is being played, for the visualizers.
    pub scope: crate::scope::Scope,
}

impl Mixer {
    pub fn new(mode: Mode) -> Self {
        Mixer { streams: HashMap::new(), mode, volume: 1.0, muted: false, scope: Default::default() }
    }

    pub fn render(&mut self, out: &mut [f32], out_rate: u32, out_ch: usize) {
        out.fill(0.0);
        if out_ch == 0 {
            return;
        }
        let gain = if self.muted { 0.0 } else { self.volume };
        for s in self.streams.values_mut() {
            s.buf.render_add(out, out_rate, out_ch, gain);
        }
        for v in out.iter_mut() {
            *v = soft_clip(*v);
        }
        self.scope.push(out, out_rate, out_ch);
    }

    pub fn set_mode(&mut self, mode: Mode) {
        if mode == self.mode {
            return;
        }
        self.mode = mode;
        for s in self.streams.values_mut() {
            s.buf.set_mode(mode);
        }
    }

    /// Drop streams that stopped sending. Returns removed entries so the
    /// caller can free them outside the lock.
    pub fn take_stale(&mut self, timeout: Duration) -> Vec<StreamEntry> {
        let now = Instant::now();
        let stale: Vec<u32> = self
            .streams
            .iter()
            .filter(|(_, s)| now.duration_since(s.buf.last_seen) > timeout)
            .map(|(k, _)| *k)
            .collect();
        stale.into_iter().filter_map(|k| self.streams.remove(&k)).collect()
    }

    pub fn stats(&self) -> Vec<StreamStats> {
        let mut v: Vec<StreamStats> = self
            .streams
            .iter()
            .map(|(id, s)| {
                let (rate, ch) = s.buf.format();
                StreamStats {
                    id: *id,
                    name: s.name.clone(),
                    from: s.from.clone(),
                    sample_rate: rate,
                    channels: ch,
                    buffer_ms: s.buf.buffer_ms(),
                    target_ms: s.buf.target_ms(),
                    lost: s.buf.lost,
                    late: s.buf.late,
                    underruns: s.buf.underruns,
                    level: s.buf.peak,
                    capture_ms: s.buf.capture_ms,
                }
            })
            .collect();
        v.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
        v
    }
}

#[inline]
fn soft_clip(x: f32) -> f32 {
    let a = x.abs();
    if a <= 0.9 {
        x
    } else {
        x.signum() * (0.9 + 0.1 * ((a - 0.9) / 0.1).tanh())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;
    /// Channel 1 carries a slow ramp so the delay of every sample can be read back.
    const RAMP: f64 = 19_200.0;

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> f64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }
        fn exp(&mut self, mean: f64) -> f64 {
            -mean * (1.0 - self.next()).ln()
        }
    }

    #[derive(Clone, Copy)]
    struct Net {
        base_ms: f64,
        jitter_ms: f64,
        loss: f64,
        /// Average seconds between stalls, and how long each stall holds packets.
        spike_every: f64,
        spike_ms: f64,
        /// Wi-Fi power save: packets only come out at beacon times.
        beacon_ms: f64,
    }

    #[derive(Clone, Copy)]
    struct Sim {
        mode: Mode,
        chunk_ms: f64,
        render_frames: usize,
        secs: f64,
        net: Net,
        /// Sender goes quiet for `.1` seconds every `.0` seconds.
        pause: Option<(f64, f64)>,
        flag_resume: bool,
    }

    // Every field shows up in assertion messages through Debug.
    #[allow(dead_code)]
    #[derive(Debug)]
    struct Result {
        underruns: u64,
        lost: u64,
        late: u64,
        glitch_ms: f64,
        delay_ms: f64,
        target_ms: f64,
    }

    fn run(sim: Sim) -> Result {
        let mut rng = Rng(0x9E3779B97F4A7C15);
        let pkt = (RATE as f64 * sim.mode.packet_ms() / 1000.0).round() as usize;
        let chunk = (RATE as f64 * sim.chunk_ms / 1000.0) as usize;
        let total = (sim.secs * RATE as f64) as usize;
        // Stalls: (start, end) in seconds.
        let mut spikes = Vec::new();
        if sim.net.spike_every > 0.0 {
            let mut t = rng.exp(sim.net.spike_every);
            while t < sim.secs {
                spikes.push((t, t + sim.net.spike_ms / 1000.0));
                t += rng.exp(sim.net.spike_every);
            }
        }
        let paused = |t: f64| sim.pause.is_some_and(|(every, len)| t % every > every - len);

        // Every packet: (arrival s, seq, flags, samples).
        let mut packets = Vec::new();
        let mut seq = 0u32;
        let mut pending: Vec<f32> = Vec::new();
        let mut was_paused = false;
        let mut idx = 0usize;
        while idx < total {
            let sent = (idx + chunk) as f64 / RATE as f64;
            if paused(sent) {
                was_paused = true;
                pending.clear();
                idx += chunk;
                continue;
            }
            for f in idx..idx + chunk {
                pending.push(0.5);
                pending.push(((f as f64 % RAMP) / RAMP) as f32);
            }
            idx += chunk;
            while pending.len() >= pkt * 2 {
                let samples: Vec<f32> = pending.drain(..pkt * 2).collect();
                let flags = if was_paused && sim.flag_resume { FLAG_RESUME } else { 0 };
                was_paused = false;
                let mut arrive = sent + (sim.net.base_ms + rng.exp(sim.net.jitter_ms)) / 1000.0;
                for (a, b) in &spikes {
                    if sent >= *a && sent < *b {
                        arrive = arrive.max(*b + rng.next() * 0.001);
                    }
                }
                if sim.net.beacon_ms > 0.0 {
                    let b = sim.net.beacon_ms / 1000.0;
                    arrive = (arrive / b).ceil() * b;
                }
                if rng.next() >= sim.net.loss {
                    packets.push((arrive, seq, flags, samples));
                }
                seq = seq.wrapping_add(1);
            }
        }
        packets.sort_by(|a, b| a.0.total_cmp(&b.0));

        let mut b = StreamBuffer::new(RATE, 2, sim.mode);
        let mut out = vec![0.0f32; sim.render_frames * 2];
        let mut next = 0;
        let mut t_frames = 0usize;
        let warm = 5.0;
        let (mut glitch, mut delay_sum, mut delay_n) = (0usize, 0.0, 0usize);
        while t_frames < total {
            let now = t_frames as f64 / RATE as f64;
            while next < packets.len() && packets[next].0 <= now {
                let (_, s, fl, ref p) = packets[next];
                b.push(s, fl, p);
                next += 1;
            }
            out.fill(0.0);
            b.render_add(&mut out, RATE, 2, 1.0);
            for (i, fr) in out.chunks_exact(2).enumerate() {
                let t = (t_frames + i) as f64 / RATE as f64;
                // Only judge time when the sender was producing sound a moment ago.
                if t < warm || paused(t) || paused(t - 0.3) {
                    continue;
                }
                if fr[0] < 0.45 {
                    glitch += 1;
                } else if fr[0] > 0.49 && fr[1] > 0.02 && fr[1] < 0.98 {
                    let played = (t_frames + i) as f64 % RAMP;
                    let d = (played - fr[1] as f64 * RAMP).rem_euclid(RAMP);
                    delay_sum += d;
                    delay_n += 1;
                }
            }
            t_frames += sim.render_frames;
        }
        Result {
            underruns: b.underruns,
            lost: b.lost,
            late: b.late,
            glitch_ms: glitch as f64 * 1000.0 / RATE as f64,
            delay_ms: if delay_n > 0 { delay_sum / delay_n as f64 * 1000.0 / RATE as f64 } else { f64::NAN },
            target_ms: b.target_ms(),
        }
    }

    const LAN: Net = Net { base_ms: 0.3, jitter_ms: 0.2, loss: 0.0, spike_every: 0.0, spike_ms: 0.0, beacon_ms: 0.0 };
    const WIFI: Net = Net { base_ms: 2.0, jitter_ms: 1.5, loss: 0.003, spike_every: 4.0, spike_ms: 25.0, beacon_ms: 0.0 };
    const POWER_SAVE: Net = Net { base_ms: 2.0, jitter_ms: 1.0, loss: 0.001, spike_every: 0.0, spike_ms: 0.0, beacon_ms: 102.4 };

    fn sim(mode: Mode, net: Net) -> Sim {
        // A Windows-like sender (10 ms capture blocks) and receiver (10 ms callbacks).
        Sim { mode, chunk_ms: 10.0, render_frames: 480, secs: 60.0, net, pause: None, flag_resume: true }
    }

    #[test]
    fn wired_lan_settles_low_and_clean() {
        for mode in Mode::ALL {
            let r = run(sim(mode, LAN));
            println!("LAN {mode:?}: {r:?}");
            assert_eq!(r.glitch_ms, 0.0, "{mode:?}");
            assert!(r.underruns <= 1, "{mode:?}");
        }
        let game = run(sim(Mode::Game, LAN));
        assert!(game.delay_ms < 30.0, "game delay {}", game.delay_ms);
    }

    #[test]
    fn wifi_hiccups_are_absorbed() {
        for mode in Mode::ALL {
            let r = run(sim(mode, WIFI));
            println!("Wi-Fi {mode:?}: {r:?}");
        }
        let bal = run(sim(Mode::Balanced, WIFI));
        assert!(bal.underruns <= 2, "{bal:?}");
        assert!(bal.delay_ms < 60.0, "{bal:?}");
        let game = run(sim(Mode::Game, WIFI));
        assert!(game.underruns <= 15, "{game:?}");
        assert!(game.delay_ms < bal.delay_ms, "{game:?}");
        let music = run(sim(Mode::Music, WIFI));
        assert_eq!(music.underruns, 0, "{music:?}");
    }

    #[test]
    fn power_save_wifi_adapts() {
        for mode in Mode::ALL {
            let r = run(sim(mode, POWER_SAVE));
            println!("power save {mode:?}: {r:?}");
        }
        let bal = run(sim(Mode::Balanced, POWER_SAVE));
        assert!(bal.underruns <= 3, "{bal:?}");
        assert!(bal.glitch_ms < 100.0, "{bal:?}");
    }

    #[test]
    fn sender_pauses_are_not_glitches() {
        for mode in Mode::ALL {
            let s = Sim { pause: Some((5.0, 2.0)), ..sim(mode, LAN) };
            let r = run(s);
            println!("pauses {mode:?}: {r:?}");
            assert_eq!(r.underruns, 0, "{mode:?}");
            assert_eq!(r.glitch_ms, 0.0, "{mode:?}");
            // An old sender without the flag: long pauses still aren't counted.
            let r = run(Sim { flag_resume: false, ..s });
            assert_eq!(r.underruns, 0, "no flag {mode:?}");
        }
    }

    #[test]
    fn small_packets_small_blocks() {
        let r = run(Sim { chunk_ms: 2.5, render_frames: 240, ..sim(Mode::Game, LAN) });
        println!("small {r:?}");
        assert!(r.delay_ms < 20.0, "{r:?}");
    }

    #[test]
    fn phone_to_pc_small_blocks() {
        // Android-like sender (5 ms reads) to a 256-frame sound card.
        for mode in Mode::ALL {
            let r = run(Sim { chunk_ms: 5.0, render_frames: 256, ..sim(mode, WIFI) });
            println!("phone->pc {mode:?}: {r:?}");
        }
    }

    #[test]
    fn loss_is_concealed_and_reorder_kept() {
        let mut b = StreamBuffer::new(RATE, 2, Mode::Balanced);
        let p: Vec<f32> = (0..240).flat_map(|_| [0.5f32, 0.5]).collect();
        b.push(0, 0, &p);
        b.push(2, 0, &p); // 1 is late, not lost yet
        b.push(1, 0, &p);
        assert_eq!((b.lost, b.late), (0, 0), "reordered packet used");
        b.push(4, 0, &p); // 3 never comes
        b.push(5, 0, &p);
        b.fill_gaps();
        assert_eq!(b.lost, 1);
        b.push(3, 0, &p);
        assert_eq!(b.late, 1);
    }

    #[test]
    fn resamples_44k_to_48k() {
        let mut b = StreamBuffer::new(44_100, 2, Mode::Game);
        for s in 0..40 {
            let p: Vec<f32> = (0..220)
                .flat_map(|i| {
                    let v = (2.0 * std::f32::consts::PI * 1000.0 * (s * 220 + i) as f32 / 44_100.0).sin() * 0.5;
                    [v, v]
                })
                .collect();
            b.push(s as u32, 0, &p);
        }
        let mut out = vec![0.0f32; 480 * 2];
        b.render_add(&mut out, 48_000, 2, 1.0);
        b.render_add(&mut out, 48_000, 2, 1.0);
        assert_eq!(b.underruns, 0);
        assert!(out.iter().any(|v| v.abs() > 0.3));
    }
}
