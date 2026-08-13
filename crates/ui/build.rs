fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        if let Err(err) = res.compile() {
            // Don't fail a `cargo build` on a machine without the Windows
            // resource compiler (e.g. cross-compiling from Linux for CI
            // checks) — the exe just won't have an embedded icon there.
            println!("cargo:warning=failed to embed Windows icon resource: {err}");
        }
    }
}
