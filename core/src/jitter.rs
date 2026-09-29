//! Per-source jitter buffer with clock-drift correction and resampling,
//! plus the mixer that sums every incoming source into the output device.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

/// Packets further ahead than this are treated as a resync, not a loss.
const MAX_CONCEAL_PACKETS: u32 = 8;
const MAX_TARGET_MS: f64 = 250.0;
const UNDERRUN_STEP_MS: f64 = 5.0;
/// After this long without an underrun the target shrinks back toward the base.
const SHRINK_AFTER: Duration = Duration::from_secs(20);
const FADE_MS: f64 = 5.0;

pub struct StreamBuffer {
    src_rate: u32,
    src_ch: usize,
    /// Interleaved source samples. Frame 0 is one frame of history for interpolation.
    buf: VecDeque<f32>,
    /// Fractional read position in frames, always >= 1.
    pos: f64,
    next_seq: Option<u32>,
    last_packet: Vec<f32>,
    playing: bool,
    base_target_ms: f64,
    target_ms: f64,
    fill_ema: f64,
    ramp: f32,
    last_underrun: Instant,
    last_shrink: Instant,
    pub last_seen: Instant,
    pub lost: u64,
    pub late: u64,
    pub underruns: u64,
    pub peak: f32,
}

impl StreamBuffer {
    pub fn new(src_rate: u32, src_ch: usize, base_target_ms: f64) -> Self {
        let now = Instant::now();
        let mut buf = VecDeque::with_capacity(src_rate as usize * src_ch);
        buf.extend(std::iter::repeat(0.0).take(src_ch)); // history frame
        StreamBuffer {
            src_rate,
            src_ch,
            buf,
            pos: 1.0,
            next_seq: None,
            last_packet: Vec::new(),
            playing: false,
            base_target_ms,
            target_ms: base_target_ms,
            fill_ema: 0.0,
            ramp: 0.0,
            last_underrun: now,
            last_shrink: now,
            last_seen: now,
            lost: 0,
            late: 0,
            underruns: 0,
            peak: 0.0,
        }
    }

    pub fn format(&self) -> (u32, usize) {
        (self.src_rate, self.src_ch)
    }

    pub fn set_base_target(&mut self, ms: f64) {
        self.base_target_ms = ms;
        self.target_ms = ms;
    }

    fn frames(&self) -> usize {
        self.buf.len() / self.src_ch
    }

    /// Frames buffered ahead of the read position.
    pub fn fill_frames(&self) -> f64 {
        (self.frames() as f64 - self.pos - 2.0).max(0.0)
    }

    pub fn buffer_ms(&self) -> f64 {
        self.fill_frames() * 1000.0 / self.src_rate as f64
    }

    pub fn target_ms(&self) -> f64 {
        self.target_ms
    }

    fn target_frames(&self) -> f64 {
        self.target_ms * self.src_rate as f64 / 1000.0
    }

    /// Add one packet of interleaved samples.
    pub fn push(&mut self, seq: u32, samples: &[f32]) {
        self.last_seen = Instant::now();
        if let Some(next) = self.next_seq {
            let diff = seq.wrapping_sub(next) as i32;
            if diff < 0 {
                self.late += 1;
                return;
            }
            if diff > 0 {
                self.lost += diff as u64;
                if diff as u32 <= MAX_CONCEAL_PACKETS && self.playing {
                    // Conceal the gap by repeating the previous packet.
                    for _ in 0..diff {
                        let prev = std::mem::take(&mut self.last_packet);
                        self.buf.extend(prev.iter().copied());
                        self.last_packet = prev;
                    }
                }
            }
        }
        self.next_seq = Some(seq.wrapping_add(1));
        self.buf.extend(samples.iter().copied());
        self.last_packet.clear();
        self.last_packet.extend_from_slice(samples);

        // Never let latency pile up: if far over target, jump forward.
        let max_fill = (self.target_frames() * 3.0).max(self.src_rate as f64 * 0.1);
        if self.fill_frames() > max_fill {
            let drop = (self.fill_frames() - self.target_frames()) as usize;
            self.skip_frames(drop);
        }
    }

    fn skip_frames(&mut self, n: usize) {
        let n = n.min(self.frames().saturating_sub(4));
        self.buf.drain(..n * self.src_ch);
        self.fill_ema = self.fill_frames();
    }

    #[inline]
    fn sample(&self, frame: usize, ch: usize) -> f32 {
        let ch = ch.min(self.src_ch - 1);
        self.buf.get(frame * self.src_ch + ch).copied().unwrap_or(0.0)
    }

