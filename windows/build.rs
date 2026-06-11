fn main() {
    println!("cargo:rerun-if-changed=../assets/airdropd.ico");
    println!("cargo:rerun-if-changed=../assets/airdropd-icon.png");

    #[cfg(windows)]
    embed_windows_icon();
}

#[cfg(windows)]
fn embed_windows_icon() {
    if !std::path::Path::new("../assets/airdropd.ico").exists() {
        return;
    }
    let mut res = winres::WindowsResource::new();
    res.set_icon("../assets/airdropd.ico");
    if let Err(e) = res.compile() {
        eprintln!("warning: failed to embed Windows icon: {e}");
    }
}
