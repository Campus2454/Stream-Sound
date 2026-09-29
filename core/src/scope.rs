//! Recent audio kept for the apps' visualizers: a peak envelope for the
//! scrolling waveform and the last couple of thousand samples for the
//! frequency bars. Audio threads only append; the heavy lifting (FFT) runs on
//! whichever thread asks for a picture, on a copy.

use std::time::Instant;

/// Which side of the engine to look at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tap {
    /// Audio this device is capturing and sending.
    Send,
    /// Audio this device is playing from other devices (after volume).
    Receive,
}

/// Envelope resolution and length: 400 columns of 5 ms = the last 2 seconds.
const COL_SECS: f64 = 0.005;
const COLS: usize = 400;
/// Samples kept for the spectrum (~43 ms at 48 kHz).
const RAW: usize = 2048;
/// Show silence once nothing has arrived for this long.
const IDLE_SECS: f64 = 0.15;

pub struct Scope {
    env: Vec<f32>,
    env_pos: usize,
    acc_peak: f32,
    acc_frames: usize,
    raw: Vec<f32>,
    raw_pos: usize,
    rate: u32,
    last_push: Option<Instant>,
}

impl Default for Scope {
    fn default() -> Self {
        Scope {
            env: vec![0.0; COLS],
            env_pos: 0,
            acc_peak: 0.0,
            acc_frames: 0,
            raw: vec![0.0; RAW],
            raw_pos: 0,
            rate: 48_000,
            last_push: None,
        }
    }
}

impl Scope {
    /// Append interleaved audio. Cheap enough for an audio callback.
    pub fn push(&mut self, data: &[f32], rate: u32, ch: usize) {
        if ch == 0 || rate == 0 || data.len() < ch {
            return;
        }
        self.rate = rate;
        self.last_push = Some(Instant::now());
        let per_col = ((rate as f64 * COL_SECS) as usize).max(1);
        let inv = 1.0 / ch as f32;
        for frame in data.chunks_exact(ch) {
            let mono = frame.iter().sum::<f32>() * inv;
            let peak = frame.iter().fold(0.0f32, |m, v| m.max(v.abs()));
            self.raw[self.raw_pos] = mono;
            self.raw_pos = (self.raw_pos + 1) % RAW;
            self.acc_peak = self.acc_peak.max(peak);
            self.acc_frames += 1;
            if self.acc_frames >= per_col {
                self.env[self.env_pos] = self.acc_peak.min(1.0);
                self.env_pos = (self.env_pos + 1) % COLS;
                self.acc_peak = 0.0;
                self.acc_frames = 0;
            }
        }
    }

    /// Copy what the visualizers need, oldest first.
    pub fn snapshot(&self) -> Snapshot {
        let mut env = Vec::with_capacity(COLS);
        env.extend_from_slice(&self.env[self.env_pos..]);
        env.extend_from_slice(&self.env[..self.env_pos]);
        let mut raw = Vec::with_capacity(RAW);
        raw.extend_from_slice(&self.raw[self.raw_pos..]);
        raw.extend_from_slice(&self.raw[..self.raw_pos]);
        let quiet_secs = self.last_push.map(|t| t.elapsed().as_secs_f64()).unwrap_or(f64::MAX);
        // Scroll silence in for the time nothing arrived (a paused source).
        let missing = ((quiet_secs / COL_SECS) as usize).min(COLS);
        if missing > 0 {
            env.drain(..missing);
            env.resize(COLS, 0.0);
        }
        if quiet_secs > IDLE_SECS {
            raw.fill(0.0);
        }
        Snapshot { env, raw, rate: self.rate }
    }
}

/// A copy of recent audio to draw from.
#[derive(Clone, Debug)]
pub struct Snapshot {
    env: Vec<f32>,
    raw: Vec<f32>,
    rate: u32,
}

impl Snapshot {
    /// The last 2 seconds as `cols` peak values in 0..=1, oldest first.
    pub fn waveform(&self, cols: usize) -> Vec<f32> {
        if cols == 0 {
            return Vec::new();
        }
        let n = self.env.len();
        (0..cols)
            .map(|i| {
                let a = i * n / cols;
                let b = ((i + 1) * n / cols).max(a + 1).min(n);
                self.env[a..b].iter().fold(0.0f32, |m, v| m.max(*v))
            })
            .collect()
    }

