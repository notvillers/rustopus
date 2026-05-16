use std::sync::mpsc;
use egui::RichText;

use crate::api::{self, Endpoint, EndpointParams};
use crate::config::ClientConfig;

enum FetchResult {
    Ok(String),
    Err(String),
}

pub struct RustopusApp {
    config: ClientConfig,
    selected_endpoint: Endpoint,
    endpoint_params: std::collections::HashMap<String, EndpointParams>,
    response: Option<FetchResult>,
    is_fetching: bool,
    rx: Option<mpsc::Receiver<FetchResult>>,
    status_msg: String,
}

impl RustopusApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let config = ClientConfig::load();
        let mut endpoint_params = std::collections::HashMap::new();
        for ep in Endpoint::all() {
            endpoint_params.insert(ep.label().to_string(), EndpointParams::default());
        }
        Self {
            config,
            selected_endpoint: Endpoint::Products,
            endpoint_params,
            response: None,
            is_fetching: false,
            rx: None,
            status_msg: String::new(),
        }
    }

    fn current_params_mut(&mut self) -> &mut EndpointParams {
        self.endpoint_params
            .get_mut(self.selected_endpoint.label())
            .expect("endpoint params always initialised")
    }

    fn current_params(&self) -> &EndpointParams {
        self.endpoint_params
            .get(self.selected_endpoint.label())
            .expect("endpoint params always initialised")
    }

    fn start_fetch(&mut self, ctx: &egui::Context) {
        let server_url = self.config.server_url.clone();
        let octopus_url = self.config.octopus_url.clone();
        let authcode = self.config.authcode.clone();
        let xmlns = self.config.xmlns.clone();
        let pid = self.config.pid.clone();
        let endpoint = self.selected_endpoint.clone();
        let params = self.current_params().clone();
        let ctx = ctx.clone();

        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        self.is_fetching = true;
        self.status_msg = "Fetching…".to_string();
        self.response = None;

        std::thread::spawn(move || {
            let result = api::fetch(
                &server_url,
                &octopus_url,
                &authcode,
                &xmlns,
                &pid,
                &endpoint,
                &params,
            );
            let msg = match result {
                Ok(body) => FetchResult::Ok(body),
                Err(e) => FetchResult::Err(e),
            };
            let _ = tx.send(msg);
            ctx.request_repaint();
        });
    }

    fn poll_result(&mut self) {
        if let Some(rx) = &self.rx {
            if let Ok(result) = rx.try_recv() {
                self.is_fetching = false;
                self.status_msg = match &result {
                    FetchResult::Ok(_) => "Done.".to_string(),
                    FetchResult::Err(e) => format!("Error: {e}"),
                };
                self.response = Some(result);
                self.rx = None;
            }
        }
    }

    fn save_response(&self) {
        let content = match &self.response {
            Some(FetchResult::Ok(s)) => s.clone(),
            _ => return,
        };

        let params = self.current_params();
        let is_csv = params.data_type.to_lowercase() == "csv";
        let (filter_name, extension) = if is_csv {
            ("CSV files", "csv")
        } else {
            ("XML files", "xml")
        };
        let default_name = format!(
            "{}.{}",
            self.selected_endpoint.label().to_lowercase(),
            extension
        );

        if let Some(path) = rfd::FileDialog::new()
            .set_file_name(&default_name)
            .add_filter(filter_name, &[extension])
            .save_file()
        {
            match std::fs::write(&path, &content) {
                Ok(_) => {}
                Err(e) => eprintln!("Failed to save file: {e}"),
            }
        }
    }
}

