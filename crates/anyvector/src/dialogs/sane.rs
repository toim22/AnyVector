//! Scanners through SANE's command-line program `scanimage` (Debian/Ubuntu package sane-utils):
//! its device list, a device's resolutions and color modes, and the scan command. Used by File ▸
//! Acquire Image off Windows (`scan.rs`); plain text in and out, so tested on every system.

#![cfg_attr(windows, allow(dead_code))]

use serde::{Deserialize, Serialize};

/// The program, found on the PATH.
pub const SCANIMAGE: &str = "scanimage";

/// File ▸ Acquire Image: the chosen device and the last scan settings.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScanSettings {
    /// The SANE device name (`scanimage -d`).
    pub device: Option<String>,
    /// The device's label when it was chosen.
    pub label: Option<String>,
    pub mode: Option<String>,
    pub dpi: Option<u32>,
}

/// A scanner `scanimage -L` lists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    /// What `-d` takes, e.g. `pixma:04A91912` or `airscan:e0:Canon MF240`.
    pub name: String,
    /// Maker, model and kind, e.g. `CANON Canon PIXMA MG5200 multi-function peripheral`.
    pub label: String,
}

/// The devices in the output of `scanimage -L`: lines like
/// ``device `test:0' is a Noname frontend-tester virtual device``. "No scanners were
/// identified" and anything else is skipped.
pub fn parse_devices(out: &str) -> Vec<Device> {
    let mut list = vec![];
    for line in out.lines() {
        let Some(rest) = line.trim().strip_prefix("device ") else { continue };
        // the name is quoted `like this' (older and newer versions alike), or 'like this'
        let Some(rest) = rest.strip_prefix('`').or_else(|| rest.strip_prefix('\'')) else { continue };
        let (name, label) = match rest.find("' is a ") {
            Some(i) => (&rest[..i], rest[i + "' is a ".len()..].trim()),
            None => match rest.rfind('\'') {
                Some(i) => (&rest[..i], ""),
                None => continue,
            },
        };
        if name.is_empty() || list.iter().any(|d: &Device| d.name == name) {
            continue;
        }
        let label = if label.is_empty() { name } else { label };
        list.push(Device { name: name.to_string(), label: label.to_string() });
    }
    list
}

/// The color modes and resolutions a device offers (`scanimage -d <device> -A`), with its
/// defaults. Empty lists: the device has no such option, and the scan leaves it alone.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeviceOptions {
    pub modes: Vec<String>,
    pub mode: Option<String>,
    pub resolutions: Vec<u32>,
    pub resolution: Option<u32>,
}

/// The resolutions offered when a device takes any value in a range.
const COMMON_DPI: [u32; 8] = [75, 100, 150, 200, 300, 400, 600, 1200];

/// What is offered when the device's options could not be read.
pub fn fallback_options() -> DeviceOptions {
    DeviceOptions { modes: vec!["Color".into(), "Gray".into(), "Lineart".into()], mode: Some("Color".into()), resolutions: vec![75, 150, 300, 600], resolution: Some(300) }
}

/// The values and default of an option line: `--mode Gray|Color [Gray]` gives
/// (`Gray|Color`, `Gray`). An option that does not apply now is `[inactive]`: None.
fn option_spec<'a>(line: &'a str, name: &str) -> Option<(&'a str, &'a str)> {
    let rest = line.trim_start().strip_prefix(name)?.strip_prefix(' ')?;
    let open = rest.rfind(" [")?;
    let default = rest[open + 2..].trim_end().strip_suffix(']')?;
    (default != "inactive").then(|| (rest[..open].trim(), default))
}

fn number(s: &str) -> Option<u32> {
    let v: f64 = s.trim().trim_end_matches("dpi").trim().parse().ok()?;
    (v.is_finite() && v >= 1.0).then(|| v.round() as u32)
}

