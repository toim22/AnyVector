//! `crates/vector-app/assets/legal/third-party-licenses.txt`: the open-source components that go
//! into vector.exe (every crate the app depends on for Windows, build and test tools excluded),
//! with their licenses. Shown by Help ▸ About Vector ▸ Legal Notices.

use crate::{root, Result};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};

pub fn run() -> Result<()> {
    let out = std::process::Command::new(env!("CARGO"))
        .current_dir(root())
        .args(["metadata", "--format-version", "1", "--filter-platform", "x86_64-pc-windows-msvc"])
        .output()?;
    if !out.status.success() {
        return Err(format!("cargo metadata failed: {}", String::from_utf8_lossy(&out.stderr)).into());
    }
    let meta: Value = serde_json::from_slice(&out.stdout)?;
    let packages: HashMap<&str, &Value> = meta["packages"].as_array().ok_or("no packages")?.iter().filter_map(|p| Some((p["id"].as_str()?, p))).collect();
    let nodes: HashMap<&str, &Value> = meta["resolve"]["nodes"].as_array().ok_or("no resolve")?.iter().filter_map(|n| Some((n["id"].as_str()?, n))).collect();
    let app = packages.values().find(|p| p["name"] == "vector-app").and_then(|p| p["id"].as_str()).ok_or("vector-app not found")?;

    // everything vector.exe links: normal dependencies only (no dev- or build-dependencies)
    let mut seen: HashSet<&str> = HashSet::new();
    let mut stack = vec![app];
    while let Some(id) = stack.pop() {
        if !seen.insert(id) {
            continue;
        }
        for d in nodes.get(id).and_then(|n| n["deps"].as_array()).into_iter().flatten() {
            let normal = d["dep_kinds"].as_array().is_some_and(|k| k.iter().any(|k| k["kind"].is_null()));
            if normal {
                if let Some(pkg) = d["pkg"].as_str() {
                    stack.push(pkg);
                }
            }
        }
    }

    let mut by_name: BTreeMap<String, (String, String, String)> = BTreeMap::new();
    for id in &seen {
        let p = packages[id];
        let name = p["name"].as_str().unwrap_or_default().to_string();
        if name.starts_with("vector-") {
            continue;
        }
        let version = p["version"].as_str().unwrap_or_default().to_string();
        let license = p["license"].as_str().map(str::to_string).or_else(|| p["license_file"].as_str().map(|f| format!("see {f}"))).unwrap_or_else(|| "unspecified".into());
        let repo = p["repository"].as_str().or(p["homepage"].as_str()).unwrap_or_default().to_string();
        let key = format!("{name} {version}");
        by_name.insert(key.to_lowercase(), (key, license, repo));
    }

    let mut licenses: BTreeMap<String, usize> = BTreeMap::new();
    for (_, l, _) in by_name.values() {
        *licenses.entry(l.clone()).or_default() += 1;
    }
    let mut s = String::new();
    s.push_str("Vector is built with the open-source components listed below.\n");
    s.push_str("Each one is used under the license shown next to it. The license texts\n");
    s.push_str("are published with each component at the address given.\n\n");
    s.push_str(&format!("{} components. Licenses used:\n", by_name.len()));
    for (l, n) in &licenses {
        s.push_str(&format!("  {l}: {n}\n"));
    }
    s.push('\n');
    for (key, license, repo) in by_name.values() {
        s.push_str(&format!("{key}\n    License: {license}\n"));
        if !repo.is_empty() {
            s.push_str(&format!("    {repo}\n"));
        }
    }
    let dir = root().join("crates/vector-app/assets/legal");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("third-party-licenses.txt");
    std::fs::write(&path, s)?;
    println!("wrote {} ({} components)", path.display(), by_name.len());
    Ok(())
}
