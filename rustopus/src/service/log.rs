use std::env;
use std::io::Write;
use chrono::Local;
use std::fs::OpenOptions;

pub fn logger<S: AsRef<str>>(message: S) {
    let current_dir = env::current_dir().expect("Failed to get current directory");
    let log_dir = current_dir.join("src").join("log");
    if !log_dir.exists() {
        std::fs::create_dir_all(&log_dir).expect("Failed to create log directory");
    }
    let now = Local::now();
    let filename = format!("{}.log", now.format("%Y_%m_%d"));
    let filepath = log_dir.join(filename);

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(filepath)
        .expect("Failed to open log file");

    let timestamp = now.format("%Y-%m-%d %H:%M:%S").to_string();
    let content = format!("[{}] {}", timestamp, message.as_ref());

    match writeln!(file, "{}", content) {
        Ok(_) => {
            println!("{}", content);
        }
        Err(e) => {
            println!("Failed to log: '{}', error: {}", content, e);
        }
    }
}


pub fn log_with_ip<S: AsRef<str>>(ip_address: &str, message: S) {
    logger(format!("|{}| {}", ip_address, message.as_ref()));
}