//! JNI functions behind `app.streamsound.Native` (Kotlin).
//!
//! Every entry point catches panics so a bug in the engine can never take
//! the app down; failures come back as error strings or silence instead.

use jni::objects::{JFloatArray, JObject, JString};
use jni::sys::{jboolean, jfloat, jint, jlong, jstring, JNI_TRUE};
use jni::JNIEnv;
use parking_lot::{Mutex, RwLock};
use ssnd_core::proto::DEFAULT_AUDIO_PORT;
use ssnd_core::{Engine, EngineConfig, Source};
use std::net::{SocketAddr, ToSocketAddrs};
use std::panic::{catch_unwind, AssertUnwindSafe};

struct Handle {
    engine: RwLock<Engine>,
    render_buf: Mutex<Vec<f32>>,
    capture_buf: Mutex<Vec<f32>>,
}

fn handle<'a>(h: jlong) -> Option<&'a Handle> {
    if h == 0 {
        None
    } else {
        // SAFETY: created by Box::into_raw in `create`, freed only in `destroy`.
        Some(unsafe { &*(h as *const Handle) })
    }
}

fn guard<T>(default: T, f: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(default)
}

fn jstr(env: &mut JNIEnv, s: &JString) -> String {
    env.get_string(s).map(|s| s.into()).unwrap_or_default()
}

fn out_str(env: &mut JNIEnv, s: &str) -> jstring {
    env.new_string(s).map(|s| s.into_raw()).unwrap_or(std::ptr::null_mut())
}

