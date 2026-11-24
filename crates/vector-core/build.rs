#[allow(dead_code)]
#[path = "version.rs"]
mod version;

fn main() {
    println!("cargo:rerun-if-changed=../../Cargo.toml");
    println!("cargo:rerun-if-changed=version.rs");
    let toml = std::fs::read_to_string("../../Cargo.toml").expect("the workspace's Cargo.toml");
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    println!("cargo:rustc-env=ANYVECTOR_VERSION={}", version::version_on(&toml, &os));
}
