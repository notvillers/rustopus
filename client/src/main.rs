#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod api;
mod cron;
mod scheduler;
mod menubar;

fn rust_theme() -> egui::Visuals {
    // Rust brand orange: #CE422B
    let rust_orange   = egui::Color32::from_rgb(206, 66, 43);
    let panel_bg      = egui::Color32::from_rgb(36, 28, 26);
    let window_bg     = egui::Color32::from_rgb(28, 22, 20);
    let widget_bg     = egui::Color32::from_rgb(50, 40, 37);
    let widget_hover  = egui::Color32::from_rgb(66, 52, 48);
    let widget_active = egui::Color32::from_rgb(82, 60, 54);
    let text_color    = egui::Color32::from_rgb(230, 215, 205);
    let subtle_text   = egui::Color32::from_rgb(155, 135, 125);
    let subtle_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(75, 58, 53));
    let rust_stroke   = egui::Stroke::new(1.0, rust_orange);

    let mut v = egui::Visuals::dark();
    v.override_text_color  = Some(text_color);
    v.hyperlink_color      = rust_orange;
    v.faint_bg_color       = egui::Color32::from_rgb(42, 33, 30);
    v.extreme_bg_color     = egui::Color32::from_rgb(18, 14, 12);
    v.code_bg_color        = egui::Color32::from_rgb(30, 24, 22);
    v.warn_fg_color        = egui::Color32::from_rgb(220, 140, 60);
    v.error_fg_color       = egui::Color32::from_rgb(230, 80, 60);
    v.panel_fill           = panel_bg;
    v.window_fill          = window_bg;
    v.window_stroke        = subtle_stroke;

    v.selection.bg_fill = egui::Color32::from_rgba_unmultiplied(206, 66, 43, 110);
    v.selection.stroke  = rust_stroke;

    v.widgets.noninteractive.bg_fill     = panel_bg;
    v.widgets.noninteractive.weak_bg_fill = panel_bg;
    v.widgets.noninteractive.bg_stroke   = subtle_stroke;
    v.widgets.noninteractive.fg_stroke   = egui::Stroke::new(1.0, subtle_text);

    v.widgets.inactive.bg_fill     = widget_bg;
    v.widgets.inactive.weak_bg_fill = widget_bg;
    v.widgets.inactive.bg_stroke   = subtle_stroke;
    v.widgets.inactive.fg_stroke   = egui::Stroke::new(1.0, text_color);

    v.widgets.hovered.bg_fill     = widget_hover;
    v.widgets.hovered.weak_bg_fill = widget_hover;
    v.widgets.hovered.bg_stroke   = rust_stroke;
    v.widgets.hovered.fg_stroke   = egui::Stroke::new(1.5, rust_orange);

    v.widgets.active.bg_fill     = widget_active;
    v.widgets.active.weak_bg_fill = widget_active;
    v.widgets.active.bg_stroke   = rust_stroke;
    v.widgets.active.fg_stroke   = egui::Stroke::new(2.0, rust_orange);

    v.widgets.open.bg_fill     = widget_hover;
    v.widgets.open.weak_bg_fill = widget_hover;
    v.widgets.open.bg_stroke   = rust_stroke;
    v.widgets.open.fg_stroke   = egui::Stroke::new(1.5, rust_orange);

    v
}

fn app_icon() -> egui::IconData {
    let bytes = include_bytes!("assets/images/octopus.png");
    let img = image::load_from_memory(bytes)
        .expect("valid PNG icon")
        .into_rgba8();
    let (w, h) = img.dimensions();
    egui::IconData {
        rgba: img.into_raw(),
        width: w,
        height: h,
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Rustopus Client")
            .with_inner_size([1250.0, 700.0])
            .with_min_inner_size([900.0, 500.0])
            .with_icon(app_icon()),
        ..Default::default()
    };

    eframe::run_native(
        "Rustopus",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(rust_theme());
            Ok(Box::new(app::RustopusApp::new(cc)))
        }),
    )
}
