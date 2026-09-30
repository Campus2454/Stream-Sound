//! Command-line mode, handy for testing and for headless relays.
//!
//!   stream-sound recv  [--port 47800] [--mode game|balanced|music] [--forward IP[:PORT],..] [--no-play] [--seconds N]
//!   stream-sound send  --to IP[:PORT],.. [--source system|tone|app:NAME|input:NAME] [--mode ...] [--seconds N]
//!   stream-sound sources
//!   stream-sound peers
//!   stream-sound update     # install the latest GitHub release over this file

use ssnd_core::proto::DEFAULT_AUDIO_PORT;
use ssnd_core::{list_sources, Engine, EngineConfig, Mode, Source};
use std::net::{SocketAddr, ToSocketAddrs};
use std::time::{Duration, Instant};

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

fn parse_addrs(list: &str) -> anyhow::Result<Vec<SocketAddr>> {
    list.split(',')
        .filter(|s| !s.is_empty())
        .map(|s| {
            let s = if s.contains(':') { s.to_string() } else { format!("{s}:{DEFAULT_AUDIO_PORT}") };
            s.to_socket_addrs()?.next().ok_or_else(|| anyhow::anyhow!("bad address {s}"))
        })
        .collect()
}

fn parse_source(s: &str) -> Source {
    match s {
        "system" => Source::System,
        "tone" => Source::Tone,
        _ if s.starts_with("app:") => Source::App { key: s[4..].to_string() },
        _ if s.starts_with("input:") => Source::Input { name: s[6..].to_string() },
        _ => Source::System,
    }
}

pub fn run(args: Vec<String>) -> anyhow::Result<()> {
    let cmd = args.first().map(String::as_str).unwrap_or("");
    let seconds: Option<u64> = arg(&args, "--seconds").and_then(|s| s.parse().ok());
    let deadline = seconds.map(|s| Instant::now() + Duration::from_secs(s));
    let mut cfg = EngineConfig::default();
    if let Some(p) = arg(&args, "--port").and_then(|s| s.parse().ok()) {
        cfg.audio_port = p;
    }
    if let Some(m) = arg(&args, "--mode") {
        cfg.mode = Mode::parse(&m).ok_or_else(|| anyhow::anyhow!("--mode must be game, balanced or music"))?;
    }
    if let Some(n) = arg(&args, "--name") {
        cfg.name = n;
    }
    match cmd {
        "update" => {
            let beta = crate::settings::load().beta;
            match crate::updater::current() {
                Some(v) => println!("this is version {v}"),
                None => println!("this is a local build"),
            }
            match crate::updater::check(beta)? {
                crate::updater::Check::UpToDate => println!("already up to date"),
                crate::updater::Check::NoRelease if beta => println!("no release has been published on GitHub yet"),
                crate::updater::Check::NoRelease => println!("no official release yet (betas are off in Settings)"),
                crate::updater::Check::Newer(rel) => {
                    println!("downloading {} ...", rel.tag);
                    let file = crate::updater::download(&rel, |_| {})?;
                    crate::updater::start_install(&file)?;
                    println!("the installer is updating to version {}", rel.version);
                }
            }
            Ok(())
        }
        "sources" => {
            for s in list_sources() {
                println!("{:?}\t{}", s.source, s.label);
            }
            Ok(())
        }
        "peers" => {
            let e = Engine::new(cfg);
            std::thread::sleep(Duration::from_secs(3));
            for p in e.peers() {
                println!("{}\t{}\t{}", p.name, p.addr(), if p.receiving { "receiving" } else { "" });
            }
            Ok(())
        }
        "recv" => {
            let mut e = Engine::new(cfg);
            if let Some(f) = arg(&args, "--forward") {
                e.set_forward(parse_addrs(&f)?);
            }
            e.set_play_local(!args.iter().any(|a| a == "--no-play"));
            e.start_receiving()?;
            let ips: Vec<String> = e.local_ips().iter().map(|i| i.to_string()).collect();
            println!("receiving on {} port {} ({} mode)", ips.join(", "), e.audio_port, e.mode().as_str());
            loop {
                std::thread::sleep(Duration::from_secs(1));
                if let Some(err) = e.receiver_error() {
                    println!("output: {err}");
                }
                let out_ms = e.output_latency_ms();
                for s in e.streams() {
                    println!(
                        "stream {:08x} '{}' from {} {}Hz/{}ch delay~{:.0}ms (capture {:.1} + buffer {:.1} + speaker {:.1}) target={:.1}ms lost={} late={} underruns={} level={:.2}",
                        s.id, s.name, s.from, s.sample_rate, s.channels,
                        s.capture_ms + s.buffer_ms + out_ms, s.capture_ms, s.buffer_ms, out_ms,
                        s.target_ms, s.lost, s.late, s.underruns, s.level
                    );
                }
                println!(
                    "output {} Hz, block {} frames, speaker delay {:.1} ms",
                    ssnd_core::engine::OUTPUT_RATE.load(std::sync::atomic::Ordering::Relaxed),
                    ssnd_core::engine::OUTPUT_BLOCK_FRAMES.load(std::sync::atomic::Ordering::Relaxed),
                    out_ms
                );
                if e.forwarded_packets() > 0 {
                    println!("forwarded {} packets", e.forwarded_packets());
                }
                if deadline.map(|d| Instant::now() > d).unwrap_or(false) {
                    return Ok(());
                }
            }
        }
        "send" => {
            let to = arg(&args, "--to").ok_or_else(|| anyhow::anyhow!("--to is required"))?;
            let source = parse_source(&arg(&args, "--source").unwrap_or_else(|| "system".into()));
            cfg.discovery = false;
            let mut e = Engine::new(cfg);
            e.start_sending(source, parse_addrs(&to)?, args.iter().any(|a| a == "--keep-local"))?;
            loop {
                std::thread::sleep(Duration::from_secs(1));
                let s = e.sender_stats();
                println!(
                    "sent {} packets level={:.2} capture delay {:.1} ms {}",
                    s.packets,
                    s.level,
                    s.capture_ms,
                    s.error.unwrap_or_default()
                );
                if deadline.map(|d| Instant::now() > d).unwrap_or(false) {
                    return Ok(());
                }
            }
        }
        _ => {
            eprintln!("usage: stream-sound [recv|send|sources|peers|update] (no arguments opens the app)");
            Ok(())
        }
    }
}
