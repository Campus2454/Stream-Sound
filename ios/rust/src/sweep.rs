//! Discovery without broadcast.
//!
//! iOS drops UDP broadcast (both ways) for apps without Apple's restricted
//! multicast entitlement, which a sideloaded app can't have. So the iPhone
//! says hello by unicast instead: to every address in its Wi-Fi subnet, to
//! devices it already knows, and to devices added by IP. Other devices list
//! it as usual and answer with unicast hellos of their own, which the
//! engine's discovery socket hears, so the iPhone learns about them too.

use parking_lot::{Mutex, RwLock};
use ssnd_core::proto::DISCOVERY_PORT;
use ssnd_core::Engine;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

/// Largest subnet swept (a /22); bigger networks get the /24 around us.
const MAX_HOSTS: u32 = 1022;
/// Full sweeps while nobody has answered yet, and once we know someone.
const SWEEP_EAGER: Duration = Duration::from_secs(2);
const SWEEP_CALM: Duration = Duration::from_secs(10);

pub struct Sweep {
    stop: Arc<AtomicBool>,
    manual: Arc<Mutex<Vec<Ipv4Addr>>>,
}

impl Sweep {
    pub fn start(engine: Arc<RwLock<Engine>>) -> Option<Sweep> {
        let sock = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
        let stop = Arc::new(AtomicBool::new(false));
        let manual: Arc<Mutex<Vec<Ipv4Addr>>> = Arc::default();
        {
            let stop = stop.clone();
            let manual = manual.clone();
            thread::Builder::new()
                .name("ssnd-sweep".into())
                .spawn(move || run(engine, sock, stop, manual))
                .ok()?;
        }
        Some(Sweep { stop, manual })
    }

    pub fn set_manual(&self, ips: Vec<Ipv4Addr>) {
        *self.manual.lock() = ips;
    }
}

impl Drop for Sweep {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn run(engine: Arc<RwLock<Engine>>, sock: UdpSocket, stop: Arc<AtomicBool>, manual: Arc<Mutex<Vec<Ipv4Addr>>>) {
    let mut last_sweep: Option<Instant> = None;
    let mut last_subnet = Vec::new();
    while !stop.load(Ordering::Relaxed) {
        let (msg, known) = {
            let e = engine.read();
            let msg = format!(
                "SSND1|{}|{}|{}|{}",
                e.id,
                e.name().replace('|', " "),
                e.audio_port,
                e.is_receiving() as u8
            );
            let known: Vec<Ipv4Addr> = e
                .peers()
                .iter()
                .filter_map(|p| match p.ip {
                    IpAddr::V4(v) if !v.is_loopback() => Some(v),
                    _ => None,
                })
                .collect();
            (msg, known)
        };
        let mut direct = known.clone();
        direct.extend(manual.lock().iter().copied());
        direct.sort();
        direct.dedup();
        for ip in &direct {
            let _ = sock.send_to(msg.as_bytes(), SocketAddr::new(IpAddr::V4(*ip), DISCOVERY_PORT));
        }

        let subnet = subnet_hosts();
        let every = if known.is_empty() { SWEEP_EAGER } else { SWEEP_CALM };
        // A new network (or a new address) is swept at once.
        let due = subnet != last_subnet || last_sweep.map(|t| t.elapsed() >= every).unwrap_or(true);
        if due {
            last_sweep = Some(Instant::now());
            for (i, ip) in subnet.iter().enumerate() {
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                if !direct.contains(ip) {
                    let _ = sock.send_to(msg.as_bytes(), SocketAddr::new(IpAddr::V4(*ip), DISCOVERY_PORT));
                }
                // Spread the burst so the Wi-Fi queue never overflows.
                if i % 64 == 63 {
                    thread::sleep(Duration::from_millis(3));
                }
            }
            last_subnet = subnet;
        }
        for _ in 0..10 {
            if stop.load(Ordering::Relaxed) {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
}

/// Every other address in this device's Wi-Fi / Ethernet subnets.
fn subnet_hosts() -> Vec<Ipv4Addr> {
    let mut out = Vec::new();
    for i in if_addrs::get_if_addrs().unwrap_or_default() {
        // en* = Wi-Fi / wired / USB Ethernet on Apple devices. Skip cellular
        // (pdp_ip*), VPN tunnels (utun*) and Personal Hotspot bridges' peers.
        if i.is_loopback() || !(i.name.starts_with("en") || i.name.starts_with("bridge")) {
            continue;
        }
        if let if_addrs::IfAddr::V4(a) = i.addr {
            if a.ip.is_link_local() || !a.ip.is_private() {
                continue;
            }
            out.extend(hosts(a.ip, a.netmask));
        }
    }
    out.sort();
    out.dedup();
    out
}

fn hosts(ip: Ipv4Addr, mask: Ipv4Addr) -> Vec<Ipv4Addr> {
    let me = u32::from(ip);
    let mut m = u32::from(mask);
    if m == 0 || !m > MAX_HOSTS + 1 {
        m = 0xFFFF_FF00;
    }
    let net = me & m;
    let size = !m;
    (1..size).map(|h| net | h).filter(|a| *a != me).map(Ipv4Addr::from).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subnet_sizes() {
        let h = hosts(Ipv4Addr::new(192, 168, 1, 23), Ipv4Addr::new(255, 255, 255, 0));
        assert_eq!(h.len(), 253);
        assert!(!h.contains(&Ipv4Addr::new(192, 168, 1, 23)));
        assert!(!h.contains(&Ipv4Addr::new(192, 168, 1, 0)));
        assert!(!h.contains(&Ipv4Addr::new(192, 168, 1, 255)));
        let h = hosts(Ipv4Addr::new(10, 0, 5, 9), Ipv4Addr::new(255, 255, 252, 0));
        assert_eq!(h.len(), 1021);
        // A /16 falls back to the /24 around us.
        let h = hosts(Ipv4Addr::new(172, 16, 9, 9), Ipv4Addr::new(255, 255, 0, 0));
        assert_eq!(h.len(), 253);
        assert!(h.iter().all(|a| a.octets()[2] == 9));
    }
}
