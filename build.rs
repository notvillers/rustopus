use std::path::{PathBuf};

const SRC_FOLDER: &'static str = "src";
const C_FOLDER: &'static str = "C";

pub struct BuildableCFile {
    pub path: PathBuf,
    pub name: String
}

impl From<(&str, &str)> for BuildableCFile {
    fn from((filename, name): (&str, &str)) -> Self {
        BuildableCFile {
            path: PathBuf::from(SRC_FOLDER).join(C_FOLDER).join(filename),
            name: name.to_string()
        }
    }
}


fn build_c_file(buildabe_c_file: BuildableCFile) {
    cc::Build::new()
        .file(buildabe_c_file.path)
        .compile(&buildabe_c_file.name);
}


fn main() {
    build_c_file(("append.c", "append").into());
    build_c_file(("date_prefix.c", "date_prefix").into());
}
