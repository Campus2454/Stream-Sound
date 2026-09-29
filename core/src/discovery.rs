//! LAN discovery: every device broadcasts a small hello once a second and
//! keeps a list of the devices it hears from.

use crate::proto::DISCOVERY_PORT;
use parking_lot::Mutex;
use socket2::{Domain, Protocol, Socket, Type};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const PEER_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug)]
pub struct Peer {
    pub id: String,
    pub name: String,
    pub ip: IpAddr,
    pub audio_port: u16,
    pub receiving: bool,
    pub last_seen: Instant,
}

impl Peer {
    pub fn addr(&self) -> SocketAddr {
        SocketAddr::new(self.ip, self.audio_port)
    }
}

pub struct Discovery {
    peers: Arc<Mutex<HashMap<String, Peer>>>,
    stop: Arc<AtomicBool>,
}

pub struct Announce {
    pub id: String,
    pub name: Arc<parking_lot::RwLock<String>>,
    pub audio_port: u16,
    pub receiving: Arc<AtomicBool>,
}

fn bind_shared(port: u16) -> std::io::Result<UdpSocket> {
    let s = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    s.set_reuse_address(true)?;
    #[cfg(all(unix, not(target_os = "solaris")))]
    s.set_reuse_port(true)?;
    s.set_broadcast(true)?;
    s.bind(&SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port)).into())?;
    Ok(s.into())
}

/// Virtual adapters (VMs, containers) other devices can't reach.
fn is_virtual(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    ["docker", "veth", "br-", "virbr", "vethernet", "virtualbox", "vmware", "vmnet", "wsl"]
        .iter()
        .any(|p| n.starts_with(p) || n.contains(p))
}

/// This device's IPv4 LAN addresses, the one the OS routes through first.
pub fn local_ips() -> Vec<Ipv4Addr> {
    let mut v = Vec::new();
    // The address used to reach the internet; connecting a UDP socket sends nothing.
    if let Ok(s) = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)) {
        if s.connect((Ipv4Addr::new(8, 8, 8, 8), 53)).is_ok() {
            if let Ok(SocketAddr::V4(a)) = s.local_addr() {
                if !a.ip().is_unspecified() && !a.ip().is_loopback() {
                    v.push(*a.ip());
                }
            }
        }
    }
    let mut others: Vec<Ipv4Addr> = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter(|i| !i.is_loopback() && !is_virtual(&i.name))
        .filter_map(|i| match i.addr {
            if_addrs::IfAddr::V4(a) if !a.ip.is_link_local() => Some(a.ip),
            _ => None,
        })
        .collect();
    others.sort_by_key(|ip| !ip.is_private());
    for ip in others {
        if !v.contains(&ip) {
            v.push(ip);
        }
    }
    v
}

fn broadcast_targets() -> Vec<SocketAddr> {
    let mut v: Vec<SocketAddr> = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter(|i| !i.is_loopback())
        .filter_map(|i| match i.addr {
            if_addrs::IfAddr::V4(a) => a.broadcast.map(|b| SocketAddr::new(IpAddr::V4(b), DISCOVERY_PORT)),
            _ => None,
        })
        .collect();
    v.push(SocketAddr::new(IpAddr::V4(Ipv4Addr::BROADCAST), DISCOVERY_PORT));
    // Loopback lets two copies on one computer find each other.
    v.push(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), DISCOVERY_PORT));
    v.sort();
    v.dedup();
    v
}

impl Discovery {
    pub fn start(me: Announce) -> anyhow::Result<Discovery> {
        let peers: Arc<Mutex<HashMap<String, Peer>>> = Arc::default();
        let stop = Arc::new(AtomicBool::new(false));
        let sock = bind_shared(DISCOVERY_PORT)?;
        sock.set_read_timeout(Some(Duration::from_millis(250)))?;
        let tx = sock.try_clone()?;

        {
            let stop = stop.clone();
            thread::Builder::new().name("ssnd-announce".into()).spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    let msg = format!(
                        "SSND1|{}|{}|{}|{}",
                        me.id,
                        me.name.read().replace('|', " "),
                        me.audio_port,
                        me.receiving.load(Ordering::Relaxed) as u8
                    );
                    for t in broadcast_targets() {
                        let _ = tx.send_to(msg.as_bytes(), t);
                    }
                    thread::sleep(Duration::from_millis(1000));
                }
            })?;
        }
        {
            let stop = stop.clone();
            let peers = peers.clone();
            thread::Builder::new().name("ssnd-discover".into()).spawn(move || {
                let mut buf = [0u8; 512];
                while !stop.load(Ordering::Relaxed) {
                    if let Ok((n, from)) = sock.recv_from(&mut buf) {
                        if let Some(p) = parse_hello(&buf[..n], from.ip()) {
                            let mut map = peers.lock();
                            // Prefer a LAN address over loopback for the same device.
                            let keep_old = map
                                .get(&p.id)
                                .map(|old| p.ip.is_loopback() && !old.ip.is_loopback()
                                    && old.last_seen.elapsed() < PEER_TIMEOUT)
                                .unwrap_or(false);
                            if keep_old {
                                if let Some(old) = map.get_mut(&p.id) {
                                    old.last_seen = Instant::now();
                                    old.receiving = p.receiving;
                                }
                            } else {
                                map.insert(p.id.clone(), p);
                            }
                        }
                    }
                    peers.lock().retain(|_, p| p.last_seen.elapsed() < PEER_TIMEOUT);
                }
            })?;
        }
        Ok(Discovery { peers, stop })
    }

    /// Other devices, excluding `self_id`.
    pub fn peers(&self, self_id: &str) -> Vec<Peer> {
        let mut v: Vec<Peer> = self.peers.lock().values().filter(|p| p.id != self_id).cloned().collect();
        v.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
        v
    }
}

impl Drop for Discovery {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn parse_hello(buf: &[u8], ip: IpAddr) -> Option<Peer> {
    let s = std::str::from_utf8(buf).ok()?;
    let mut it = s.split('|');
    if it.next()? != "SSND1" {
        return None;
    }
    Some(Peer {
        id: it.next()?.to_string(),
        name: it.next()?.to_string(),
        audio_port: it.next()?.parse().ok()?,
        receiving: it.next().map(|v| v == "1").unwrap_or(false),
        ip,
        last_seen: Instant::now(),
    })
}
