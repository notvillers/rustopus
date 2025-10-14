use std::env;
use std::path::PathBuf;
use crate::service::log::elogger;

/// This function gets the root dir based on the current filesystem architect
fn get_root_path() -> PathBuf {
    #[cfg(windows)]
    {
        "C:\\".into()
    }
    #[cfg(not(windows))]
    {
        "/".into()
    }
}


// This function gets current or root dir
pub fn get_current_or_root_dir() -> PathBuf {
    match env::current_dir() {
        Ok(path) => return path,
        Err(error) => {
            elogger(format!("Error reading current directory: {}", error));
        }
    }
    get_root_path()
}
