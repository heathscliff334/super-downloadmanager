//! An asynchronous HTTP download engine intended to be driven by a desktop UI.
//!
//! Create a [`DownloadManager`] inside a Tokio runtime, enqueue requests, and keep
//! the returned handles. A UI can poll [`DownloadHandle::snapshot`] each frame or
//! subscribe to changes with [`DownloadHandle::subscribe`]. The runtime must stay
//! alive until downloads finish. Dropping a handle does not cancel its download.
//!
//! Each transfer writes to a private temporary file in the destination directory.
//! The completed file is installed only if the destination does not exist. Failed
//! and cancelled downloads are discarded. This engine does not resume transfers.

use futures_util::StreamExt;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    io::AsyncWriteExt,
    sync::{watch, Semaphore},
};
use tokio_util::sync::CancellationToken;
use url::Url;

/// Identifies a job within one manager instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DownloadId(pub u64);

/// Request for a single GET. The output path is explicit; server-provided filenames
/// are never trusted. Its parent directory must already exist.
#[derive(Debug, Clone)]
pub struct DownloadRequest {
    pub url: Url,
    pub destination: PathBuf,
}

/// A job's lifecycle state. Failure messages are suitable for display, not parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadStatus {
    Queued,
    Downloading,
    Completed,
    Cancelled,
    Failed(String),
}

impl DownloadStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled | Self::Failed(_))
    }
}

/// Latest state of a job. `total_bytes` is unknown when the server omits length.
#[derive(Debug, Clone)]
pub struct DownloadSnapshot {
    pub id: DownloadId,
    pub request: DownloadRequest,
    pub status: DownloadStatus,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
}

/// Cloneable handle to a job. Watch receivers coalesce intermediate progress updates.
#[derive(Clone)]
pub struct DownloadHandle {
    state: watch::Receiver<DownloadSnapshot>,
    cancel: CancellationToken,
}

impl DownloadHandle {
    pub fn snapshot(&self) -> DownloadSnapshot {
        self.state.borrow().clone()
    }
    pub fn subscribe(&self) -> watch::Receiver<DownloadSnapshot> {
        self.state.clone()
    }
    /// Request cancellation, including while queued. Completion can win a race with cancellation.
    pub fn cancel(&self) {
        self.cancel.cancel();
    }
    /// Wait for a terminal snapshot. Multiple callers may wait independently.
    pub async fn wait(&self) -> DownloadSnapshot {
        let mut state = self.subscribe();
        loop {
            let snapshot = state.borrow_and_update().clone();
            if snapshot.status.is_terminal() {
                return snapshot;
            }
            if state.changed().await.is_err() {
                return state.borrow().clone();
            }
        }
    }
}

/// Owns shared HTTP and concurrency settings. Jobs continue if the manager is dropped.
#[derive(Clone)]
pub struct DownloadManager {
    client: reqwest::Client,
    slots: Arc<Semaphore>,
    next_id: Arc<AtomicU64>,
}

impl DownloadManager {
    /// Constructs a manager with a positive maximum number of active transfers.
    pub fn new(max_concurrent: usize) -> Result<Self, &'static str> {
        if max_concurrent == 0 || max_concurrent > Semaphore::MAX_PERMITS {
            return Err("max_concurrent must be between 1 and Tokio's semaphore limit");
        }
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(30))
            .build()
            .map_err(|_| "failed to build HTTP client")?;
        Ok(Self {
            client,
            slots: Arc::new(Semaphore::new(max_concurrent)),
            next_id: Arc::new(AtomicU64::new(1)),
        })
    }

    /// Enqueues a job on the current Tokio runtime. Validation failures appear in
    /// its terminal snapshot, just like network or filesystem failures.
    pub fn enqueue(&self, request: DownloadRequest) -> DownloadHandle {
        let id = DownloadId(self.next_id.fetch_add(1, Ordering::Relaxed));
        let (state, receiver) = watch::channel(DownloadSnapshot {
            id,
            request,
            status: DownloadStatus::Queued,
            downloaded_bytes: 0,
            total_bytes: None,
        });
        let cancel = CancellationToken::new();
        let handle = DownloadHandle {
            state: receiver,
            cancel: cancel.clone(),
        };
        let manager = self.clone();
        tokio::spawn(async move {
            let result = manager.run(&state, &cancel).await;
            state.send_modify(|snapshot| {
                snapshot.status = match result {
                    Ok(()) => DownloadStatus::Completed,
                    Err(JobError::Cancelled) => DownloadStatus::Cancelled,
                    Err(JobError::Failed(message)) => DownloadStatus::Failed(message),
                };
            });
        });
        handle
    }

    async fn run(
        &self,
        state: &watch::Sender<DownloadSnapshot>,
        cancel: &CancellationToken,
    ) -> Result<(), JobError> {
        let request = state.borrow().request.clone();
        if !matches!(request.url.scheme(), "http" | "https") {
            return Err(JobError::Failed(
                "only HTTP and HTTPS URLs are supported".into(),
            ));
        }
        let name = request
            .destination
            .file_name()
            .ok_or_else(|| JobError::Failed("destination must name a file".into()))?;
        if name.is_empty() {
            return Err(JobError::Failed("destination must name a file".into()));
        }
        let parent = request
            .destination
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| std::path::Path::new("."));
        let _permit = tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(JobError::Cancelled),
            permit = self.slots.acquire() => permit.map_err(|e| JobError::Failed(e.to_string()))?,
        };
        state.send_modify(|s| s.status = DownloadStatus::Downloading);
        let response = tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(JobError::Cancelled),
            result = self.client.get(request.url.clone()).send() => result.map_err(failed)?,
        };
        if !response.status().is_success()
            || response.status() == reqwest::StatusCode::PARTIAL_CONTENT
        {
            return Err(JobError::Failed(format!(
                "unexpected HTTP status {}",
                response.status()
            )));
        }
        let total = response.content_length();
        state.send_modify(|s| s.total_bytes = total);
        let temp = tempfile::Builder::new()
            .prefix(".download-")
            .tempfile_in(parent)
            .map_err(failed)?;
        let mut file = tokio::fs::File::from_std(temp.reopen().map_err(failed)?);
        let mut stream = response.bytes_stream();
        loop {
            let chunk = tokio::select! {
                biased;
                _ = cancel.cancelled() => return Err(JobError::Cancelled),
                chunk = stream.next() => chunk,
            };
            let Some(chunk) = chunk else { break };
            let chunk = chunk.map_err(failed)?;
            tokio::select! {
                biased;
                _ = cancel.cancelled() => return Err(JobError::Cancelled),
                result = file.write_all(&chunk) => result.map_err(failed)?,
            }
            state.send_modify(|s| s.downloaded_bytes += chunk.len() as u64);
        }
        if total.is_some_and(|n| state.borrow().downloaded_bytes != n) {
            return Err(JobError::Failed(
                "response length did not match Content-Length".into(),
            ));
        }
        tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(JobError::Cancelled),
            result = file.sync_all() => result.map_err(failed)?,
        }
        drop(file);
        if cancel.is_cancelled() {
            return Err(JobError::Cancelled);
        }
        temp.persist_noclobber(&request.destination)
            .map_err(failed)?;
        Ok(())
    }
}

#[derive(Debug)]
enum JobError {
    Cancelled,
    Failed(String),
}
fn failed(error: impl std::fmt::Display) -> JobError {
    JobError::Failed(error.to_string())
}