/// Read the `--mode` and `--resolution` options of `scanimage -A` output.
pub fn parse_options(out: &str) -> DeviceOptions {
    let mut o = DeviceOptions::default();
    for line in out.lines() {
        if let Some((values, default)) = option_spec(line, "--mode") {
            o.modes = values.split('|').map(str::trim).filter(|m| !m.is_empty()).map(String::from).collect();
            o.mode = Some(default.to_string()).filter(|d| o.modes.contains(d));
        } else if let Some((values, default)) = option_spec(line, "--resolution") {
            // `1..1200dpi (in steps of 1)` or `75|150|300dpi`
            let values = values.split(" (").next().unwrap_or(values).trim().trim_end_matches("dpi");
            o.resolutions = match values.split_once("..") {
                Some((lo, hi)) => match (number(lo), number(hi)) {
                    (Some(lo), Some(hi)) if lo <= hi => {
                        let mut v: Vec<u32> = COMMON_DPI.iter().copied().filter(|d| (lo..=hi).contains(d)).collect();
                        if v.is_empty() {
                            v.push(lo);
                        }
                        v
                    }
                    _ => vec![],
                },
                None => values.split('|').filter_map(number).collect(),
            };
            o.resolutions.sort_unstable();
            o.resolutions.dedup();
            o.resolution = number(default).filter(|d| o.resolutions.contains(d));
        }
    }
    o
}

/// The mode to start with: the last one used when the device has it, else color, else the
/// device's own default.
pub fn initial_mode(o: &DeviceOptions, last: Option<&str>) -> usize {
    let at = |m: &str| o.modes.iter().position(|x| x.eq_ignore_ascii_case(m));
    last.and_then(at).or_else(|| at("Color")).or_else(|| o.mode.as_deref().and_then(at)).unwrap_or(0)
}

/// The resolution to start with: the last one used, else 300 dpi, else the device's default.
pub fn initial_dpi(o: &DeviceOptions, last: Option<u32>) -> usize {
    let at = |d: u32| o.resolutions.iter().position(|x| *x == d);
    last.and_then(at).or_else(|| at(300)).or_else(|| o.resolution.and_then(at)).unwrap_or(0)
}

/// What a color mode is called in the dialog (SANE's standard names in the words of the Windows
/// scan dialog; a device's own names as they are).
pub fn mode_label(mode: &str) -> &str {
    match mode.to_ascii_lowercase().as_str() {
        "color" => "Color",
        "gray" | "grey" | "grayscale" => "Grayscale",
        "lineart" | "binary" => "Black and White",
        "halftone" => "Halftone",
        _ => mode,
    }
}

/// The arguments of `scanimage` for a scan to PNG on its standard output.
pub fn scan_args(device: &str, mode: Option<&str>, dpi: Option<u32>) -> Vec<String> {
    let mut a = vec!["-d".to_string(), device.to_string(), "--format=png".to_string()];
    if let Some(m) = mode {
        a.push("--mode".into());
        a.push(m.into());
    }
    if let Some(d) = dpi {
        a.push("--resolution".into());
        a.push(d.to_string());
    }
    a
}

