//! Stream Sound engine: low-latency LAN audio streaming shared by every platform.

pub mod capture;
pub mod discovery;
pub mod engine;
pub mod jitter;
pub mod proto;
#[cfg(target_os = "linux")]
mod pulse;
pub mod rt;
pub mod scope;

pub use capture::{list_sources, Source, SourceInfo, SourceKind};
pub use discovery::Peer;
pub use engine::{Engine, EngineConfig, Renderer, SenderStats};
pub use jitter::{Mode, StreamStats};
pub use scope::{Snapshot as ScopeSnapshot, Tap};
