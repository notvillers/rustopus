#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod api;
mod cron;
mod scheduler;
mod menubar;

fn octopus_theme() -> egui::Visuals {
    // Palette mirrors the RustOpus docs landing page (src/static/docs/landing.css):
    //   --ko-blue #0038E8 · --ko-blue-deep #002bb4 · --cream #F5F2E9 · --card #FDFCF8
    // The calm deep-blue navies are the dominant surfaces; the vibrant --ko-blue is
    // reserved as an ACCENT (hover / active / selection) so it pops without flooding
    // the window. Cream text, white headings, amber warm signal.
    let well_blue    = egui::Color32::from_rgb(0, 16, 58);    // #00103a — text-field / code wells
    let window_navy  = egui::Color32::from_rgb(0, 26, 94);    // #001a5e — window backdrop
    let widget_navy  = egui::Color32::from_rgb(0, 36, 134);   // #002486 — resting buttons/fields
    let panel_blue   = egui::Color32::from_rgb(0, 43, 180);   // #002bb4 — main panel surface
    let stripe_blue  = egui::Color32::from_rgb(13, 55, 192);  // #0d37c0 — striped-row tint
    let ko_blue      = egui::Color32::from_rgb(0, 56, 232);   // #0038E8 — vibrant accent
    let ko_bright    = egui::Color32::from_rgb(26, 77, 255);  // #1a4dff — active accent
    let cream        = egui::Color32::from_rgb(245, 242, 233); // #F5F2E9 — body text
    let white        = egui::Color32::from_rgb(255, 255, 255); // headings / links
    let subtle_text  = egui::Color32::from_rgb(183, 195, 235); // muted cream-blue
    let subtle_stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(74, 104, 200));
    let cream_stroke  = egui::Stroke::new(1.0_f32, cream);

    let mut v = egui::Visuals::dark();
    v.override_text_color  = Some(cream);
    v.hyperlink_color      = white;
    v.faint_bg_color       = stripe_blue;
    v.extreme_bg_color     = well_blue;
    v.code_bg_color        = well_blue;
    v.warn_fg_color        = egui::Color32::from_rgb(230, 170, 70);
    v.error_fg_color       = egui::Color32::from_rgb(255, 120, 110);
    v.panel_fill           = panel_blue;
    v.window_fill          = window_navy;
    v.window_stroke        = subtle_stroke;

    // Selected tabs / text selection use the vibrant brand blue as the accent.
    v.selection.bg_fill = egui::Color32::from_rgba_unmultiplied(0, 56, 232, 200);
    v.selection.stroke  = cream_stroke;

    v.widgets.noninteractive.bg_fill     = panel_blue;
    v.widgets.noninteractive.weak_bg_fill = panel_blue;
    v.widgets.noninteractive.bg_stroke   = subtle_stroke;
    v.widgets.noninteractive.fg_stroke   = egui::Stroke::new(1.0_f32, subtle_text);

    v.widgets.inactive.bg_fill     = widget_navy;
    v.widgets.inactive.weak_bg_fill = widget_navy;
    v.widgets.inactive.bg_stroke   = subtle_stroke;
    v.widgets.inactive.fg_stroke   = egui::Stroke::new(1.0_f32, cream);

    v.widgets.hovered.bg_fill     = ko_blue;
    v.widgets.hovered.weak_bg_fill = ko_blue;
    v.widgets.hovered.bg_stroke   = cream_stroke;
    v.widgets.hovered.fg_stroke   = egui::Stroke::new(1.5_f32, white);

    v.widgets.active.bg_fill     = ko_bright;
    v.widgets.active.weak_bg_fill = ko_bright;
    v.widgets.active.bg_stroke   = cream_stroke;
    v.widgets.active.fg_stroke   = egui::Stroke::new(2.0_f32, white);

    v.widgets.open.bg_fill     = ko_blue;
    v.widgets.open.weak_bg_fill = ko_blue;
    v.widgets.open.bg_stroke   = cream_stroke;
    v.widgets.open.fg_stroke   = egui::Stroke::new(1.5_f32, white);

    v
}

fn app_icon() -> egui::IconData {
    let bytes = include_bytes!("assets/images/octo_icon.png");
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
            cc.egui_ctx.set_visuals(octopus_theme());
            Ok(Box::new(app::RustopusApp::new(cc)))
        }),
    )
}
