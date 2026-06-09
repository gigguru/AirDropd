fn main() {
    println!("cargo:rerun-if-changed=assets/airdropd.ico");
    println!("cargo:rerun-if-changed=assets/airdropd-icon.png");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        if std::path::Path::new("assets/airdropd.ico").exists() {
            let mut res = winres::WindowsResource::new();
            res.set_icon("assets/airdropd.ico");
            if let Err(e) = res.compile() {
                eprintln!("warning: failed to embed Windows icon: {e}");
            }
        }
    }
}
