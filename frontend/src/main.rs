#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use download_manager_ui::{app::DownloadApp, real::RealEngine};

fn main() -> eframe::Result {
    let dir = DownloadApp::default_save_dir();
    let engine = Box::new(RealEngine::new().expect("could not initialize download engine"));
    let options = eframe::NativeOptions {
        // OpenGL works on Linux systems without a Vulkan-capable GPU or driver.
        #[cfg(target_os = "linux")]
        renderer: eframe::Renderer::Glow,
        viewport: egui::ViewportBuilder::default()
            .with_title("Download Manager")
            .with_inner_size([1120.0, 720.0])
            .with_min_inner_size([860.0, 480.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Download Manager",
        options,
        Box::new(move |cc| Ok(Box::new(DownloadApp::new(&cc.egui_ctx, engine, dir)))),
    )
}
