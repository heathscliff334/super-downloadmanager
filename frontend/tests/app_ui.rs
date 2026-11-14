use download_manager_ui::app::{Command, DownloadApp, Filter};
use download_manager_ui::engine::DownloadState;
use download_manager_ui::sim::SimulatedEngine;
use egui::{RawInput, Rect, pos2};
use std::time::Duration;

fn frame(ctx: &egui::Context, app: &mut DownloadApp) {
    let input = RawInput { screen_rect: Some(Rect::from_min_max(pos2(0.0, 0.0), pos2(1100.0, 700.0))), ..Default::default() };
    ctx.run_ui(input, |ui| app.show(ui)).drop_without_applying_deltas();
}

fn app(ctx: &egui::Context, demo: bool) -> DownloadApp {
    let engine: Box<SimulatedEngine> = if demo {
        Box::new(SimulatedEngine::with_demo_data("/tmp/dl".into()))
    } else {
        Box::new(SimulatedEngine::new())
    };
    DownloadApp::new(ctx, engine, "/tmp/dl".into())
}

#[test]
fn demo_app_renders_many_frames_in_every_filter() {
    let ctx = egui::Context::default();
    let mut app = app(&ctx, true);
    let all = [Filter::All, Filter::Downloading, Filter::Queued, Filter::Paused, Filter::Failed, Filter::Completed];
    for f in all {
        app.filter = f;
        for _ in 0..5 {
            app.tick(Duration::from_millis(100));
            frame(&ctx, &mut app);
        }
    }
    app.selected.insert(app.snapshot[0].id);
    frame(&ctx, &mut app); // details panel
}

#[test]
fn add_links_is_all_or_nothing() {
    let ctx = egui::Context::default();
    let mut app = app(&ctx, false);
    assert!(app.add_links("  \n", "").is_err());
    assert!(app.add_links("https://example.com/a.zip\nbogus", "").is_err());
    assert!(app.snapshot.is_empty());
    assert_eq!(app.add_links("https://example.com/a.zip\n\nhttps://example.com/b.mp4\n", ""), Ok(2));
    assert_eq!(app.snapshot.len(), 2);
    assert_eq!(app.snapshot[0].save_dir.to_str(), Some("/tmp/dl"), "empty dir falls back to default");
}

#[test]
fn commands_update_state_and_prune_selection() {
    let ctx = egui::Context::default();
    let mut app = app(&ctx, false);
    app.add_links("https://example.com/a.zip", "/x").unwrap();
    let id = app.snapshot[0].id;
    app.tick(Duration::from_millis(100));
    app.apply(Command::Pause(vec![id]));
    assert_eq!(app.snapshot[0].state, DownloadState::Paused);
    app.apply(Command::Resume(vec![id]));
    assert_ne!(app.snapshot[0].state, DownloadState::Paused);
    app.selected.insert(id);
    app.apply(Command::Remove(vec![id]));
    assert!(app.snapshot.is_empty() && app.selected.is_empty());
    frame(&ctx, &mut app); // empty state renders
}

#[test]
fn filters_and_search() {
    let ctx = egui::Context::default();
    let app = app(&ctx, true);
    let n = |f: Filter| app.snapshot.iter().filter(|d| f.matches(d)).count();
    assert_eq!(n(Filter::All), app.snapshot.len());
    assert_eq!(n(Filter::Completed), 1);
    assert_eq!(n(Filter::Paused), 1);
    assert_eq!(n(Filter::Kind(download_manager_ui::util::FileKind::Document)), 1);
}
