use crate::service::path::get_current_or_root_dir;
use std::ffi::{CString, CStr};
use std::path::{Path, PathBuf};
use std::os::raw::c_char;

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(windows)]
#[allow(unused_imports)]
use std::os::windows::ffi::OsStrExt;

/// This function returns the C string path based on the file system architect
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
    fn get_datetime_str_c() -> *const c_char;
    fn get_date_str_c() -> *const c_char;
}


/// This function gets datetime string from C function
fn get_datetime_str() -> String {
    unsafe {
        CStr::from_ptr(get_datetime_str_c()).to_str().unwrap_or("unknown").to_string()
    }
}


/// This function gets date string from C function
fn get_date_str() -> String {
    unsafe {
        CStr::from_ptr(get_date_str_c()).to_str().unwrap_or("unknown").to_string()
    }
}


/// `AppendFile` enum
enum AppendFileError {
    Open,
    Write,
    NewLine,
    Unknown(i32)
}


/// This function append string to a file's content
fn append_to_file(path: &PathBuf, content: &str) -> Result<(), AppendFileError> {
    let c_path = match path_to_cstring(&path) {
        Ok(csstring) => csstring,
        Err(error) => {
            println!("Error while searching for path {:#?}, error: '{}'", path, error);
            return Err(AppendFileError::Open);
        }
    };

    let c_content = match CString::new(content) {
        Ok(csstring) => csstring,
        Err(error) => {
            println!("Content contained interior null byte, error {}", error);
            return Err(AppendFileError::Write)
        }
    };

    unsafe {
        match append_to_file_c(c_path.as_ptr(), c_content.as_ptr()) {
            0 => Ok(()),
            1 => Err(AppendFileError::Open),
            2 => Err(AppendFileError::Write),
            3 => Err(AppendFileError::NewLine),
            error => Err(AppendFileError::Unknown(error))
        }
    }
}


/// `LogType` enum
enum LogType {
    Ok,
    Error
}


/// This functions handles log content based on the given `LogType` enum
fn log_handler<S: AsRef<str>>(message: S, log_type: Option<LogType>) {
    let error_prefix: String = if let LogType::Error = log_type.as_ref().unwrap_or(&LogType::Ok) {
        "ERROR: ".into()
    } else {
        "".into()
    };

    let content = format!("[{}] {}{}", get_datetime_str(), error_prefix, message.as_ref());

    if let LogType::Error = log_type.unwrap_or(LogType::Ok) {
        eprintln!("{}", content)
    } else {
        println!("{}", content)
    };

    let log_dir = get_current_or_root_dir().join("log");
    if !log_dir.exists() {
        if let Err(e) = std::fs::create_dir_all(&log_dir) {
            println!("Failed to create log directory '{}', content '{}', error '{}'", &log_dir.to_string_lossy(), content, e);
            return
        }
    }

    let file_path = log_dir.join(format!("{}.log", get_date_str()));

    if let Err(error) = append_to_file(&file_path, &content) {
        match error {
            AppendFileError::Open => eprintln!("Error opening '{:#?}'", file_path),
            AppendFileError::Write => eprintln!("Error writing '{:#?}'", file_path),
            AppendFileError::NewLine => eprintln!("Error adding new line '{:#?}'", file_path),
            AppendFileError::Unknown(e) => eprintln!("General error while appending '{:#?}': {}", file_path, e)
        }
    }
}


/// This function logs as not an error
pub fn logger<S: AsRef<str>>(message: S) {
    log_handler(message, None);
}


/// This function logs as an error
pub fn elogger<S: AsRef<str>>(message: S) {
    log_handler(message, Some(LogType::Error));
}


/// This function logs with ip address based on `LogType`
fn log_with_ip_handle<S: AsRef<str>>(ip_address: &str, message: S, log_type: Option<LogType>) {
    if let LogType::Error = log_type.unwrap_or(LogType::Ok) {
        elogger(format!("|{}| {}", ip_address, message.as_ref()));
        return;
    }
    logger(format!("|{}| {}", ip_address, message.as_ref()))
}


/// This function logs with ip as not an error
pub fn log_with_ip<S: AsRef<str>>(ip_address: &str, message: S) {
    log_with_ip_handle(ip_address, message.as_ref(), None);
}


/// This function logs with ip as an error
pub fn elog_with_ip<S: AsRef<str>>(ip_address: &str, message: S) {
    log_with_ip_handle(ip_address, message, Some(LogType::Error));
}


/// This function logs with ip address and uuid based on `LogType`
fn log_with_ip_uuid_handle<S: AsRef<str>>(ip_address: &str, uuid: &str, message: S, log_type: Option<LogType>) {
    if let LogType::Error = log_type.unwrap_or(LogType::Ok) {
        elog_with_ip(ip_address, format!("{}: {}", uuid, message.as_ref()));
        return;
    }
    log_with_ip(ip_address, format!("{}: {}", uuid, message.as_ref()))
}


/// This function logs with ip address and uuid as not an error
pub fn log_with_ip_uuid<S: AsRef<str>>(ip_address: &str, uuid: &str, message: S) {
    log_with_ip_uuid_handle(ip_address, uuid, message, None);
}


/// This function logs with ip address and uuid as an error
pub fn elog_with_ip_uuid<S: AsRef<str>>(ip_address: &str, uuid: &str, message: S) {
    log_with_ip_uuid_handle(ip_address, uuid, message, Some(LogType::Error));
}
