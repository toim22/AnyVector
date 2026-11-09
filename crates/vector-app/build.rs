fn main() {
    println!("cargo:rerun-if-changed=assets/vector.ico");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs/heads");

    // Source ID (About box): the commit the program was built from.
    let source_id = std::process::Command::new("git")
        .args(["rev-parse", "--short=10", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=VECTOR_SOURCE_ID={source_id}");
    // Release (About box): the month the program was built, e.g. "November 2026".
    println!("cargo:rustc-env=VECTOR_RELEASE={}", release_month());

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/vector.ico");
        res.set("ProductName", "AnyVector");
        res.set("FileDescription", "AnyVector - vector graphics editor");
        res.set("CompanyName", "AnyVector Team");
        res.set("LegalCopyright", "Copyright (c) 2026 AnyVector Team");
        if let Err(e) = res.compile() {
            println!("cargo:warning=could not embed Windows resources: {e}");
        }
    }
}

/// "Month YYYY" of today's date (UTC).
fn release_month() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    // civil-from-days (Howard Hinnant)
    let z = (secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    format!("{} {y}", MONTHS[(m - 1) as usize])
}
