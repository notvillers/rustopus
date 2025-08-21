use std::path::PathBuf;

pub struct BuildableCFile {
    pub path: PathBuf,
    pub name: String
}

impl From<(PathBuf, &str)> for BuildableCFile {
    fn from((path, name): (PathBuf, &str)) -> Self {
        BuildableCFile {
            path: path,
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
    build_c_file((PathBuf::from("src").join("C").join("append.c"), "append").into());
}
