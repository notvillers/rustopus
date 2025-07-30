use std::path::PathBuf;

fn main() {
    let append_c = PathBuf::from("src").join("service").join("append.c");
    cc::Build::new()
        .file(append_c)
        .compile("append");
}
