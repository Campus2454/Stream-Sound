//! Minimal PulseAudio "simple" API, loaded at runtime.
//!
//! Loading libpulse-simple with dlopen instead of linking it keeps the Linux
//! app a single file that only needs glibc: it talks to whatever sound server
//! the desktop runs (PulseAudio, or PipeWire through pipewire-pulse).

use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::sync::OnceLock;

#[repr(C)]
struct SampleSpec {
    format: c_int,
    rate: u32,
    channels: u8,
}

#[repr(C)]
pub struct BufferAttr {
    pub maxlength: u32,
    pub tlength: u32,
    pub prebuf: u32,
    pub minreq: u32,
    pub fragsize: u32,
}

impl BufferAttr {
    /// Every field "server default" (u32::MAX).
    pub fn default_all() -> BufferAttr {
        BufferAttr { maxlength: u32::MAX, tlength: u32::MAX, prebuf: u32::MAX, minreq: u32::MAX, fragsize: u32::MAX }
    }
}

const PA_SAMPLE_FLOAT32LE: c_int = 5;
const PA_SAMPLE_FLOAT32BE: c_int = 6;
const PA_STREAM_PLAYBACK: c_int = 1;
const PA_STREAM_RECORD: c_int = 2;

type NewFn = unsafe extern "C" fn(
    server: *const c_char,
    name: *const c_char,
    dir: c_int,
    dev: *const c_char,
    stream_name: *const c_char,
    ss: *const SampleSpec,
    map: *const c_void,
    attr: *const BufferAttr,
    error: *mut c_int,
) -> *mut c_void;
type ReadFn = unsafe extern "C" fn(s: *mut c_void, data: *mut c_void, bytes: usize, error: *mut c_int) -> c_int;
type WriteFn = unsafe extern "C" fn(s: *mut c_void, data: *const c_void, bytes: usize, error: *mut c_int) -> c_int;
type FreeFn = unsafe extern "C" fn(s: *mut c_void);
type LatencyFn = unsafe extern "C" fn(s: *mut c_void, error: *mut c_int) -> u64;
type StrErrorFn = unsafe extern "C" fn(error: c_int) -> *const c_char;

struct Api {
    _simple: libloading::Library,
    _pulse: libloading::Library,
    new: NewFn,
    read: ReadFn,
    write: WriteFn,
    free: FreeFn,
    latency: LatencyFn,
    strerror: StrErrorFn,
}

fn api() -> Result<&'static Api, String> {
    static API: OnceLock<Result<Api, String>> = OnceLock::new();
    API.get_or_init(|| unsafe {
        let missing = |e: libloading::Error| {
            format!("ไม่พบ PulseAudio/PipeWire (libpulse): {e}")
        };
        let pulse = libloading::Library::new("libpulse.so.0").map_err(missing)?;
        let simple = libloading::Library::new("libpulse-simple.so.0").map_err(missing)?;
        let new = *simple.get::<NewFn>(b"pa_simple_new\0").map_err(missing)?;
        let read = *simple.get::<ReadFn>(b"pa_simple_read\0").map_err(missing)?;
        let write = *simple.get::<WriteFn>(b"pa_simple_write\0").map_err(missing)?;
        let free = *simple.get::<FreeFn>(b"pa_simple_free\0").map_err(missing)?;
        let latency = *simple.get::<LatencyFn>(b"pa_simple_get_latency\0").map_err(missing)?;
        let strerror = *pulse.get::<StrErrorFn>(b"pa_strerror\0").map_err(missing)?;
        Ok(Api { _simple: simple, _pulse: pulse, new, read, write, free, latency, strerror })
    })
    .as_ref()
    .map_err(|e| e.clone())
}

fn error_text(api: &Api, code: c_int) -> String {
    // SAFETY: pa_strerror returns a static string (or null).
    let p = unsafe { (api.strerror)(code) };
    if p.is_null() {
        format!("error {code}")
    } else {
        unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
    }
}

/// One playback or record stream of interleaved f32 samples.
pub struct Stream {
    api: &'static Api,
    s: *mut c_void,
}

// A pa_simple handle may be used from any single thread at a time.
unsafe impl Send for Stream {}

pub enum Direction {
    Playback,
    Record,
}

impl Stream {
    pub fn open(
        dir: Direction,
        device: Option<&str>,
        stream_name: &str,
        rate: u32,
        channels: u8,
        attr: &BufferAttr,
    ) -> Result<Stream, String> {
        let api = api()?;
        let name = CString::new("Stream Sound").unwrap_or_default();
        let sname = CString::new(stream_name).unwrap_or_default();
        let dev = device.map(|d| CString::new(d).unwrap_or_default());
        let spec = SampleSpec {
            format: if cfg!(target_endian = "little") { PA_SAMPLE_FLOAT32LE } else { PA_SAMPLE_FLOAT32BE },
            rate,
            channels,
        };
        let dir = match dir {
            Direction::Playback => PA_STREAM_PLAYBACK,
            Direction::Record => PA_STREAM_RECORD,
        };
        let mut err: c_int = 0;
        // SAFETY: every pointer is valid for the duration of the call.
        let s = unsafe {
            (api.new)(
                std::ptr::null(),
                name.as_ptr(),
                dir,
                dev.as_ref().map(|d| d.as_ptr()).unwrap_or(std::ptr::null()),
                sname.as_ptr(),
                &spec,
                std::ptr::null(),
                attr,
                &mut err,
            )
        };
        if s.is_null() {
            return Err(error_text(api, err));
        }
        Ok(Stream { api, s })
    }

    pub fn read(&self, out: &mut [f32]) -> Result<(), String> {
        let mut err: c_int = 0;
        // SAFETY: `out` is a valid writable buffer of the given byte length.
        let r = unsafe { (self.api.read)(self.s, out.as_mut_ptr().cast(), std::mem::size_of_val(out), &mut err) };
        if r < 0 {
            Err(error_text(self.api, err))
        } else {
            Ok(())
        }
    }

    /// How long until audio written now is heard (playback), or how old
    /// audio read now is (record), in microseconds.
    pub fn latency_us(&self) -> Option<u64> {
        let mut err: c_int = 0;
        // SAFETY: `s` is a live stream.
        let us = unsafe { (self.api.latency)(self.s, &mut err) };
        (us != u64::MAX).then_some(us)
    }

    pub fn write(&self, data: &[f32]) -> Result<(), String> {
        let mut err: c_int = 0;
        // SAFETY: `data` is a valid readable buffer of the given byte length.
        let r = unsafe { (self.api.write)(self.s, data.as_ptr().cast(), std::mem::size_of_val(data), &mut err) };
        if r < 0 {
            Err(error_text(self.api, err))
        } else {
            Ok(())
        }
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        // SAFETY: `s` came from pa_simple_new and is freed exactly once.
        unsafe { (self.api.free)(self.s) }
    }
}
