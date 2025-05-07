use std::env;
use std::path::PathBuf;

pub fn get_current_or_root_dir() -> PathBuf {
    match env::current_dir() {
        Ok(path) => path,
        Err(e) => {
            println!("Error reading current directory: {}", e);
            get_root_path()
        }
    }
}


fn get_root_path() -> PathBuf {
    #[cfg(windows)]
    {
        PathBuf::from("C:\\")
    }
    #[cfg(not(windows))]
    {
        PathBuf::from("/")
    }
}