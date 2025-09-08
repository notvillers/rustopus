use std::env;
use std::path::PathBuf;
use crate::service::log::elogger;

pub fn get_current_or_root_dir() -> PathBuf {
    match env::current_dir() {
        Ok(path) => path,
        Err(error) => {
            elogger(format!("Error reading current directory: {}", error));
            get_root_path()
        }
    }
}


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
