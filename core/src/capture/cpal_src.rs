//! Capture through cpal. On Windows, opening an *output* device for input
//! gives WASAPI loopback, i.e. everything the computer is playing.

use super::{SampleSink, Source, SourceInfo, SourceKind};
use anyhow::{anyhow, Context};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use std::sync::atomic::{AtomicBool, Ordering};

/// Delay between sound being played/recorded and our callback getting it.
fn note_capture(info: &cpal::InputCallbackInfo) {
    let ts = info.timestamp();
    if let Some(d) = ts.callback.duration_since(&ts.capture) {
        super::CAPTURE_LATENCY_US.store(d.as_micros() as u64, Ordering::Relaxed);
    }
}

/// Windows loopback delivers nothing while no app is playing, so the stream
/// would stop and start. Playing silence on the same device keeps it running.
fn keep_alive(device: &cpal::Device) -> Option<cpal::Stream> {
    let cfg = device.default_output_config().ok()?;
    let config: StreamConfig = cfg.config();
    let stream = match cfg.sample_format() {
        SampleFormat::F32 => device.build_output_stream(&config, |d: &mut [f32], _| d.fill(0.0), |_| {}, None),
        SampleFormat::I16 => device.build_output_stream(&config, |d: &mut [i16], _| d.fill(0), |_| {}, None),
        SampleFormat::I32 => device.build_output_stream(&config, |d: &mut [i32], _| d.fill(0), |_| {}, None),
        _ => return None,
    }
    .ok()?;
    stream.play().ok()?;
    Some(stream)
}
use std::sync::Arc;
use std::time::Duration;

pub fn list_inputs() -> Vec<SourceInfo> {
    let host = cpal::default_host();
    host.input_devices()
        .map(|it| {
            it.filter_map(|d| d.name().ok())
                .map(|name| SourceInfo {
                    label: format!("อินพุต: {name}"),
                    source: Source::Input { name },
                    kind: SourceKind::Input,
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn run(source: &Source, sink: SampleSink, stop: &AtomicBool) -> anyhow::Result<()> {
    let host = cpal::default_host();
    let (device, supported) = match source {
        Source::System => {
            let d = host.default_output_device().context("no output device to capture from")?;
            let c = d.default_output_config().context("output device has no config")?;
            (d, c)
        }
        Source::Input { name } => {
            let d = host
                .input_devices()?
                .find(|d| d.name().ok().as_deref() == Some(name.as_str()))
                .with_context(|| format!("input device not found: {name}"))?;
            let c = d.default_input_config()?;
            (d, c)
        }
        _ => return Err(anyhow!("source not supported on this platform")),
    };
    let rate = supported.sample_rate().0;
    let ch = supported.channels() as usize;
    let config: StreamConfig = supported.config();
    let failed = Arc::new(AtomicBool::new(false));
    let err_fn = {
        let failed = failed.clone();
        move |_e: cpal::StreamError| failed.store(true, Ordering::Relaxed)
    };
    let sink = Arc::new(parking_lot::Mutex::new(sink));
    let _silence = if matches!(source, Source::System) { keep_alive(&device) } else { None };
    let stream = match supported.sample_format() {
        SampleFormat::F32 => {
            let sink = sink.clone();
            device.build_input_stream(
                &config,
                move |d: &[f32], info: &cpal::InputCallbackInfo| {
                    note_capture(info);
                    (sink.lock())(d, rate, ch)
                },
                err_fn,
                None,
            )?
        }
        SampleFormat::I16 => {
            let sink = sink.clone();
            let mut tmp = Vec::new();
            device.build_input_stream(
                &config,
                move |d: &[i16], info: &cpal::InputCallbackInfo| {
                    note_capture(info);
                    tmp.clear();
                    tmp.extend(d.iter().map(|s| *s as f32 / 32768.0));
                    (sink.lock())(&tmp, rate, ch)
                },
                err_fn,
                None,
            )?
        }
        SampleFormat::I32 => {
            let sink = sink.clone();
            let mut tmp = Vec::new();
            device.build_input_stream(
                &config,
                move |d: &[i32], info: &cpal::InputCallbackInfo| {
                    note_capture(info);
                    tmp.clear();
                    tmp.extend(d.iter().map(|s| *s as f32 / 2147483648.0));
                    (sink.lock())(&tmp, rate, ch)
                },
                err_fn,
                None,
            )?
        }
        f => return Err(anyhow!("unsupported sample format {f:?}")),
    };
    stream.play()?;
    while !stop.load(Ordering::Relaxed) {
        if failed.load(Ordering::Relaxed) {
            return Err(anyhow!("capture device stopped; reconnecting"));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}
