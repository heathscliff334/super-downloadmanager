//! The adapter boundary between the UI and whatever actually moves bytes.
//!
//! The UI only ever talks to `dyn DownloadEngine`. To connect a real engine,
//! implement this trait (e.g. a thin client over the backend's IPC/HTTP API) and
//! pass it to [`crate::app::DownloadApp::new`]. Return `EngineInfo { is_real: true, .. }`
//! and the UI drops its "simulated" warnings.

use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DownloadId(pub u64);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DownloadState {
    Queued,
    Downloading,
    Paused,
    Completed,
    Failed(String),
}

impl DownloadState {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Queued => "Queued",
            Self::Downloading => "Downloading",
            Self::Paused => "Paused",
            Self::Completed => "Completed",
            Self::Failed(_) => "Failed",
        }
    }
    /// Still has work pending (will or may consume bandwidth).
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Queued | Self::Downloading)
    }
}

/// Read-only snapshot of one download, as reported by the engine.
#[derive(Clone, Debug)]
pub struct Download {
    pub id: DownloadId,
    pub url: String,
    pub file_name: String,
    pub save_dir: PathBuf,
    /// `None` when the server did not report a length.
    pub total_bytes: Option<u64>,
    pub downloaded_bytes: u64,
    pub speed_bps: u64,
    pub state: DownloadState,
}

impl Download {
    pub fn progress(&self) -> Option<f32> {
        match (self.state == DownloadState::Completed, self.total_bytes) {
            (true, _) => Some(1.0),
            (_, Some(t)) if t > 0 => {
                Some((self.downloaded_bytes as f64 / t as f64).min(1.0) as f32)
            }
            _ => None,
        }
    }
    pub fn eta(&self) -> Option<Duration> {
        let remaining = self.total_bytes?.saturating_sub(self.downloaded_bytes);
        (self.state == DownloadState::Downloading && self.speed_bps > 0)
            .then(|| Duration::from_secs(remaining / self.speed_bps))
    }
}

#[derive(Clone, Debug)]
pub struct AddRequest {
    pub url: String,
    /// Engine picks a name (e.g. from Content-Disposition/URL) when `None`.
    pub file_name: Option<String>,
    pub save_dir: PathBuf,
}

#[derive(Clone, Debug)]
pub struct EngineInfo {
    pub name: String,
    /// `false` for simulators/mocks: the UI shows a persistent warning.
    pub is_real: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EngineError {
    NotFound(DownloadId),
    InvalidState(&'static str),
    Rejected(String),
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(id) => write!(f, "download #{} no longer exists", id.0),
            Self::InvalidState(m) => f.write_str(m),
            Self::Rejected(m) => f.write_str(m),
        }
    }
}

pub type EngineResult<T> = Result<T, EngineError>;

/// Commands are `&mut self`; `downloads()` is a cheap snapshot the UI polls every frame.
/// Real engines that run on their own threads should keep shared state and
/// return a clone here; `tick` can then be a no-op.
pub trait DownloadEngine {
    fn info(&self) -> EngineInfo;
    fn supports_pause(&self) -> bool {
        true
    }
    fn supports_speed_limit(&self) -> bool {
        true
    }
    fn add(&mut self, req: AddRequest) -> EngineResult<DownloadId>;
    /// Valid from Downloading/Queued (-> Paused).
    fn pause(&mut self, id: DownloadId) -> EngineResult<()>;
    /// Valid from Paused/Failed (-> Queued/Downloading); this is also "retry".
    fn resume(&mut self, id: DownloadId) -> EngineResult<()>;
    /// Removes from the list. Does NOT delete files on disk.
    fn remove(&mut self, id: DownloadId) -> EngineResult<()>;
    /// Global limit in bytes/s; `None` = unlimited.
    fn set_speed_limit(&mut self, limit_bps: Option<u64>);
    fn speed_limit(&self) -> Option<u64>;
    /// Called once per frame with elapsed time. Simulators advance here.
    fn tick(&mut self, dt: Duration);
    fn downloads(&self) -> Vec<Download>;
}
