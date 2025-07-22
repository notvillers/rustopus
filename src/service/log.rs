use std::io::Write;
use chrono::Local;
use std::fs::OpenOptions;
use crate::service::path::get_current_or_root_dir;

enum LogType {
    Ok,
    Error
}


fn log_handler<S: AsRef<str>>(message: S, log_type: Option<LogType>) {

    let error_prefix = match log_type.as_ref().unwrap_or(&LogType::Ok) {
        LogType::Error => "ERROR: ".to_string(),
        _ => "".to_string()
    };

    let content = format!("[{}] {}{}", Local::now().format("%Y-%m-%d %H:%M:%S").to_string(), error_prefix, message.as_ref());
    
    match log_type.unwrap_or(LogType::Ok) {
        LogType::Error => eprintln!("{}", content),
        _ => println!("{}", content)
    };

    let log_dir = get_current_or_root_dir().join("log");
    if !log_dir.exists() {
        if let Err(e) = std::fs::create_dir_all(&log_dir) {
            println!("Failed to create log directory '{}', content '{}', error '{}'", &log_dir.to_string_lossy(), content, e);
            return
        }
    }
    match OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_dir.join(format!("{}.log", Local::now().format("%Y_%m_%d")))) {
        Ok(mut file) => {
            match writeln!(file, "{}", content) {
                Err(e) => println!("Failed to log content '{}', error '{}'", content, e),
                _ => {}
            }
        }
        Err(e) => println!("Failed to open logfile '{}', content '{}', error '{}'", &log_dir.to_string_lossy(), content, e)
    }
}


pub fn elogger<S: AsRef<str>>(message: S) {
    log_handler(message, Some(LogType::Error));
}


pub fn logger<S: AsRef<str>>(message: S) {
    log_handler(message, None);
}


fn log_with_ip_handle<S: AsRef<str>>(ip_address: &str, message: S, log_type: Option<LogType>) {
    match log_type.unwrap_or(LogType::Ok) {
        LogType::Error => elogger(format!("|{}| {}", ip_address, message.as_ref())),
        _ => logger(format!("|{}| {}", ip_address, message.as_ref()))
    }
}


pub fn log_with_ip<S: AsRef<str>>(ip_address: &str, message: S) {
    log_with_ip_handle(ip_address, message.as_ref(), None);
}


pub fn elog_with_ip<S: AsRef<str>>(ip_address: &str, message: S) {
    log_with_ip_handle(ip_address, message, Some(LogType::Error));
}


fn log_with_ip_uuid_handle<S: AsRef<str>>(ip_address: &str, uuid: &str, message: S, log_type: Option<LogType>) {
    match log_type.unwrap_or(LogType::Ok) {
        LogType::Error => elog_with_ip(ip_address, format!("{}: {}", uuid, message.as_ref())),
        _ => log_with_ip(ip_address, format!("{}: {}", uuid, message.as_ref()))
    }
}


pub fn log_with_ip_uuid<S: AsRef<str>>(ip_address: &str, uuid: &str, message: S) {
    log_with_ip_uuid_handle(ip_address, uuid, message, None);
}


pub fn elog_with_ip_uuid<S: AsRef<str>>(ip_address: &str, uuid: &str, message: S) {
    log_with_ip_uuid_handle(ip_address, uuid, message, Some(LogType::Error));
}
