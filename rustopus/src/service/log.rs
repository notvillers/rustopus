use std::io::Write;
use chrono::Local;
use std::fs::OpenOptions;
use crate::service::path::get_current_or_root_dir;

pub fn logger<S: AsRef<str>>(message: S) {
    let content = format!("[{}] {}", Local::now().format("%Y-%m-%d %H:%M:%S").to_string(), message.as_ref());

    let log_dir = get_current_or_root_dir().join("src").join("log");
    if !log_dir.exists() {
        match std::fs::create_dir_all(&log_dir) {
            Ok(_) => {},
            Err(e) => {
                println!("Failed to create log directory '{}', content '{}', error '{}'", &log_dir.to_string_lossy(), content, e);
                return
            }
        }
    }
    match OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_dir.join(format!("{}.log", Local::now().format("%Y_%m_%d")))) {
        Ok(mut file) => {

            match writeln!(file, "{}", content) {
                Ok(_) => {
                    println!("{}", content);
                }
                Err(e) => {
                    println!("Failed to log content '{}', error '{}'", content, e);
                }
            }
        }
        Err(e) => {
            println!("Failed to open logfile '{}', content '{}', error '{}'", &log_dir.to_string_lossy(), content, e);
        }
    }
}


pub fn log_with_ip<S: AsRef<str>>(ip_address: &str, message: S) {
    logger(format!("|{}| {}", ip_address, message.as_ref()));
}