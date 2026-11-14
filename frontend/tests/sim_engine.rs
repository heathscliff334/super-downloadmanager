use download_manager_ui::engine::*;
use download_manager_ui::sim::SimulatedEngine;
use std::time::Duration;

fn req(url: &str) -> AddRequest {
    AddRequest { url: url.into(), file_name: None, save_dir: "/tmp/dl".into() }
}
fn state(e: &SimulatedEngine, id: DownloadId) -> DownloadState {
    e.downloads().into_iter().find(|d| d.id == id).unwrap().state
}
fn run(e: &mut SimulatedEngine, secs: u32) {
    for _ in 0..secs * 10 {
        e.tick(Duration::from_millis(100));
    }
}

#[test]
fn identifies_itself_as_not_real() {
    assert!(!SimulatedEngine::new().info().is_real);
}

#[test]
fn rejects_invalid_urls_and_unknown_ids() {
    let mut e = SimulatedEngine::new();
    assert!(matches!(e.add(req("not a url")), Err(EngineError::Rejected(_))));
    let ghost = DownloadId(99);
    assert_eq!(e.pause(ghost), Err(EngineError::NotFound(ghost)));
    assert_eq!(e.remove(ghost), Err(EngineError::NotFound(ghost)));
}

#[test]
fn queues_beyond_concurrency_limit_and_names_files() {
    let mut e = SimulatedEngine::new();
    for i in 0..5 {
        e.add(req(&format!("https://example.com/f{i}.zip"))).unwrap();
    }
    e.tick(Duration::from_millis(100));
    let d = e.downloads();
    assert_eq!(d.iter().filter(|d| d.state == DownloadState::Downloading).count(), 3);
    assert_eq!(d.iter().filter(|d| d.state == DownloadState::Queued).count(), 2);
    assert_eq!(d[0].file_name, "f0.zip");
}

#[test]
fn pause_resume_transitions() {
    let mut e = SimulatedEngine::new();
    let id = e.add(req("https://example.com/a.bin")).unwrap();
    e.tick(Duration::from_millis(100));
    e.pause(id).unwrap();
    assert_eq!(state(&e, id), DownloadState::Paused);
    let before = e.downloads()[0].downloaded_bytes;
    run(&mut e, 2);
    assert_eq!(e.downloads()[0].downloaded_bytes, before, "paused must not progress");
    assert!(matches!(e.pause(id), Err(EngineError::InvalidState(_))));
    e.resume(id).unwrap();
    assert_eq!(state(&e, id), DownloadState::Queued);
    assert!(matches!(e.resume(id), Err(EngineError::InvalidState(_))));
}

#[test]
fn completes_and_frees_slot_for_queued() {
    let mut e = SimulatedEngine::new();
    let ids: Vec<_> = (0..4).map(|i| e.add(req(&format!("https://example.com/f{i}.iso"))).unwrap()).collect();
    run(&mut e, 600);
    for id in ids {
        assert_eq!(state(&e, id), DownloadState::Completed);
    }
    for d in e.downloads() {
        assert_eq!(Some(d.downloaded_bytes), d.total_bytes);
        assert_eq!(d.speed_bps, 0);
    }
}

#[test]
fn fail_url_fails_then_retry_succeeds() {
    let mut e = SimulatedEngine::new();
    let id = e.add(req("https://example.com/fail.exe")).unwrap();
    run(&mut e, 600);
    assert!(matches!(state(&e, id), DownloadState::Failed(_)));
    e.resume(id).unwrap();
    run(&mut e, 600);
    assert_eq!(state(&e, id), DownloadState::Completed);
}

#[test]
fn speed_limit_is_respected() {
    let mut e = SimulatedEngine::new();
    for i in 0..3 {
        e.add(req(&format!("https://example.com/big{i}.iso"))).unwrap();
    }
    e.set_speed_limit(Some(1 << 20));
    assert_eq!(e.speed_limit(), Some(1 << 20));
    for _ in 0..50 {
        e.tick(Duration::from_millis(100));
        assert!(e.downloads().iter().map(|d| d.speed_bps).sum::<u64>() <= 1 << 20);
    }
}

#[test]
fn remove_drops_item() {
    let mut e = SimulatedEngine::new();
    let id = e.add(req("https://example.com/a.bin")).unwrap();
    e.remove(id).unwrap();
    assert!(e.downloads().is_empty());
}
