use std::sync::{Arc, Mutex, mpsc};
use egui::RichText;

use crate::api::{self, Endpoint, EndpointParams};
use crate::config::ClientConfig;
use crate::cron::{CronConfig, CronJob, IntervalUnit};
use crate::scheduler::{self, SchedulerResult};

enum FetchResult {
    Ok(String),
    Err(String),
}

#[derive(PartialEq)]
enum AppTab {
    Fetch,
    Crons,
}

/// Which cron form state is active.
enum CronFormState {
    Hidden,
    Adding,
    Editing(usize),
}

pub struct RustopusApp {
    // ── Shared state ───────────────────────────────────────────────────────
    config: ClientConfig,
    config_arc: Arc<Mutex<ClientConfig>>,

    // ── Fetch tab ──────────────────────────────────────────────────────────
    selected_endpoint: Endpoint,
    endpoint_params: std::collections::HashMap<String, EndpointParams>,
    response: Option<FetchResult>,
    is_fetching: bool,
    rx: Option<mpsc::Receiver<FetchResult>>,
    status_msg: String,

    // ── Tab ────────────────────────────────────────────────────────────────
    active_tab: AppTab,

    // ── Crons tab ──────────────────────────────────────────────────────────
    cron_jobs: Arc<Mutex<Vec<CronJob>>>,
    sched_tx: mpsc::Sender<SchedulerResult>,
    sched_rx: mpsc::Receiver<SchedulerResult>,
    cron_form: CronFormState,
    cron_draft: CronJob,
    cron_status_msg: String,
}

