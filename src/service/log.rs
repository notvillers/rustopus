use chrono::Local;
use crate::service::path::get_current_or_root_dir;
use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::os::raw::c_char;

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

fn path_to_cstring(path: &Path) -> Result<CString, std::ffi::NulError> {
    #[cfg(unix)]
    {
        CString::new(path.as_os_str().as_bytes())
    }
    #[cfg(windows)]
    {
        let s = path.to_str().expect("Non-UTF-8 path are not supported on Windows");
        CString::new(s)
    }
}


unsafe extern "C" {
    fn append_to_file_c(filename: *const c_char, string_to_append: *const c_char) -> i32;
}


enum AppendFileError {
    Open,
    Write,
    NewLine,
    Unknown(i32)
}


fn append_to_file(path: &PathBuf, content: &str) -> Result<(), AppendFileError> {
    let c_path = path_to_cstring(&path).expect("Invalid path");
    let c_content = CString::new(content).expect("Content contained interior null byte");

    let result = unsafe {
        append_to_file_c(c_path.as_ptr(), c_content.as_ptr())
    };

    match result {
        0 => Ok(()),
        1 => Err(AppendFileError::Open),
        2 => Err(AppendFileError::Write),
        3 => Err(AppendFileError::NewLine),
        error => Err(AppendFileError::Unknown(error))
    }
}


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

    let file_path = log_dir.join(format!("{}.log", Local::now().format("%Y_%m_%d")));

    match append_to_file(&file_path, &content) {
        Err(e) => {
            match e {
                AppendFileError::Open => eprintln!("Error opening '{:#?}'", file_path),
                AppendFileError::Write => eprintln!("Error writing '{:#?}'", file_path),
                AppendFileError::NewLine => eprintln!("Error adding new line '{:#?}'", file_path),
                AppendFileError::Unknown(e) => eprintln!("Error while appending '{:#?}': {}", file_path, e)
            }
        },
        _ => {}
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