fn parse_addrs(list: &str) -> Vec<SocketAddr> {
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

#[no_mangle]
pub extern "system" fn Java_app_streamsound_Native_create(
    mut env: JNIEnv,
    _this: JObject,
    name: JString,
    latency_ms: jfloat,
) -> jlong {
    let name = jstr(&mut env, &name);
    guard(0, || {
        let cfg = EngineConfig {
            name,
            audio_port: DEFAULT_AUDIO_PORT,
            latency_ms: latency_ms as f64,
            discovery: true,
            external_output: true,
        };
        let h = Box::new(Handle {
            engine: RwLock::new(Engine::new(cfg)),
            render_buf: Mutex::new(vec![0.0; 4096]),
            capture_buf: Mutex::new(vec![0.0; 4096]),
        });
        Box::into_raw(h) as jlong
    })
}

#[no_mangle]
pub extern "system" fn Java_app_streamsound_Native_destroy(_env: JNIEnv, _this: JObject, h: jlong) {
    if h != 0 {
        guard((), || {
            // SAFETY: pointer came from `create` and the app calls this once.
            drop(unsafe { Box::from_raw(h as *mut Handle) });
        })
    }
}

/// Returns an empty string on success, otherwise the error message.
#[no_mangle]
pub extern "system" fn Java_app_streamsound_Native_startReceiving(mut env: JNIEnv, _this: JObject, h: jlong) -> jstring {
    let msg = guard("internal error".to_string(), || match handle(h) {
        Some(x) => x.engine.write().start_receiving().err().map(|e| format!("{e:#}")).unwrap_or_default(),
        None => "engine not started".into(),
    });
    out_str(&mut env, &msg)
}

#[no_mangle]
pub extern "system" fn Java_app_streamsound_Native_stopReceiving(_env: JNIEnv, _this: JObject, h: jlong) {
    guard((), || {
        if let Some(x) = handle(h) {
            x.engine.write().stop_receiving();
        }
    })
}

/// Fill `out` with `frames` frames of mixed audio (interleaved `ch` channels).
#[no_mangle]
pub extern "system" fn Java_app_streamsound_Native_render(
    env: JNIEnv,
    _this: JObject,
    h: jlong,
    out: JFloatArray,
    frames: jint,
    rate: jint,
    ch: jint,
) {
    guard((), || {
        let Some(x) = handle(h) else { return };
        let n = (frames.max(0) as usize) * (ch.max(1) as usize);
        let mut buf = x.render_buf.lock();
        if buf.len() < n {
            buf.resize(n, 0.0);
        }
        let slice = &mut buf[..n];
        x.engine.read().render(slice, rate.max(8000) as u32, ch.max(1) as usize);
        let _ = env.set_float_array_region(&out, 0, slice);
    })
}

/// Start sending audio that the app pushes with `pushCapture`.
#[no_mangle]
pub extern "system" fn Java_app_streamsound_Native_startSending(
    mut env: JNIEnv,
    _this: JObject,
    h: jlong,
    dests: JString,
) -> jstring {
    let dests = jstr(&mut env, &dests);
    let msg = guard("internal error".to_string(), || match handle(h) {
        Some(x) => x
            .engine
            .write()
            .start_sending(Source::External, parse_addrs(&dests), false)
            .err()
            .map(|e| format!("{e:#}"))
            .unwrap_or_default(),
        None => "engine not started".into(),
    });
    out_str(&mut env, &msg)
}

#[no_mangle]
pub extern "system" fn Java_app_streamsound_Native_stopSending(_env: JNIEnv, _this: JObject, h: jlong) {
    guard((), || {
        if let Some(x) = handle(h) {
            x.engine.write().stop_sending();
        }
    })
}

#[no_mangle]
pub extern "system" fn Java_app_streamsound_Native_pushCapture(
    env: JNIEnv,
    _this: JObject,
    h: jlong,
    data: JFloatArray,
    len: jint,
    rate: jint,
    ch: jint,
) {
    guard((), || {
        let Some(x) = handle(h) else { return };
        let n = len.max(0) as usize;
        let mut buf = x.capture_buf.lock();
        if buf.len() < n {
            buf.resize(n, 0.0);
        }
        if env.get_float_array_region(&data, 0, &mut buf[..n]).is_ok() {
            x.engine.read().push_capture(&buf[..n], rate.max(8000) as u32, ch.max(1) as usize);
        }
    })
}

#[no_mangle]
pub extern "system" fn Java_app_streamsound_Native_setSendDests(mut env: JNIEnv, _this: JObject, h: jlong, dests: JString) {
    let dests = jstr(&mut env, &dests);
    guard((), || {
        if let Some(x) = handle(h) {
            x.engine.read().set_send_dests(parse_addrs(&dests));
        }
    })
}

#[no_mangle]
pub extern "system" fn Java_app_streamsound_Native_setForward(mut env: JNIEnv, _this: JObject, h: jlong, dests: JString) {
    let dests = jstr(&mut env, &dests);
    guard((), || {
        if let Some(x) = handle(h) {
            x.engine.read().set_forward(parse_addrs(&dests));
        }
    })
}

#[no_mangle]
pub extern "system" fn Java_app_streamsound_Native_setPlayLocal(_env: JNIEnv, _this: JObject, h: jlong, on: jboolean) {
    guard((), || {
        if let Some(x) = handle(h) {
            x.engine.read().set_play_local(on == JNI_TRUE);
        }
    })
}

#[no_mangle]
pub extern "system" fn Java_app_streamsound_Native_setVolume(_env: JNIEnv, _this: JObject, h: jlong, v: jfloat) {
    guard((), || {
        if let Some(x) = handle(h) {
            x.engine.read().set_volume(v);
        }
    })
}

#[no_mangle]
pub extern "system" fn Java_app_streamsound_Native_setLatency(_env: JNIEnv, _this: JObject, h: jlong, ms: jfloat) {
    guard((), || {
        if let Some(x) = handle(h) {
            x.engine.read().set_latency_ms(ms as f64);
        }
    })
}

/// Everything the UI shows, as one JSON object.
#[no_mangle]
pub extern "system" fn Java_app_streamsound_Native_stateJson(mut env: JNIEnv, _this: JObject, h: jlong) -> jstring {
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
                    r#"{{"id":{},"name":"{}","from":"{}","bufferMs":{:.1},"targetMs":{:.1},"lost":{},"underruns":{},"level":{:.3},"rate":{}}}"#,
                    s.id,
                    esc(&s.name),
                    esc(&s.from),
                    s.buffer_ms,
                    s.target_ms,
                    s.lost,
                    s.underruns,
                    s.level,
                    s.sample_rate
                )
            })
            .collect();
        let st = e.sender_stats();
        format!(
            r#"{{"name":"{}","receiving":{},"receiverError":"{}","playLocal":{},"forwarded":{},"sending":{},"sentPackets":{},"sendLevel":{:.3},"sendError":"{}","peers":[{}],"streams":[{}]}}"#,
            esc(&e.name),
            e.is_receiving(),
            esc(&e.receiver_error().unwrap_or_default()),
            e.play_local(),
            e.forwarded_packets(),
            st.active,
            st.packets,
            st.level,
            esc(&st.error.unwrap_or_default()),
            peers.join(","),
            streams.join(",")
        )
    });
    out_str(&mut env, &json)
}