impl eframe::App for RustopusApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_result();

        // ── Left panel: connection settings ──────────────────────────────
        egui::SidePanel::left("connection_panel")
            .resizable(false)
            .min_width(260.0)
            .show(ctx, |ui| {
                ui.heading(
                    egui::RichText::new("Connection")
                        .color(egui::Color32::from_rgb(206, 66, 43)),
                );
                ui.separator();

                egui::Grid::new("conn_grid")
                    .num_columns(2)
                    .spacing([8.0, 6.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label("Server URL:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.config.server_url)
                                .desired_width(180.0),
                        );
                        ui.end_row();

                        ui.label("Octopus URL:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.config.octopus_url)
                                .desired_width(180.0),
                        );
                        ui.end_row();

                        ui.label("Authcode:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.config.authcode)
                                .desired_width(180.0),
                        );
                        ui.end_row();

                        ui.label("xmlns (optional):");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.config.xmlns)
                                .desired_width(180.0),
                        );
                        ui.end_row();

                        ui.label("Partner ID (pid):");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.config.pid)
                                .desired_width(180.0),
                        );
                        ui.end_row();
                    });

                ui.add_space(8.0);
                if ui.button("💾  Save settings").clicked() {
                    self.config.save();
                    self.status_msg = "Settings saved.".to_string();
                }
            });

        // ── Bottom status bar ─────────────────────────────────────────────
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if self.is_fetching {
                    ui.spinner();
                }
                ui.label(RichText::new(&self.status_msg).small());
            });
        });

        // ── Central panel ─────────────────────────────────────────────────
        egui::CentralPanel::default().show(ctx, |ui| {
            // Endpoint tabs
            ui.horizontal(|ui| {
                for ep in Endpoint::all() {
                    let selected = self.selected_endpoint == ep;
                    if ui
                        .selectable_label(selected, ep.label())
                        .clicked()
                    {
                        self.selected_endpoint = ep;
                    }
                }
            });
            ui.separator();

            // Per-endpoint parameters form
            let ep = self.selected_endpoint.clone();

            egui::Grid::new("params_grid")
                .num_columns(2)
                .spacing([8.0, 6.0])
                .show(ui, |ui| {
                    if ep.has_from_date() {
                        ui.label("From date:");
                        let params = self.current_params_mut();
                        ui.add(
                            egui::TextEdit::singleline(&mut params.from_date)
                                .hint_text("2024-01-01T00:00:00Z")
                                .desired_width(240.0),
                        );
                        ui.end_row();
                    }

                    if ep.has_to_date() {
                        ui.label("To date:");
                        let params = self.current_params_mut();
                        ui.add(
                            egui::TextEdit::singleline(&mut params.to_date)
                                .hint_text("2024-12-31T23:59:59Z")
                                .desired_width(240.0),
                        );
                        ui.end_row();
                    }

                    if ep.has_language() {
                        ui.label("Language:");
                        let params = self.current_params_mut();
                        egui::ComboBox::from_id_salt("lang_combo")
                            .selected_text(if params.language.is_empty() {
                                "English (default)"
                            } else {
                                &params.language
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut params.language,
                                    String::new(),
                                    "English (default)",
                                );
                                ui.selectable_value(
                                    &mut params.language,
                                    "hu".to_string(),
                                    "Hungarian (hu)",
                                );
                            });
                        ui.end_row();
                    }

                    if ep.has_data_type() {
                        ui.label("Format:");
                        let params = self.current_params_mut();
                        egui::ComboBox::from_id_salt("format_combo")
                            .selected_text(if params.data_type.is_empty() {
                                "XML"
                            } else {
                                "CSV"
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut params.data_type,
                                    String::new(),
                                    "XML",
                                );
                                ui.selectable_value(
                                    &mut params.data_type,
                                    "csv".to_string(),
                                    "CSV",
                                );
                            });
                        ui.end_row();
                    }

                    if ep.has_type_mod() {
                        ui.label("Type mod:");
                        let params = self.current_params_mut();
                        ui.add(
                            egui::TextEdit::singleline(&mut params.type_mod)
                                .hint_text("1 (default)")
                                .desired_width(100.0),
                        );
                        ui.end_row();
                    }

                    if ep.has_unpaid() {
                        ui.label("Unpaid:");
                        let params = self.current_params_mut();
                        egui::ComboBox::from_id_salt("unpaid_combo")
                            .selected_text(if params.unpaid == "1" { "Yes" } else { "No (default)" })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut params.unpaid,
                                    "0".to_string(),
                                    "No (default)",
                                );
                                ui.selectable_value(
                                    &mut params.unpaid,
                                    "1".to_string(),
                                    "Yes",
                                );
                            });
                        ui.end_row();
                    }
                });

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let fetch_btn = ui.add_enabled(
                    !self.is_fetching,
                    egui::Button::new("▶  Fetch"),
                );
                if fetch_btn.clicked() {
                    self.start_fetch(ctx);
                }

                let has_response = matches!(&self.response, Some(FetchResult::Ok(_)));
                let save_btn = ui.add_enabled(has_response, egui::Button::new("💾  Save to file"));
                if save_btn.clicked() {
                    self.save_response();
                }

                if self.response.is_some() && ui.button("🗑  Clear").clicked() {
                    self.response = None;
                    self.status_msg = String::new();
                }
            });

            ui.separator();

            // Response area
            let (response_text, truncated) = match &self.response {
                Some(FetchResult::Ok(s)) | Some(FetchResult::Err(s)) => {
                    let lines: Vec<&str> = s.lines().collect();
                    if lines.len() > 100 {
                        (lines[..100].join("\n"), true)
                    } else {
                        (s.clone(), false)
                    }
                }
                None => (String::new(), false),
            };
            let is_error = matches!(&self.response, Some(FetchResult::Err(_)));

            if truncated {
                ui.label(
                    egui::RichText::new("⚠ Showing first 100 lines. Save to file to see the full response.")
                        .small()
                        .color(egui::Color32::from_rgb(206, 66, 43)),
                );
            }

            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let color = if is_error {
                        egui::Color32::from_rgb(220, 80, 80)
                    } else {
                        ui.visuals().text_color()
                    };
                    ui.add(
                        egui::TextEdit::multiline(&mut response_text.clone())
                            .desired_width(f32::INFINITY)
                            .desired_rows(20)
                            .font(egui::TextStyle::Monospace)
                            .text_color(color)
                            .interactive(false),
                    );
                });
        });
    }
}
