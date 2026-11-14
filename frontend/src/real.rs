//! Adapter for the local asynchronous HTTP engine.
use crate::engine::*;
use crate::util::{file_name_from_url, is_windows_reserved_name};
use download_manager::{DownloadHandle, DownloadManager, DownloadRequest, DownloadStatus};
use std::time::Duration;

struct Job {
    handle: DownloadHandle,
    speed: u64,
    previous: u64,
}

pub struct RealEngine {
    runtime: tokio::runtime::Runtime,
    manager: DownloadManager,
    jobs: Vec<Job>,
}
impl RealEngine {
    pub fn new() -> Result<Self, String> {
        let runtime = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
        let manager = DownloadManager::new(3).map_err(str::to_string)?;
        Ok(Self {
            runtime,
            manager,
            jobs: Vec::new(),
        })
    }
    fn find(&self, id: DownloadId) -> EngineResult<usize> {
        self.jobs
            .iter()
            .position(|j| j.handle.snapshot().id.0 == id.0)
            .ok_or(EngineError::NotFound(id))
    }
}
impl DownloadEngine for RealEngine {
    fn info(&self) -> EngineInfo {
        EngineInfo {
            name: "HTTP engine".into(),
            is_real: true,
        }
    }
    fn supports_pause(&self) -> bool {
        false
    }
    fn supports_speed_limit(&self) -> bool {
        false
    }
    fn add(&mut self, req: AddRequest) -> EngineResult<DownloadId> {
        let url = url::Url::parse(&req.url).map_err(|e| EngineError::Rejected(e.to_string()))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(EngineError::Rejected(
                "Only HTTP and HTTPS links are supported".into(),
            ));
        }
        if !req.save_dir.is_dir() {
            return Err(EngineError::Rejected("Save folder does not exist".into()));
        }
        let name = req
            .file_name
            .unwrap_or_else(|| file_name_from_url(&req.url));
        if std::path::Path::new(&name).components().count() != 1
            || name == "."
            || name == ".."
            || name.contains(['/', '\\'])
            || name.chars().any(|c| c.is_control() || "\\:*?\"<>|".contains(c))
            || name.ends_with([' ', '.'])
            || is_windows_reserved_name(&name)
        {
            return Err(EngineError::Rejected("Invalid file name".into()));
        }
        let _guard = self.runtime.enter();
        let handle = self.manager.enqueue(DownloadRequest {
            url,
            destination: req.save_dir.join(name),
        });
        let id = DownloadId(handle.snapshot().id.0);
        self.jobs.push(Job {
            handle,
            speed: 0,
            previous: 0,
        });
        Ok(id)
    }
    fn pause(&mut self, _: DownloadId) -> EngineResult<()> {
        Err(EngineError::InvalidState("Pause is not supported yet"))
    }
    fn resume(&mut self, id: DownloadId) -> EngineResult<()> {
        let i = self.find(id)?;
        let snapshot = self.jobs[i].handle.snapshot();
        if !matches!(snapshot.status, DownloadStatus::Failed(_)) {
            return Err(EngineError::InvalidState(
                "Only failed downloads can be retried",
            ));
        }
        let _guard = self.runtime.enter();
        self.jobs[i] = Job {
            handle: self.manager.enqueue(snapshot.request),
            speed: 0,
            previous: 0,
        };
        Ok(())
    }
    fn remove(&mut self, id: DownloadId) -> EngineResult<()> {
        let i = self.find(id)?;
        let job = self.jobs.remove(i);
        job.handle.cancel();
        Ok(())
    }
    fn set_speed_limit(&mut self, _: Option<u64>) {}
    fn speed_limit(&self) -> Option<u64> {
        None
    }
    fn tick(&mut self, dt: Duration) {
        if dt.is_zero() {
            return;
        }
        for job in &mut self.jobs {
            let snapshot = job.handle.snapshot();
            job.speed = if snapshot.status == DownloadStatus::Downloading {
                (snapshot.downloaded_bytes.saturating_sub(job.previous) as f64 / dt.as_secs_f64())
                    as u64
            } else {
                0
            };
            job.previous = snapshot.downloaded_bytes;
        }
    }
    fn downloads(&self) -> Vec<Download> {
        self.jobs
            .iter()
            .map(|j| {
                let s = j.handle.snapshot();
                let state = match s.status {
                    DownloadStatus::Queued => DownloadState::Queued,
                    DownloadStatus::Downloading => DownloadState::Downloading,
                    DownloadStatus::Completed => DownloadState::Completed,
                    DownloadStatus::Cancelled => DownloadState::Failed("Cancelled".into()),
                    DownloadStatus::Failed(e) => DownloadState::Failed(e),
                };
                Download {
                    id: DownloadId(s.id.0),
                    url: s.request.url.to_string(),
                    file_name: s
                        .request
                        .destination
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    save_dir: s
                        .request
                        .destination
                        .parent()
                        .unwrap_or(std::path::Path::new("."))
                        .to_path_buf(),
                    total_bytes: s.total_bytes,
                    downloaded_bytes: s.downloaded_bytes,
                    speed_bps: j.speed,
                    state,
                }
            })
            .collect()
    }
}
