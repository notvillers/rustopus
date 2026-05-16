use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use chrono::Utc;

use crate::api;
use crate::config::ClientConfig;
use crate::cron::CronJob;

const POLL_INTERVAL_SECS: u64 = 30;

#[derive(Debug)]
pub struct SchedulerResult {
    pub job_name: String,
    pub ran_at: String,
    pub status: String,
}

/// Spawns the background scheduler thread.
///
/// The thread wakes every [`POLL_INTERVAL_SECS`] seconds, finds due jobs from
/// the shared list, and runs each one in its own thread. Results are sent back
/// through `tx`.
pub fn start(
    jobs: Arc<Mutex<Vec<CronJob>>>,
    config: Arc<Mutex<ClientConfig>>,
    tx: std::sync::mpsc::Sender<SchedulerResult>,
    ctx_waker: egui::Context,
) {
    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(POLL_INTERVAL_SECS));
        run_due_jobs(&jobs, &config, &tx, &ctx_waker);
    });
}

/// Checks all jobs and immediately runs any that are due.
/// This is also called by "Run now" buttons, passing the specific job index.
pub fn run_due_jobs(
    jobs: &Arc<Mutex<Vec<CronJob>>>,
    config: &Arc<Mutex<ClientConfig>>,
    tx: &std::sync::mpsc::Sender<SchedulerResult>,
    ctx_waker: &egui::Context,
) {
    let snapshots: Vec<(usize, CronJob)> = {
        let guard = jobs.lock().unwrap();
        guard
            .iter()
            .enumerate()
            .filter(|(_, j)| j.enabled && j.is_due())
            .map(|(i, j)| (i, j.clone()))
            .collect()
    };

    for (idx, job) in snapshots {
        let cfg = config.lock().unwrap().clone();
        let tx = tx.clone();
        let jobs = Arc::clone(jobs);
        let ctx = ctx_waker.clone();

        thread::spawn(move || {
            let ran_at = Utc::now().to_rfc3339();
            let output_path = job.output_path();

            let result = api::fetch(
                &cfg.server_url,
                &cfg.octopus_url,
                &cfg.authcode,
                &cfg.xmlns,
                &cfg.pid,
                &crate::api::Endpoint::from_label(&job.endpoint)
                    .unwrap_or(crate::api::Endpoint::Products),
                &job.params,
            );

            let status = match result {
                Ok(body) => {
                    if let Some(parent) = output_path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    match std::fs::write(&output_path, &body) {
                        Ok(_) => format!("✔ {}", output_path.display()),                        Err(e) => format!("Fetch OK but save failed: {e}"),
                    }
                }
                Err(e) => format!("Error: {e}"),
            };

            // Update last_run and last_status in the shared list.
            {
                let mut guard = jobs.lock().unwrap();
                if let Some(j) = guard.get_mut(idx) {
                    j.last_run = Some(ran_at.clone());
                    j.last_status = Some(status.clone());
                }
            }

            let _ = tx.send(SchedulerResult {
                job_name: job.name.clone(),
                ran_at,
                status,
            });
            ctx.request_repaint();
        });
    }
}

/// Run a single job by index immediately, regardless of schedule.
pub fn run_job_now(
    idx: usize,
    jobs: &Arc<Mutex<Vec<CronJob>>>,
    config: &Arc<Mutex<ClientConfig>>,
    tx: &std::sync::mpsc::Sender<SchedulerResult>,
    ctx_waker: &egui::Context,
) {
    let job = {
        let guard = jobs.lock().unwrap();
        guard.get(idx).cloned()
    };
    let Some(job) = job else { return };

    let cfg = config.lock().unwrap().clone();
    let tx = tx.clone();
    let jobs = Arc::clone(jobs);
    let ctx = ctx_waker.clone();

    thread::spawn(move || {
        let ran_at = Utc::now().to_rfc3339();
        let output_path = job.output_path();

        let result = api::fetch(
            &cfg.server_url,
            &cfg.octopus_url,
            &cfg.authcode,
            &cfg.xmlns,
            &cfg.pid,
            &crate::api::Endpoint::from_label(&job.endpoint)
                .unwrap_or(crate::api::Endpoint::Products),
            &job.params,
        );

        let status = match result {
            Ok(body) => {
                if let Some(parent) = output_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                match std::fs::write(&output_path, &body) {
                    Ok(_) => format!("✔ {}", output_path.display()),
                    Err(e) => format!("Fetch OK but save failed: {e}"),
                }
            }
            Err(e) => format!("Error: {e}"),
        };

        {
            let mut guard = jobs.lock().unwrap();
            if let Some(j) = guard.get_mut(idx) {
                j.last_run = Some(ran_at.clone());
                j.last_status = Some(status.clone());
            }
        }

        let _ = tx.send(SchedulerResult {
            job_name: job.name.clone(),
            ran_at,
            status,
        });
        ctx.request_repaint();
    });
}
