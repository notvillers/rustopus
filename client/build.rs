fn main() {
    println!("cargo:rerun-if-changed=src/assets/images/octopus.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("src/assets/images/octopus.ico");
        if let Err(e) = res.compile() {
            println!("cargo:warning=failed to embed exe icon: {e}");
        }
    }
}