    /// Mix this stream into `out` (interleaved, `out_ch` channels at `out_rate`).
    pub fn render_add(&mut self, out: &mut [f32], out_rate: u32, out_ch: usize, gain: f32) {
        let now = Instant::now();
        let fill = self.fill_frames();
        if !self.playing {
            if fill >= self.target_frames() {
                self.playing = true;
                self.ramp = 0.0;
                self.fill_ema = fill;
            } else {
                return;
            }
        }

        if now.duration_since(self.last_underrun) > SHRINK_AFTER
            && now.duration_since(self.last_shrink) > SHRINK_AFTER
            && self.target_ms > self.base_target_ms
        {
            self.target_ms = (self.target_ms - 2.0).max(self.base_target_ms);
            self.last_shrink = now;
        }

        // Drift control: nudge playback speed so the buffer hovers at target.
        self.fill_ema += (fill - self.fill_ema) * 0.02;
        let target = self.target_frames().max(1.0);
        let err = (self.fill_ema - target) / target;
        let adj = (err * 0.004).clamp(-0.006, 0.006);
        let step = self.src_rate as f64 / out_rate as f64 * (1.0 + adj);
        let ramp_step = (1000.0 / (FADE_MS * out_rate as f64)) as f32;

        let frames_out = out.len() / out_ch;
        let mut peak = self.peak * 0.9;
        for f in 0..frames_out {
            let i = self.pos.floor() as usize;
            if i + 2 >= self.frames() {
                // Ran dry: stop, rebuffer with a larger target.
                self.playing = false;
                self.underruns += 1;
                self.last_underrun = now;
                self.target_ms = (self.target_ms + UNDERRUN_STEP_MS).min(MAX_TARGET_MS);
                let _ = f;
                break;
            }
            let t = (self.pos - i as f64) as f32;
            if self.ramp < 1.0 {
                self.ramp = (self.ramp + ramp_step).min(1.0);
            }
            let g = gain * self.ramp;
            for c in 0..out_ch {
                let v = match (self.src_ch, c) {
                    (1, 0) | (1, 1) => self.cubic(i, 0, t),
                    (2, 0) | (2, 1) if out_ch >= 2 => self.cubic(i, c, t),
                    (2, 0) => 0.5 * (self.cubic(i, 0, t) + self.cubic(i, 1, t)),
                    _ => 0.0,
                } * g;
                peak = peak.max(v.abs());
                out[f * out_ch + c] += v;
            }
            self.pos += step;
        }
        self.peak = peak;

        // Discard consumed frames, keeping one frame of history.
        let consumed = (self.pos.floor() as usize).saturating_sub(1).min(self.frames());
        if consumed > 0 {
            self.buf.drain(..consumed * self.src_ch);
            self.pos -= consumed as f64;
        }
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
    pub buffer_ms: f64,
    pub target_ms: f64,
    pub lost: u64,
    pub late: u64,
    pub underruns: u64,
    pub level: f32,
}

/// Sums every incoming stream. Shared between the network thread (push)
/// and the audio callback (render), guarded by a short mutex.
pub struct Mixer {
    pub streams: HashMap<u32, StreamEntry>,
    pub base_target_ms: f64,
    pub volume: f32,
    pub muted: bool,
}

impl Mixer {
    pub fn new(base_target_ms: f64) -> Self {
        Mixer { streams: HashMap::new(), base_target_ms, volume: 1.0, muted: false }
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
    }

    pub fn set_base_target(&mut self, ms: f64) {
        self.base_target_ms = ms;
        for s in self.streams.values_mut() {
            s.buf.set_base_target(ms);
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

    fn sine_packet(start: usize, frames: usize, rate: f32) -> Vec<f32> {
        (0..frames)
            .flat_map(|i| {
                let v = (2.0 * std::f32::consts::PI * 1000.0 * (start + i) as f32 / rate).sin() * 0.5;
                [v, v]
            })
            .collect()
    }

    #[test]
    fn steady_stream_plays_without_underrun() {
        let mut b = StreamBuffer::new(48_000, 2, 20.0);
        let mut out = vec![0.0f32; 256 * 2];
        let mut seq = 0;
        let mut produced = 0;
        let mut consumed = 0;
        // Simulate 2 s: sender 240-frame packets, device 256-frame callbacks.
        for _ in 0..2000 {
            // one millisecond per iteration
            while produced < consumed + 48 * 25 {
                b.push(seq, &sine_packet(produced, 240, 48_000.0));
                seq += 1;
                produced += 240;
            }
            consumed += 48;
            if consumed % 256 < 48 {
                out.fill(0.0);
                b.render_add(&mut out, 48_000, 2, 1.0);
            }
        }
        assert_eq!(b.underruns, 0);
        assert!(b.peak > 0.3, "audio came out");
    }

    #[test]
    fn loss_is_counted_and_late_dropped() {
        let mut b = StreamBuffer::new(48_000, 2, 20.0);
        let p = sine_packet(0, 240, 48_000.0);
        b.push(0, &p);
        b.push(3, &p);
        b.push(2, &p);
        assert_eq!(b.lost, 2);
        assert_eq!(b.late, 1);
    }

    #[test]
    fn resamples_44k_to_48k() {
        let mut b = StreamBuffer::new(44_100, 2, 10.0);
        for s in 0..40 {
            b.push(s, &sine_packet(s as usize * 220, 220, 44_100.0));
        }
        let mut out = vec![0.0f32; 480 * 2];
        b.render_add(&mut out, 48_000, 2, 1.0);
        assert_eq!(b.underruns, 0);
        assert!(out.iter().any(|v| v.abs() > 0.3));
    }
}
