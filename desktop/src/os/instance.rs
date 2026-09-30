//! One copy of the app at a time. The first copy listens on a local port;
//! a second copy asks it to show its window and then quits, so opening the
//! app while it sits in the tray brings the window back instead of failing
//! to receive on a port that is already taken.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::time::{Duration, Instant};

/// Local only, next to the audio (47800) and discovery (47801) ports.
const PORT: u16 = 47809;
const ASK: &[u8] = b"SSND show\n";
/// From the installer: close so the files can be replaced. Same length as
/// `ASK`, so copies from before it read it whole and ignore it.
const QUIT: &[u8] = b"SSND quit\n";
const ANSWER: &[u8] = b"SSND ok\n";

pub enum Claim {
    /// This is the only copy; `serve` answers later copies.
    First(TcpListener),
    /// Another copy was running and has been asked to show itself.
    Second,
    /// Couldn't tell (the port is used by something else); run anyway.
    Unknown,
}

/// Call once at start. `wait_for_old` is for the copy an update starts: the
/// old copy is still quitting, so wait for its port instead of waking it.
pub fn claim(wait_for_old: bool) -> Claim {
    let addr = addr();
    let until = Instant::now() + Duration::from_secs(if wait_for_old { 10 } else { 0 });
    loop {
        if let Ok(l) = TcpListener::bind(addr) {
            return Claim::First(l);
        }
        if Instant::now() >= until {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    if wait_for_old {
        return Claim::Unknown;
    }
    #[cfg(windows)]
    super::windows::let_other_copy_come_forward();
    match ask_first_copy(addr, ASK) {
        Ok(true) => Claim::Second,
        _ => Claim::Unknown,
    }
}

fn addr() -> SocketAddr {
    SocketAddr::from((Ipv4Addr::LOCALHOST, PORT))
}

/// Whether a copy of the app is running (holds the port).
pub fn running() -> bool {
    TcpListener::bind(addr()).is_err()
}

/// Ask the running copy, if any, to close, and wait up to `wait` for it to
/// be gone. True when no copy is left running.
pub fn ask_to_quit(wait: Duration) -> bool {
    if !running() {
        return true;
    }
    if !matches!(ask_first_copy(addr(), QUIT), Ok(true)) {
        // A copy from before this request, or not the app at all.
        return !running();
    }
    let until = Instant::now() + wait;
    while Instant::now() < until {
        if !running() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    !running()
}

fn ask_first_copy(addr: SocketAddr, ask: &[u8]) -> std::io::Result<bool> {
    let mut s = TcpStream::connect_timeout(&addr, Duration::from_millis(500))?;
    s.set_read_timeout(Some(Duration::from_secs(1)))?;
    s.write_all(ask)?;
    let mut buf = [0u8; 16];
    let mut got = 0;
    while got < ANSWER.len() {
        let n = s.read(&mut buf[got..])?;
        if n == 0 {
            break;
        }
        got += n;
    }
    Ok(&buf[..got] == ANSWER)
}

impl Claim {
    /// Answer later copies in the background, calling `show` for each, and
    /// `quit` when an installer needs the app closed.
    pub fn serve(self, show: impl Fn() + Send + 'static, quit: impl Fn() + Send + 'static) {
        let Claim::First(listener) = self else { return };
        let _ = std::thread::Builder::new().name("ssnd-instance".into()).spawn(move || {
            for conn in listener.incoming() {
                let Ok(mut s) = conn else { continue };
                let _ = s.set_read_timeout(Some(Duration::from_secs(1)));
                let mut buf = [0u8; 16];
                let mut got = 0;
                while got < ASK.len() {
                    match s.read(&mut buf[got..]) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => got += n,
                    }
                }
                if &buf[..got] == ASK {
                    show();
                    let _ = s.write_all(ANSWER);
                } else if &buf[..got] == QUIT {
                    let _ = s.write_all(ANSWER);
                    quit();
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn second_copy_wakes_the_first() {
        // Runs on the real port, so skip if something already holds it.
        let Claim::First(l) = claim(false) else { return };
        let shown = Arc::new(AtomicUsize::new(0));
        let s = shown.clone();
        let quits = Arc::new(AtomicUsize::new(0));
        let q = quits.clone();
        Claim::First(l).serve(
            move || {
                s.fetch_add(1, Ordering::Relaxed);
            },
            move || {
                q.fetch_add(1, Ordering::Relaxed);
            },
        );
        assert!(matches!(claim(false), Claim::Second));
        assert_eq!(shown.load(Ordering::Relaxed), 1);
        // The listener stays up in this test, so the copy "doesn't go".
        assert!(!ask_to_quit(Duration::from_millis(300)));
        assert_eq!(quits.load(Ordering::Relaxed), 1);
        assert_eq!(shown.load(Ordering::Relaxed), 1);
    }
}