impl RustopusApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let config = ClientConfig::load();
        let config_arc = Arc::new(Mutex::new(config.clone()));

        let mut endpoint_params = std::collections::HashMap::new();
        for ep in Endpoint::all() {
            endpoint_params.insert(ep.label().to_string(), EndpointParams::default());
        }

        let cron_cfg = CronConfig::load();
        let cron_jobs = Arc::new(Mutex::new(cron_cfg.jobs));

        let (sched_tx, sched_rx) = mpsc::channel();
        scheduler::start(
            Arc::clone(&cron_jobs),
            Arc::clone(&config_arc),
            sched_tx.clone(),
            cc.egui_ctx.clone(),
        );

        let draft = CronJob::new(String::new(), Endpoint::Products.label().to_string());

        Self {
            config,
            config_arc,
            selected_endpoint: Endpoint::Products,
            endpoint_params,
            response: None,
            is_fetching: false,
            rx: None,
            status_msg: String::new(),
            active_tab: AppTab::Fetch,
            cron_jobs,
            sched_tx,
            sched_rx,
            cron_form: CronFormState::Hidden,
            cron_draft: draft,
            cron_status_msg: String::new(),
        }
    }

    // ── Fetch-tab helpers ──────────────────────────────────────────────────

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
        if let Some(rx) = &self.rx
            && let Ok(result) = rx.try_recv() {
                self.is_fetching = false;
                self.status_msg = match &result {
                    FetchResult::Ok(_) => "Done.".to_string(),
                    FetchResult::Err(e) => format!("Error: {e}"),
                };
                self.response = Some(result);
                self.rx = None;
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

    // ── Cron helpers ───────────────────────────────────────────────────────

    fn poll_scheduler(&mut self) {
        while let Ok(result) = self.sched_rx.try_recv() {
            self.cron_status_msg = format!(
                "[{}] {} — {}",
                result.ran_at
                    .get(..16)
                    .unwrap_or(&result.ran_at),
                result.job_name,
                result.status
            );
        }
        // Persist any last_run/last_status updates written by scheduler threads.
        let jobs = self.cron_jobs.lock().unwrap().clone();
        CronConfig { jobs }.save();
    }

    fn save_cron_jobs(&self) {
        let jobs = self.cron_jobs.lock().unwrap().clone();
        CronConfig { jobs }.save();
    }

    fn reset_draft(&mut self) {
        self.cron_draft = CronJob::new(String::new(), Endpoint::Products.label().to_string());
    }

    // ── Cron tab UI ────────────────────────────────────────────────────────

    fn show_crons_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        // Toolbar
        ui.horizontal(|ui| {
            if ui.button("➕  Add job").clicked() {
                self.reset_draft();
                self.cron_form = CronFormState::Adding;
            }
            ui.separator();
            ui.label(RichText::new(&self.cron_status_msg).small());
        });
        ui.separator();

        // Job list
        let job_count = self.cron_jobs.lock().unwrap().len();

        if job_count == 0 {
            ui.label(RichText::new("No cron jobs configured yet.").italics());
        } else {
            // Tick the countdown every second while the Crons tab is open.
            ctx.request_repaint_after(std::time::Duration::from_secs(1));

            let mut action: Option<CronAction> = None;

            egui::ScrollArea::vertical()
                .max_height(300.0)
                .show(ui, |ui| {
                    egui::Grid::new("cron_list")
                        .num_columns(9)
                        .spacing([6.0, 4.0])
                        .striped(true)
                        .show(ui, |ui| {
                            // Header
                            ui.label(RichText::new("Name").strong());
                            ui.label(RichText::new("Endpoint").strong());
                            ui.label(RichText::new("Interval").strong());
                            //ui.label(RichText::new("Output path").strong());
                            ui.label(RichText::new("On").strong());
                            ui.label(RichText::new("Last run").strong());
                            ui.label(RichText::new("Next run").strong());
                            ui.label(RichText::new("Status").strong());
                            ui.label("");
                            ui.end_row();

                            let jobs = self.cron_jobs.lock().unwrap().clone();
                            for (i, job) in jobs.iter().enumerate() {
                                ui.label(&job.name);
                                ui.label(&job.endpoint);
                                ui.label(format!(
                                    "{} {}",
                                    job.interval_value,
                                    job.interval_unit.label()
                                ));
                                // let path_str = std::path::PathBuf::from(&job.output_dir)
                                //     .join(&job.output_filename)
                                //     .display()
                                //     .to_string();
                                //ui.label(RichText::new(&path_str).small());

                                let mut enabled = job.enabled;
                                if ui.checkbox(&mut enabled, "").changed() {
                                    action = Some(CronAction::Toggle(i, enabled));
                                }

                                let last_run_text = job
                                    .last_run
                                    .as_deref()
                                    .and_then(|s| s.parse::<chrono::DateTime<chrono::Utc>>().ok())
                                    .map(|utc| {
                                        chrono::DateTime::<chrono::Local>::from(utc)
                                            .format("%Y-%m-%d %H:%M")
                                            .to_string()
                                    })
                                    .unwrap_or_else(|| "never".to_string());
                                ui.label(RichText::new(&last_run_text).small());

                                // Next-run countdown
                                let next_label = job.next_run_label();
                                let next_color = if next_label == "now" {
                                    egui::Color32::from_rgb(220, 140, 60)
                                } else {
                                    ui.visuals().text_color()
                                };
                                ui.label(RichText::new(&next_label).small().color(next_color));

                                let status_text = job.last_status.as_deref().unwrap_or("—");
                                let status_color =
                                    if status_text.starts_with("Error") || status_text.contains("failed") {
                                        egui::Color32::from_rgb(220, 80, 80)
                                    } else if status_text.starts_with("✔") {
                                        egui::Color32::from_rgb(100, 200, 100)
                                    } else {
                                        ui.visuals().text_color()
                                    };
                                ui.label(
                                    RichText::new(status_text)
                                        .small()
                                        .color(status_color),
                                );

                                ui.horizontal(|ui| {
                                    if ui.small_button("▶").on_hover_text("Run now").clicked() {
                                        action = Some(CronAction::RunNow(i));
                                    }
                                    if ui.small_button("✏").on_hover_text("Edit").clicked() {
                                        action = Some(CronAction::Edit(i));
                                    }
                                    let delete_btn = egui::Button::new(
                                        RichText::new("🗑 Delete")
                                            .small()
                                            .color(egui::Color32::from_rgb(220, 80, 80)),
                                    );
                                    if ui.add(delete_btn).clicked() {
                                        action = Some(CronAction::Delete(i));
                                    }
                                });
                                ui.end_row();
                            }
                        });
                });

            // Apply actions after borrow released
            if let Some(act) = action {
                match act {
                    CronAction::Toggle(i, val) => {
                        self.cron_jobs.lock().unwrap()[i].enabled = val;
                        self.save_cron_jobs();
                    }
                    CronAction::RunNow(i) => {
                        scheduler::run_job_now(
                            i,
                            &self.cron_jobs,
                            &self.config_arc,
                            &self.sched_tx,
                            ctx,
                        );
                    }
                    CronAction::Edit(i) => {
                        let job = self.cron_jobs.lock().unwrap()[i].clone();
                        self.cron_draft = job;
                        self.cron_form = CronFormState::Editing(i);
                    }
                    CronAction::Delete(i) => {
                        self.cron_jobs.lock().unwrap().remove(i);
                        // Reset form if we were editing the deleted job.
                        if matches!(self.cron_form, CronFormState::Editing(j) if j == i) {
                            self.cron_form = CronFormState::Hidden;
                        }
                        self.save_cron_jobs();
                    }
                }
            }
        }

        // Add / edit form
        if !matches!(self.cron_form, CronFormState::Hidden) {
            ui.separator();
            let is_adding = matches!(self.cron_form, CronFormState::Adding);
            ui.heading(if is_adding { "Add cron job" } else { "Edit cron job" });

            egui::Grid::new("cron_form_grid")
                .num_columns(2)
                .spacing([8.0, 6.0])
                .show(ui, |ui| {
                    ui.label("Name:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.cron_draft.name)
                            .desired_width(220.0),
                    );
                    ui.end_row();

                    ui.label("Endpoint:");
                    egui::ComboBox::from_id_salt("cron_ep_combo")
                        .selected_text(&self.cron_draft.endpoint)
                        .show_ui(ui, |ui| {
                            for ep in Endpoint::all() {
                                ui.selectable_value(
                                    &mut self.cron_draft.endpoint,
                                    ep.label().to_string(),
                                    ep.label(),
                                );
                            }
                        });
                    ui.end_row();

                    // Show relevant endpoint params inline
                    let ep = Endpoint::from_label(&self.cron_draft.endpoint)
                        .unwrap_or(Endpoint::Products);

                    if ep.has_from_date() {
                        ui.label("From date:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.cron_draft.params.from_date)
                                .hint_text("2024-01-01T00:00:00Z")
                                .desired_width(220.0),
                        );
                        ui.end_row();
                    }
                    if ep.has_to_date() {
                        ui.label("To date:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.cron_draft.params.to_date)
                                .hint_text("2024-12-31T23:59:59Z")
                                .desired_width(220.0),
                        );
                        ui.end_row();
                    }
                    if ep.has_language() {
                        ui.label("Language:");
                        egui::ComboBox::from_id_salt("cron_lang_combo")
                            .selected_text(if self.cron_draft.params.language.is_empty() {
                                "English (default)"
                            } else {
                                &self.cron_draft.params.language
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.cron_draft.params.language,
                                    String::new(),
                                    "English (default)",
                                );
                                ui.selectable_value(
                                    &mut self.cron_draft.params.language,
                                    "hu".to_string(),
                                    "Hungarian (hu)",
                                );
                            });
                        ui.end_row();
                    }
                    if ep.has_data_type() {
                        ui.label("Format:");
                        let mut fmt_changed = false;
                        egui::ComboBox::from_id_salt("cron_fmt_combo")
                            .selected_text(if self.cron_draft.params.data_type.is_empty() {
                                "XML"
                            } else {
                                "CSV"
                            })
                            .show_ui(ui, |ui| {
                                fmt_changed |= ui
                                    .selectable_value(
                                        &mut self.cron_draft.params.data_type,
                                        String::new(),
                                        "XML",
                                    )
                                    .changed();
                                fmt_changed |= ui
                                    .selectable_value(
                                        &mut self.cron_draft.params.data_type,
                                        "csv".to_string(),
                                        "CSV",
                                    )
                                    .changed();
                            });
                        if fmt_changed {
                            self.cron_draft.sync_filename_extension();
                        }
                        ui.end_row();
                    }
                    if ep.has_type_mod() {
                        ui.label("Type mod:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.cron_draft.params.type_mod)
                                .hint_text("1 (default)")
                                .desired_width(100.0),
                        );
                        ui.end_row();
                    }
                    if ep.has_unpaid() {
                        ui.label("Unpaid:");
                        egui::ComboBox::from_id_salt("cron_unpaid_combo")
                            .selected_text(if self.cron_draft.params.unpaid == "1" {
                                "Yes"
                            } else {
                                "No (default)"
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.cron_draft.params.unpaid,
                                    "0".to_string(),
                                    "No (default)",
                                );
                                ui.selectable_value(
                                    &mut self.cron_draft.params.unpaid,
                                    "1".to_string(),
                                    "Yes",
                                );
                            });
                        ui.end_row();
                    }

                    ui.label("Interval:");
                    ui.horizontal(|ui| {
                        let mut val_str = self.cron_draft.interval_value.to_string();
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut val_str)
                                    .desired_width(60.0),
                            )
                            .changed()
                            && let Ok(v) = val_str.parse::<u64>() {
                                self.cron_draft.interval_value = v.max(1);
                            }
                        egui::ComboBox::from_id_salt("cron_unit_combo")
                            .selected_text(self.cron_draft.interval_unit.label())
                            .show_ui(ui, |ui| {
                                for unit in [
                                    IntervalUnit::Minutes,
                                    IntervalUnit::Hours,
                                    IntervalUnit::Days,
                                ] {
                                    let label = unit.label().to_string();
                                    ui.selectable_value(
                                        &mut self.cron_draft.interval_unit,
                                        unit,
                                        label,
                                    );
                                }
                            });
                    });
                    ui.end_row();

                    ui.label("Output directory:");
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.cron_draft.output_dir)
                                .desired_width(180.0),
                        );
                        if ui.small_button("📁").clicked()
                            && let Some(dir) = rfd::FileDialog::new().pick_folder() {
                                self.cron_draft.output_dir =
                                    dir.to_string_lossy().into_owned();
                            }
                    });
                    ui.end_row();

                    ui.label("Filename:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.cron_draft.output_filename)
                            .desired_width(220.0),
                    );
                    ui.end_row();

                    ui.label("Enabled:");
                    ui.checkbox(&mut self.cron_draft.enabled, "");
                    ui.end_row();
                });

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let can_save = !self.cron_draft.name.trim().is_empty()
                    && !self.cron_draft.output_dir.is_empty()
                    && !self.cron_draft.output_filename.is_empty();

                if ui
                    .add_enabled(can_save, egui::Button::new("💾  Save"))
                    .clicked()
                {
                    let job = self.cron_draft.clone();
                    match self.cron_form {
                        CronFormState::Adding => {
                            self.cron_jobs.lock().unwrap().push(job);
                        }
                        CronFormState::Editing(i) => {
                            self.cron_jobs.lock().unwrap()[i] = job;
                        }
                        CronFormState::Hidden => {}
                    }
                    self.save_cron_jobs();
                    self.cron_form = CronFormState::Hidden;
                }

                if ui.button("✖  Cancel").clicked() {
                    self.cron_form = CronFormState::Hidden;
                }
            });
        }
    }
}

