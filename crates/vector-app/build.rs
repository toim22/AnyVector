fn main() {
    println!("cargo:rerun-if-changed=assets/vector.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/vector.ico");
        res.set("ProductName", "Vector");
        res.set("FileDescription", "Vector - vector graphics editor");
        res.set("CompanyName", "Vector Team");
        res.set("LegalCopyright", "Copyright (c) 2026 Vector Team");
        if let Err(e) = res.compile() {
            println!("cargo:warning=could not embed Windows resources: {e}");
        }
    }
}
