// The version of AnyVector on each system, `[workspace.metadata.versions]` of the workspace's Cargo.toml.
// Shared by the build scripts and xtask.

/// (system, version) in the order of the file; a system is named as in `std::env::consts::OS`.
pub fn versions(cargo_toml: &str) -> Vec<(String, String)> {
    let mut lines = cargo_toml.lines().map(str::trim);
    lines.find(|l| *l == "[workspace.metadata.versions]");
    let table = lines.take_while(|l| !l.starts_with('[')).filter(|l| !l.starts_with('#'));
    table.filter_map(|l| l.split_once('=')).map(|(k, v)| (k.trim().to_string(), v.trim().trim_matches('"').to_string())).collect()
}

/// The version on `system`; 0.0.0 on one without releases.
pub fn version_on(cargo_toml: &str, system: &str) -> String {
    versions(cargo_toml).into_iter().find(|(s, _)| s == system).map_or_else(|| "0.0.0".into(), |(_, v)| v)
}