/// The line of `scanimage`'s error output that says what went wrong, without the program name
/// (empty when there is none).
pub fn error_line(stderr: &str) -> String {
    let lines: Vec<&str> = stderr.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with("Output format is not set")).collect();
    let line = lines.iter().rev().find(|l| l.starts_with("scanimage:")).or(lines.last()).copied().unwrap_or("");
    line.strip_prefix("scanimage:").unwrap_or(line).trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scanimage_device_list() {
        let out = "device `test:0' is a Noname frontend-tester virtual device\n\
                   device `test:1' is a Noname frontend-tester virtual device\n\
                   device `airscan:e0:Canon MF240 (it's new)' is a eSCL Canon MF240 ip=192.168.1.20\n\
                   device `pixma:04A91912' is a CANON Canon PIXMA MG5200 multi-function peripheral\n";
        let d = parse_devices(out);
        assert_eq!(d.len(), 4);
        assert_eq!(d[0], Device { name: "test:0".into(), label: "Noname frontend-tester virtual device".into() });
        assert_eq!(d[2].name, "airscan:e0:Canon MF240 (it's new)", "a quote inside the name");
        assert_eq!(d[3].label, "CANON Canon PIXMA MG5200 multi-function peripheral");
        // straight quotes, and a line without a description
        let d = parse_devices("device 'escl:https://10.0.0.5:443' is a HP eSCL scanner\ndevice `net:host:plustek'\n");
        assert_eq!(d.iter().map(|d| d.name.as_str()).collect::<Vec<_>>(), ["escl:https://10.0.0.5:443", "net:host:plustek"]);
        assert_eq!(d[1].label, "net:host:plustek", "no description: the name");
    }

    #[test]
    fn no_scanner_and_errors_give_no_devices() {
        let none = "\nNo scanners were identified. If you were expecting something different,\n\
                    check that the scanner is plugged in, turned on and detected by the\n\
                    sane-find-scanner tool (if appropriate). Please read the documentation\n\
                    which came with this software (README, FAQ, manpages).\n";
        assert!(parse_devices(none).is_empty());
        assert!(parse_devices("").is_empty());
        assert!(parse_devices("scanimage: sane_init failed: Out of memory\n").is_empty());
        assert!(parse_devices("device `test:0' is a x\ndevice `test:0' is a x\n").len() == 1, "listed twice: once");
    }

    #[test]
    fn a_devices_options() {
        // SANE's test backend
        let test = "Output format is not set, using pnm as a default.\n\nAll options specific to device `test':\n  Scan Mode:\n    --mode Gray|Color [Gray]\n        Selects the scan mode (e.g., lineart, monochrome, or color).\n    --depth 1|8|16 [8]\n    --three-pass[=(yes|no)] [inactive]\n    --resolution 1..1200dpi (in steps of 1) [50]\n        Sets the resolution of the scanned image.\n    --source Flatbed|Automatic Document Feeder [Flatbed]\n";
        let o = parse_options(test);
        assert_eq!(o.modes, ["Gray", "Color"]);
        assert_eq!(o.mode.as_deref(), Some("Gray"));
        assert_eq!(o.resolutions, COMMON_DPI.to_vec(), "a range: the common values in it");
        assert_eq!(o.resolution, None, "50 dpi is not among them");
        // a typical scanner: a list of resolutions
        let pixma = "  Scan mode:\n    --resolution 75|150|300|600|1200|2400dpi [75]\n        Sets the resolution of the scanned image.\n    --mode Color|Gray|Lineart [Color]\n";
        let o = parse_options(pixma);
        assert_eq!(o.resolutions, [75, 150, 300, 600, 1200, 2400]);
        assert_eq!((o.resolution, o.mode.as_deref()), (Some(75), Some("Color")));
        assert_eq!(initial_dpi(&o, None), 2, "300 dpi first");
        assert_eq!(initial_dpi(&o, Some(1200)), 4, "else the last one used");
        assert_eq!(initial_mode(&o, Some("gray")), 1);
        assert_eq!(initial_mode(&o, Some("Halftone")), 0, "not offered: color");
        // a narrow range, an inactive option, and no options at all
        let o = parse_options("    --resolution 50..60dpi (in steps of 1) [50]\n    --mode Color [inactive]\n");
        assert_eq!(o.resolutions, [50]);
        assert!(o.modes.is_empty());
        assert_eq!(parse_options("scanimage: open of device x failed: Invalid argument\n"), DeviceOptions::default());
        assert_eq!(initial_dpi(&DeviceOptions::default(), Some(300)), 0);
    }

    #[test]
    fn the_scan_command_line() {
        assert_eq!(scan_args("test:0", Some("Color"), Some(300)), ["-d", "test:0", "--format=png", "--mode", "Color", "--resolution", "300"]);
        // a device without these options: they are left out
        assert_eq!(scan_args("airscan:e0:Canon MF240", None, None), ["-d", "airscan:e0:Canon MF240", "--format=png"]);
        assert_eq!(mode_label("Gray"), "Grayscale");
        assert_eq!(mode_label("Lineart"), "Black and White");
        assert_eq!(mode_label("True Gray"), "True Gray");
    }

    #[test]
    fn scanimage_errors_read_well() {
        assert_eq!(error_line("scanimage: open of device foo failed: Invalid argument\n"), "open of device foo failed: Invalid argument");
        assert_eq!(error_line("Output format is not set, using pnm as a default.\nscanimage: sane_start: Document feeder out of documents\n"), "sane_start: Document feeder out of documents");
        assert_eq!(error_line(""), "");
    }
}
