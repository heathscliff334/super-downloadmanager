//! A **simulator**, not a download engine: it never opens a socket or writes a file.
//! It exists so the UI can be developed and tested before a real engine is wired in.
//! Demo hook: a URL containing `fail` errors out at ~40% (resume = retry, which succeeds).

use crate::engine::*;
use crate::util::{file_name_from_url, validate_url};
use std::path::PathBuf;
use std::time::Duration;

const MAX_ACTIVE: usize = 3;
const LINK_BPS: f64 = 24.0 * 1024.0 * 1024.0;

struct Item {
    dl: Download,
    fail_at: Option<u64>,
}

pub struct SimulatedEngine {
    items: Vec<Item>,
    next_id: u64,
    limit: Option<u64>,
    clock: f64,
}

impl SimulatedEngine {
    pub fn new() -> Self {
        Self { items: vec![], next_id: 1, limit: None, clock: 0.0 }
    }

    /// Empty-state-free first launch: a few labelled sample rows on reserved `example.com` URLs.
    pub fn with_demo_data(save_dir: PathBuf) -> Self {
        let mut e = Self::new();
        let seed = [
            ("https://example.com/releases/ubuntu-24.04-desktop-amd64.iso", 0.42, DownloadState::Downloading),
            ("https://example.com/media/conference-keynote-4k.mp4", 0.08, DownloadState::Downloading),
            ("https://example.com/data/dataset-2024.tar.gz", 0.0, DownloadState::Queued),
            ("https://example.com/docs/annual-report.pdf", 1.0, DownloadState::Completed),
            ("https://example.com/audio/podcast-episode-212.mp3", 0.67, DownloadState::Paused),
            ("https://example.com/files/fail-demo-installer.exe", 0.0, DownloadState::Queued),
        ];
        for (url, frac, state) in seed {
            let id = e.add(AddRequest { url: url.into(), file_name: None, save_dir: save_dir.clone() }).unwrap();
            let it = e.items.iter_mut().find(|i| i.dl.id == id).unwrap();
            it.dl.downloaded_bytes = (it.dl.total_bytes.unwrap() as f64 * frac) as u64;
            it.dl.state = state;
        }
        e
    }

    fn find(&mut self, id: DownloadId) -> EngineResult<&mut Item> {
        self.items.iter_mut().find(|i| i.dl.id == id).ok_or(EngineError::NotFound(id))
    }
}

impl Default for SimulatedEngine {
    fn default() -> Self {
        Self::new()
    }
}

fn hash(s: &str) -> u64 {
    s.bytes().fold(0xcbf29ce484222325, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3))
}

impl DownloadEngine for SimulatedEngine {
    fn info(&self) -> EngineInfo {
        EngineInfo { name: "Simulated engine".into(), is_real: false }
    }

    fn add(&mut self, req: AddRequest) -> EngineResult<DownloadId> {
        validate_url(&req.url).map_err(|e| EngineError::Rejected(e.to_string()))?;
        let id = DownloadId(self.next_id);
        self.next_id += 1;
        let h = hash(&req.url);
        let total = (8 + h % 800) * 1024 * 1024;
        let file_name = req.file_name.unwrap_or_else(|| file_name_from_url(&req.url));
        self.items.push(Item {
            fail_at: req.url.to_lowercase().contains("fail").then_some(total * 2 / 5),
            dl: Download {
                id,
                url: req.url,
                file_name,
                save_dir: req.save_dir,
                total_bytes: Some(total),
                downloaded_bytes: 0,
                speed_bps: 0,
                state: DownloadState::Queued,
            },
        });
        Ok(id)
    }

    fn pause(&mut self, id: DownloadId) -> EngineResult<()> {
        let it = self.find(id)?;
        if !it.dl.state.is_active() {
            return Err(EngineError::InvalidState("only queued or running downloads can be paused"));
        }
        it.dl.state = DownloadState::Paused;
        it.dl.speed_bps = 0;
        Ok(())
    }

    fn resume(&mut self, id: DownloadId) -> EngineResult<()> {
        let it = self.find(id)?;
        match it.dl.state {
            DownloadState::Paused | DownloadState::Failed(_) => {
                it.fail_at = None;
                it.dl.state = DownloadState::Queued;
                Ok(())
            }
            _ => Err(EngineError::InvalidState("only paused or failed downloads can be resumed")),
        }
    }

    fn remove(&mut self, id: DownloadId) -> EngineResult<()> {
        self.find(id)?;
        self.items.retain(|i| i.dl.id != id);
        Ok(())
    }

    fn set_speed_limit(&mut self, limit_bps: Option<u64>) {
        self.limit = limit_bps;
    }

    fn speed_limit(&self) -> Option<u64> {
        self.limit
    }

    fn tick(&mut self, dt: Duration) {
        let dt = dt.as_secs_f64();
        self.clock += dt;
        let mut running = self.items.iter().filter(|i| i.dl.state == DownloadState::Downloading).count();
        for it in self.items.iter_mut().filter(|i| i.dl.state == DownloadState::Queued) {
            if running >= MAX_ACTIVE {
                break;
            }
            it.dl.state = DownloadState::Downloading;
            running += 1;
        }
        let budget = self.limit.map_or(LINK_BPS, |l| LINK_BPS.min(l as f64));
        let share = budget / running.max(1) as f64;
        for it in self.items.iter_mut().filter(|i| i.dl.state == DownloadState::Downloading) {
            // Jitter stays in [0.6, 1.0] so the global limit is never exceeded.
            let jitter = 0.8 + 0.2 * (self.clock * 1.7 + it.dl.id.0 as f64).sin();
            let speed = share * jitter;
            let total = it.dl.total_bytes.unwrap_or(u64::MAX);
            it.dl.downloaded_bytes = (it.dl.downloaded_bytes + (speed * dt) as u64).min(total);
            it.dl.speed_bps = speed as u64;
            if it.fail_at.is_some_and(|f| it.dl.downloaded_bytes >= f) {
                it.dl.state = DownloadState::Failed("Simulated connection reset".into());
                it.dl.speed_bps = 0;
            } else if it.dl.downloaded_bytes >= total {
                it.dl.state = DownloadState::Completed;
                it.dl.speed_bps = 0;
            }
        }
    }

    fn downloads(&self) -> Vec<Download> {
        self.items.iter().map(|i| i.dl.clone()).collect()
    }
}
