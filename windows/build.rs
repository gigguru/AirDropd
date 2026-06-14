fn main() {
    println!("cargo:rerun-if-changed=../assets/airdropd.ico");
    println!("cargo:rerun-if-changed=../assets/airdropd-icon.png");

    // Embed the .exe icon for the build *target*, not the host OS — required
    // when cross-compiling Windows binaries from macOS/Linux.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_windows_icon();
    }
}

fn embed_windows_icon() {
    let icon = "../assets/airdropd.ico";
    if !std::path::Path::new(icon).exists() {
        eprintln!("warning: {icon} not found — Windows executable will use the default icon");
        return;
    }
    let mut res = winres::WindowsResource::new();
    res.set_icon(icon);
    if let Err(e) = res.compile() {
        eprintln!("warning: failed to embed Windows icon: {e}");
    }
}