enum CronAction {
    Toggle(usize, bool),
    RunNow(usize),
    Edit(usize),
    Delete(usize),
}

impl eframe::App for RustopusApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_result();
        self.poll_scheduler();

        // Sync config_arc with any edits made in the connection panel.
        {
            let mut guard = self.config_arc.lock().unwrap();
            *guard = self.config.clone();
        }

        // ── Left panel: connection settings ──────────────────────────────
        egui::SidePanel::left("connection_panel")
            .resizable(false)
            .min_width(310.0)
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
            // Top-level tab bar
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.active_tab, AppTab::Fetch, "⬇  Fetch");
                ui.selectable_value(&mut self.active_tab, AppTab::Crons, "🕐  Crons");
            });
            ui.separator();

            match self.active_tab {
                AppTab::Fetch => self.show_fetch_tab(ui, ctx),
                AppTab::Crons => {
                    // show_crons_tab needs &mut self, so we call it separately.
                    let ctx_clone = ctx.clone();
                    self.show_crons_tab(ui, &ctx_clone);
                }
            }
        });
    }
}

impl RustopusApp {
    fn show_fetch_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        // Endpoint tabs
        ui.horizontal(|ui| {
            for ep in Endpoint::all() {
                let selected = self.selected_endpoint == ep;
                if ui.selectable_label(selected, ep.label()).clicked() {
                    self.selected_endpoint = ep;
                }
            }
        });
        ui.separator();

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
                        .selected_text(if params.data_type.is_empty() { "XML" } else { "CSV" })
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
                        .selected_text(if params.unpaid == "1" {
                            "Yes"
                        } else {
                            "No (default)"
                        })
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
            let fetch_btn =
                ui.add_enabled(!self.is_fetching, egui::Button::new("▶  Fetch"));
            if fetch_btn.clicked() {
                self.start_fetch(ctx);
            }

            let has_response = matches!(&self.response, Some(FetchResult::Ok(_)));
            let save_btn =
                ui.add_enabled(has_response, egui::Button::new("💾  Save to file"));
            if save_btn.clicked() {
                self.save_response();
            }

            if self.response.is_some() && ui.button("🗑  Clear").clicked() {
                self.response = None;
                self.status_msg = String::new();
            }
        });

        ui.separator();

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
                egui::RichText::new(
                    "⚠ Showing first 100 lines. Save to file to see the full response.",
                )
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
    }
}

