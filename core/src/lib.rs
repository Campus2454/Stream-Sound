//! Stream Sound engine: low-latency LAN audio streaming shared by every platform.

pub mod capture;
pub mod discovery;
pub mod engine;
pub mod jitter;
pub mod proto;

pub use capture::{list_sources, Source, SourceInfo, SourceKind};
pub use discovery::Peer;
pub use engine::{Engine, EngineConfig, SenderStats};
pub use jitter::StreamStats;
