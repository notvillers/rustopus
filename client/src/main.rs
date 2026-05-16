mod app;
mod config;
mod api;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Rustopus Client")
            .with_inner_size([1000.0, 700.0])
            .with_min_inner_size([800.0, 500.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Rustopus Client",
        options,
        Box::new(|cc| Ok(Box::new(app::RustopusApp::new(cc)))),
    )
}
