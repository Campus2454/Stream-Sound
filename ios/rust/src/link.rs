//! App <-> screen-broadcast extension messages over loopback UDP.
//!
//! The extension is a separate process that iOS starts when a screen
//! broadcast begins. The app tells it where to send (and in which mode); the
//! extension reports back how sending is going, with visualizer data, so the
//! Send tab can show it. Loopback needs no App Group and no permission.

use crate::{guard, handle, SsndHandle};
use ssnd_core::Tap;
use std::ffi::c_char;
use std::net::{Ipv4Addr, UdpSocket};

/// Where the extension listens for the app's settings.
pub const LINK_EXTENSION_PORT: u16 = 47802;
/// Where the app listens for the extension's reports.
pub const LINK_APP_PORT: u16 = 47803;

pub struct SsndLink {
    sock: UdpSocket,
}

/// Listen on 127.0.0.1:`port`. NULL if the port is taken.
#[no_mangle]
pub extern "C" fn ssnd_link_open(port: u16) -> *mut SsndLink {
    guard(std::ptr::null_mut(), || {
        let Ok(sock) = UdpSocket::bind((Ipv4Addr::LOCALHOST, port)) else { return std::ptr::null_mut() };
        if sock.set_nonblocking(true).is_err() {
            return std::ptr::null_mut();
        }
        Box::into_raw(Box::new(SsndLink { sock }))
    })
}

#[no_mangle]
pub extern "C" fn ssnd_link_close(l: *mut SsndLink) {
    if !l.is_null() {
        // SAFETY: from `ssnd_link_open`, closed once.
        guard((), || drop(unsafe { Box::from_raw(l) }))
    }
}

/// Newest waiting message, copied into `buf` (older ones are dropped).
/// Returns its length, or -1 when nothing arrived.
#[no_mangle]
pub extern "C" fn ssnd_link_recv(l: *const SsndLink, buf: *mut u8, cap: u32) -> i32 {
    if buf.is_null() || cap == 0 {
        return -1;
    }
    // SAFETY: from `ssnd_link_open`; Swift passes `cap` writable bytes.
    let Some(l) = (unsafe { l.as_ref() }) else { return -1 };
    let out = unsafe { std::slice::from_raw_parts_mut(buf, cap as usize) };
    guard(-1, || {
        let mut got = -1;
        while let Ok((n, _)) = l.sock.recv_from(out) {
            got = n as i32;
        }
        got
    })
}

/// Send `len` bytes to 127.0.0.1:`port`.
#[no_mangle]
pub extern "C" fn ssnd_link_send(port: u16, data: *const u8, len: u32) -> bool {
    if data.is_null() {
        return false;
    }
    // SAFETY: Swift passes `len` readable bytes.
    let msg = unsafe { std::slice::from_raw_parts(data, len as usize) };
    guard(false, || send(port, msg))
}

fn send(port: u16, msg: &[u8]) -> bool {
    thread_local! {
        static SOCK: Option<UdpSocket> = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).ok();
    }
    SOCK.with(|s| s.as_ref().map(|s| s.send_to(msg, (Ipv4Addr::LOCALHOST, port)).is_ok()).unwrap_or(false))
}

fn floats(v: &[f32]) -> String {
    v.iter().map(|x| format!("{:.3}", if x.is_finite() { x.clamp(0.0, 1.0) } else { 0.0 })).collect::<Vec<_>>().join(",")
}

/// Extension side: send the app one report on sending (JSON with the
/// sender's state, 32 spectrum bands and 64 waveform columns).
#[no_mangle]
pub extern "C" fn ssnd_link_report(h: *const SsndHandle, port: u16, note: *const c_char) -> bool {
    let note = crate::cstr(note);
    guard(false, || {
        let Some(x) = handle(h) else { return false };
        let e = x.engine.read();
        let st = e.sender_stats();
        let snap = e.scope(Tap::Send);
        let spectrum = snap.as_ref().map(|s| s.spectrum(32)).unwrap_or_else(|| vec![0.0; 32]);
        let waveform = snap.as_ref().map(|s| s.waveform(64)).unwrap_or_else(|| vec![0.0; 64]);
        let dests: Vec<String> = e.send_dests().iter().map(|d| format!(r#""{d}""#)).collect();
        let msg = format!(
            r#"{{"sending":{},"packets":{},"level":{:.3},"captureMs":{},"error":"{}","note":"{}","dests":[{}],"spectrum":[{}],"waveform":[{}]}}"#,
            st.active,
            st.packets,
            if st.level.is_finite() { st.level } else { 0.0 },
            crate::num(st.capture_ms),
            crate::esc(&st.error.unwrap_or_default()),
            crate::esc(&note),
            dests.join(","),
            floats(&spectrum),
            floats(&waveform)
        );
        send(port, msg.as_bytes())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_messages() {
        let port = 47_861;
        let l = ssnd_link_open(port);
        assert!(!l.is_null());
        assert!(ssnd_link_open(port).is_null(), "port already taken");
        let mut buf = vec![0u8; 2048];
        assert_eq!(ssnd_link_recv(l, buf.as_mut_ptr(), 2048), -1);
        let a = b"first";
        let b = b"second";
        assert!(ssnd_link_send(port, a.as_ptr(), a.len() as u32));
        assert!(ssnd_link_send(port, b.as_ptr(), b.len() as u32));
        std::thread::sleep(std::time::Duration::from_millis(50));
        let n = ssnd_link_recv(l, buf.as_mut_ptr(), 2048);
        assert_eq!(&buf[..n as usize], b"second", "only the newest is kept");

        let name = std::ffi::CString::new("bc").unwrap();
        let mode = std::ffi::CString::new("balanced").unwrap();
        let h = crate::ssnd_create(name.as_ptr(), mode.as_ptr(), false);
        let dests = std::ffi::CString::new("127.0.0.1:47862").unwrap();
        assert!(crate::ssnd_start_sending(h, dests.as_ptr()).is_null());
        assert!(ssnd_link_report(h, port, std::ptr::null()));
        std::thread::sleep(std::time::Duration::from_millis(50));
        let n = ssnd_link_recv(l, buf.as_mut_ptr(), 2048);
        let s = String::from_utf8_lossy(&buf[..n as usize]).into_owned();
        assert!(s.starts_with(r#"{"sending":true"#), "{s}");
        assert!(s.contains(r#""dests":["127.0.0.1:47862"]"#), "{s}");
        assert!(n < 1400, "report fits one datagram: {n}");
        crate::ssnd_destroy(h);
        ssnd_link_close(l);
    }
}