    /// Loudness in `bands` frequency bands, 30 Hz to 16 kHz on a log scale,
    /// each 0..=1 (0 = -72 dB or quieter, 1 = full scale).
    pub fn spectrum(&self, bands: usize) -> Vec<f32> {
        if bands == 0 {
            return Vec::new();
        }
        if self.raw.iter().all(|v| *v == 0.0) {
            return vec![0.0; bands];
        }
        let n = self.raw.len();
        let mut re: Vec<f32> = self
            .raw
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let w = 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (n - 1) as f32).cos();
                v * w
            })
            .collect();
        let mut im = vec![0.0f32; n];
        fft(&mut re, &mut im);
        let mag: Vec<f32> = (0..n / 2).map(|k| (re[k] * re[k] + im[k] * im[k]).sqrt()).collect();
        // A full-scale sine through a Hann window peaks at n/4.
        let full = n as f32 / 4.0;
        let bin_hz = self.rate as f32 / n as f32;
        let lo = 30.0f32;
        let hi = 16_000.0f32.min(self.rate as f32 / 2.0);
        let ratio = (hi / lo).powf(1.0 / bands as f32);
        (0..bands)
            .map(|b| {
                let f0 = lo * ratio.powi(b as i32);
                let f1 = f0 * ratio;
                let k0 = (f0 / bin_hz).floor() as usize;
                let k1 = ((f1 / bin_hz).ceil() as usize).max(k0 + 1).min(mag.len());
                let m = mag[k0.min(mag.len() - 1)..k1].iter().fold(0.0f32, |a, v| a.max(*v));
                // Music has less energy up high; tilt +3 dB per octave around
                // 1 kHz so the bars fill the width evenly.
                let centre = (f0 * f1).sqrt();
                let tilt = 3.0 * (centre / 1000.0).log2();
                let db = 20.0 * (m / full).max(1e-9).log10() + tilt;
                ((db + 72.0) / 72.0).clamp(0.0, 1.0)
            })
            .collect()
    }
}

/// In-place radix-2 FFT; `re.len()` must be a power of two.
fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * std::f64::consts::PI / len as f64;
        let (wr, wi) = (ang.cos() as f32, ang.sin() as f32);
        for start in (0..n).step_by(len) {
            let (mut cr, mut ci) = (1.0f32, 0.0f32);
            for k in 0..len / 2 {
                let a = start + k;
                let b = a + len / 2;
                let tr = re[b] * cr - im[b] * ci;
                let ti = re[b] * ci + im[b] * cr;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
                let nr = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = nr;
            }
        }
        len <<= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(freq: f32, amp: f32, secs: f32) -> Vec<f32> {
        let n = (48_000.0 * secs) as usize;
        (0..n)
            .flat_map(|i| {
                let v = (2.0 * std::f32::consts::PI * freq * i as f32 / 48_000.0).sin() * amp;
                [v, v]
            })
            .collect()
    }

    #[test]
    fn spectrum_peaks_at_the_tone() {
        let mut s = Scope::default();
        s.push(&tone(1000.0, 0.5, 0.1), 48_000, 2);
        let bands = s.snapshot().spectrum(32);
        let top = bands.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).unwrap().0;
        // 1 kHz sits about 60 % of the way along 30 Hz..16 kHz on a log scale.
        let expect = ((1000.0f32 / 30.0).ln() / (16_000.0f32 / 30.0).ln() * 32.0) as usize;
        assert!(top.abs_diff(expect) <= 1, "peak band {top}, expected ~{expect}: {bands:?}");
        assert!(bands[top] > 0.8, "a -6 dB tone reads loud: {}", bands[top]);
        assert!(bands[2] < 0.3, "low bands stay quiet: {}", bands[2]);
    }

    #[test]
    fn waveform_follows_level_and_scrolls() {
        let mut s = Scope::default();
        s.push(&tone(200.0, 0.8, 1.0), 48_000, 2);
        s.push(&vec![0.0; 48_000 / 2 * 2], 48_000, 2);
        let w = s.snapshot().waveform(40);
        assert_eq!(w.len(), 40);
        // 2 s window, oldest first: 0.5 s never filled, 1 s of tone, 0.5 s of silence.
        assert!(w[20..30].iter().all(|v| *v > 0.7), "tone visible: {w:?}");
        assert!(w[35..].iter().all(|v| *v < 0.01), "recent silence visible: {w:?}");
    }

    #[test]
    fn idle_scope_is_flat() {
        let s = Scope::default();
        let snap = s.snapshot();
        assert!(snap.waveform(10).iter().all(|v| *v == 0.0));
        assert!(snap.spectrum(10).iter().all(|v| *v == 0.0));
    }
}
