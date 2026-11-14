use super::*;

fn settings(rgb: &str, cmyk: &str, gray: &str) -> ColorSettings {
    ColorSettings { rgb_profile: rgb.into(), cmyk_profile: cmyk.into(), gray_profile: gray.into(), ..ColorSettings::default() }
}

fn close(a: Color, b: [u8; 3], tol: u8) -> bool {
    a.r.abs_diff(b[0]) <= tol && a.g.abs_diff(b[1]) <= tol && a.b.abs_diff(b[2]) <= tol
}

/// sRGB (0..255) to Adobe RGB (1998) the long way: linearize, sRGB→XYZ (D65), XYZ→Adobe RGB,
/// gamma 563/256.
fn srgb_to_adobe(c: [u8; 3]) -> [u8; 3] {
    let lin = c.map(|v| {
        let v = v as f64 / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    });
    let x = 0.4124564 * lin[0] + 0.3575761 * lin[1] + 0.1804375 * lin[2];
    let y = 0.2126729 * lin[0] + 0.7151522 * lin[1] + 0.0721750 * lin[2];
    let z = 0.0193339 * lin[0] + 0.1191920 * lin[1] + 0.9503041 * lin[2];
    let r = 2.0413690 * x - 0.5649464 * y - 0.3446944 * z;
    let g = -0.9692660 * x + 1.8760108 * y + 0.0415560 * z;
    let b = 0.0134474 * x - 0.1183897 * y + 1.0154096 * z;
    [r, g, b].map(|v| (v.clamp(0.0, 1.0).powf(256.0 / 563.0) * 255.0).round() as u8)
}

#[test]
fn color_management_off_uses_the_numbers() {
    let e = Engine::new(&ColorSettings::default(), &CmsOptions { engine: ColorEngine::None, ..CmsOptions::default() });
    assert_eq!(e.cmyk_to_rgb([100.0, 0.0, 0.0, 0.0]), Color::rgb(0, 255, 255));
    assert_eq!(e.cmyk_to_display([0.0, 100.0, 100.0, 0.0]), Color::rgb(255, 0, 0));
    assert_eq!(e.cmyk_to_display([0.0, 0.0, 0.0, 50.0]), Color::rgb(128, 128, 128));
    assert_eq!(e.rgb_to_cmyk(Color::rgb(255, 0, 0)), [0.0, 100.0, 100.0, 0.0]);
    assert_eq!(e.gray_to_rgb(128.0), Color::rgb(128, 128, 128));
}

#[test]
fn srgb_documents_show_rgb_as_it_is() {
    let e = Engine::new(&ColorSettings::default(), &CmsOptions::default());
    assert!(!e.rgb_needs_display_conversion());
    let c = Color::rgba(12, 200, 99, 40);
    assert_eq!(e.rgb_to_display(c), c);
}

#[test]
fn adobe_rgb_matches_the_matrix_math() {
    without_installed_profiles(|| {
        let s = settings("Adobe RGB (1998)", DEFAULT_SAMPLED, "Dot Gain 20%");
        let e = Engine::new(&s, &CmsOptions { engine: ColorEngine::Builtin, ..CmsOptions::default() });
        assert!(e.rgb_needs_display_conversion());
        for c in [[0u8, 255, 0], [255, 0, 0], [30, 120, 200], [128, 128, 128]] {
            let want = srgb_to_adobe(c);
            let got = e.display_to_rgb(Color::rgb(c[0], c[1], c[2]));
            assert!(close(got, want, 2), "sRGB {c:?} -> Adobe RGB {got:?}, expected {want:?}");
            // and back to the screen
            let back = e.rgb_to_display(got);
            assert!(close(back, c, 3), "{c:?} -> {got:?} -> {back:?}");
        }
        // the well-known value: sRGB green is about (144, 255, 60) in Adobe RGB
        assert!(close(e.display_to_rgb(Color::rgb(0, 255, 0)), [144, 255, 60], 3));
    })
}

/// A CMYK profile CorelDRAW samples (tests run without the installed profile files).
const DEFAULT_SAMPLED: &str = "U.S. Web Coated (SWOP) v2";

#[test]
fn cmyk_without_a_profile_file_looks_as_in_coreldraw() {
    without_installed_profiles(|| {
        let s = settings("sRGB IEC61966-2.1", DEFAULT_SAMPLED, "Dot Gain 20%");
        let e = Engine::new(&s, &CmsOptions::default());
        assert_eq!(e.sources()[1], ProfileSource::Sampled);
        let g = grid::profile_grid(DEFAULT_SAMPLED).unwrap();
        for ink in [[0.0, 100.0, 100.0, 0.0], [100.0, 0.0, 0.0, 0.0], [20.0, 40.0, 60.0, 10.0]] {
            let want = grid::sample(g, ink.map(|v| v / 100.0)).map(|v| v.round() as u8);
            assert!(close(e.cmyk_to_display(ink), want, 1));
            assert!(close(e.cmyk_to_rgb(ink), want, 1));
        }
        // RGB to CMYK and back keeps the look
        for ink in [[0.1, 0.8, 0.9, 0.1], [0.8, 0.4, 0.1, 0.0], [0.1, 0.15, 0.6, 0.0], [0.5, 0.45, 0.45, 0.2]] {
            let c = grid::sample(g, ink).map(|v| v.round() as u8);
            let c = Color::rgb(c[0], c[1], c[2]);
            let ink = e.rgb_to_cmyk(c);
            let back = e.cmyk_to_display(ink);
            assert!(close(back, [c.r, c.g, c.b], 6), "{c:?} -> {ink:?} -> {back:?}");
        }
        // an unknown profile name falls back to the default SWOP look
        let e = Engine::new(&settings("sRGB IEC61966-2.1", "No Such Press", "Dot Gain 20%"), &CmsOptions::default());
        assert_eq!(e.sources()[1], ProfileSource::Substituted);
        assert!(close(e.cmyk_to_display([0.0, 100.0, 100.0, 0.0]), [237, 50, 55], 1));
    })
}

#[test]
fn swop_icc_profile_when_installed() {
    // U.S. Web Coated (SWOP) v2 is installed with CorelDRAW and Adobe software; skip elsewhere
    let s = ColorSettings::default();
    let e = Engine::new(&s, &CmsOptions::default());
    let ProfileSource::File(path) = &e.sources()[1] else {
        eprintln!("U.S. Web Coated (SWOP) v2 is not installed: skipped");
        return;
    };
    eprintln!("SWOP profile: {}", path.display());
    // the ICC profile shows the inks as CorelDRAW 2026 does (its on-screen colors, sampled)
    let seen: [([f64; 4], [u8; 3]); 8] = [
        ([0.0, 100.0, 100.0, 0.0], [237, 50, 55]),
        ([100.0, 0.0, 0.0, 0.0], [0, 175, 239]),
        ([0.0, 0.0, 0.0, 100.0], [55, 52, 53]),
        ([50.0, 50.0, 50.0, 50.0], [91, 84, 82]),
        ([6.0, 9.0, 68.0, 12.0], [216, 198, 108]),
        ([53.0, 8.0, 30.0, 11.0], [114, 173, 169]),
        ([39.0, 71.0, 87.0, 23.0], [140, 89, 65]),
        ([0.0, 0.0, 0.0, 0.0], [255, 255, 255]),
    ];
    for (ink, want) in seen {
        let got = e.cmyk_to_display(ink);
        assert!(close(got, want, 4), "{ink:?}: {got:?} against {want:?}");
    }
    // the CMYK of RGB red prints close to it
    let ink = e.rgb_to_cmyk(Color::rgb(237, 50, 55));
    assert!(ink[1] > 85.0 && ink[2] > 80.0 && ink[0] < 15.0, "{ink:?}");
    assert!(e.cmyk_icc().is_some_and(|b| &b[36..40] == b"acsp" && &b[16..20] == b"CMYK"));
}

#[test]
fn pure_black_and_grays() {
    without_installed_profiles(|| {
        let s = settings("sRGB IEC61966-2.1", DEFAULT_SAMPLED, "Dot Gain 20%");
        let keep = Engine::new(&s, &CmsOptions { preserve_pure_black: true, map_gray_to_black: true, ..CmsOptions::default() });
        assert_eq!(keep.rgb_to_cmyk(Color::BLACK), [0.0, 0.0, 0.0, 100.0]);
        let gray = keep.rgb_to_cmyk(Color::rgb(128, 128, 128));
        assert_eq!(&gray[..3], &[0.0, 0.0, 0.0]);
        assert!(gray[3] > 30.0 && gray[3] < 80.0, "{gray:?}");
        // the K alone looks like the gray
        assert!(close(keep.cmyk_to_display(gray), [128, 128, 128], 6));
        let rich = Engine::new(&s, &CmsOptions { preserve_pure_black: false, map_gray_to_black: false, ..CmsOptions::default() });
        let black = rich.rgb_to_cmyk(Color::BLACK);
        assert!(black[0] > 30.0 && black[3] > 50.0, "rich black {black:?}");
        assert!(rich.rgb_to_cmyk(Color::rgb(128, 128, 128))[0] > 5.0);
        assert_eq!(rich.rgb_to_cmyk(Color::WHITE), [0.0; 4]);
    })
}

#[test]
fn gray_profiles() {
    without_installed_profiles(|| {
        let e = Engine::new(&settings("sRGB IEC61966-2.1", DEFAULT_SAMPLED, "Dot Gain 20%"), &CmsOptions::default());
        assert_eq!(e.gray_to_rgb(255.0), Color::WHITE);
        let mid = e.gray_to_rgb(128.0);
        // 50% ink with 20% dot gain prints as 70%: L* 62
        assert!(mid.r.abs_diff(149) <= 2 && mid.r == mid.g && mid.g == mid.b, "{mid:?}");
        assert!((e.rgb_to_gray(mid) - 128.0).abs() < 1.5);
        for g in [0.0, 0.25, 0.5, 0.9] {
            assert!((dot_gain_from_l(0.2, dot_gain_to_l(0.2, g)) - g).abs() < 1e-6);
        }
        let gamma = Engine::new(&settings("sRGB IEC61966-2.1", DEFAULT_SAMPLED, "Gray Gamma 2.2"), &CmsOptions::default());
        let g = gamma.gray_to_rgb(128.0);
        assert!(g.r.abs_diff(128) <= 4, "{g:?}");
    })
}

#[test]
fn cmyk_colors_keep_their_inks() {
    without_installed_profiles(|| {
        with_settings(&settings("sRGB IEC61966-2.1", DEFAULT_SAMPLED, "Dot Gain 20%"), || {
            let c = Color::from_cmyk(0.0, 60.0, 100.0, 5.0);
            assert_eq!(c.cmyk, Some([0, 60, 100, 5]));
            assert_eq!(c.to_cmyk(), [0.0, 60.0, 100.0, 5.0]);
            // the RGB is what the profile shows
            let g = grid::profile_grid(DEFAULT_SAMPLED).unwrap();
            let want = grid::sample(g, [0.0, 0.6, 1.0, 0.05]).map(|v| v.round() as u8);
            assert!(close(c, want, 1));
            // alpha changes keep the inks; blends of two CMYK colors blend the inks
            assert_eq!(c.with_alpha(10).cmyk, c.cmyk);
            let mid = Color::lerp(c, Color::from_cmyk(100.0, 0.0, 0.0, 0.0), 0.5);
            assert_eq!(mid.cmyk, Some([50, 30, 50, 3]));
            assert_eq!(Color::lerp(c, Color::rgb(0, 0, 0), 0.5).cmyk, None);
        });
    })
}

#[test]
fn colors_save_and_load_with_their_inks() {
    let c = Color::rgb(10, 20, 30).with_cmyk([1.0, 2.0, 3.0, 4.0]);
    let j = serde_json::to_string(&c).unwrap();
    assert_eq!(serde_json::from_str::<Color>(&j).unwrap(), c);
    // RGB colors are written as before, and old files load
    assert_eq!(serde_json::to_string(&Color::rgb(1, 2, 3)).unwrap(), r#"{"r":1,"g":2,"b":3,"a":255}"#);
    assert_eq!(serde_json::from_str::<Color>(r#"{"r":1,"g":2,"b":3,"a":255}"#).unwrap(), Color::rgb(1, 2, 3));
}

#[test]
fn display_without_an_active_engine_is_the_stored_rgb() {
    without_installed_profiles(|| {
        let c = Color::rgb(1, 2, 3).with_cmyk([0.0, 0.0, 0.0, 100.0]);
        if !SHOWING.load(Ordering::Relaxed) {
            assert_eq!(display(c), c);
        }
        // an export thread shows through its document's settings
        let off = with_settings(&settings("sRGB IEC61966-2.1", DEFAULT_SAMPLED, "Dot Gain 20%"), || display(c));
        assert!(close(off, [55, 52, 53], 30), "{off:?}");
    })
}

#[test]
fn soft_proof_flags_colors_the_press_cannot_print() {
    without_installed_profiles(|| {
        let e = Engine::new(&ColorSettings::default(), &CmsOptions::default());
        let lut = e.proof(DEFAULT_SAMPLED, RenderingIntent::RelativeColorimetric);
        let (green, d) = lut.apply([0.0, 255.0, 0.0]);
        assert!(d > 5.0, "pure green is out of gamut ({d})");
        assert!(green[1] < 250.0 || green[0] > 5.0, "{green:?}");
        let (gray, d) = lut.apply([150.0, 150.0, 150.0]);
        assert!(d < 4.0, "gray prints ({d})");
        assert!(gray.iter().all(|v| (v - 150.0).abs() < 8.0), "{gray:?}");
        // color management off: no change
        let off = Engine::new(&ColorSettings::default(), &CmsOptions { engine: ColorEngine::None, ..CmsOptions::default() });
        assert_eq!(off.proof(DEFAULT_SAMPLED, RenderingIntent::Perceptual).apply([0.0, 255.0, 0.0]), ([0.0, 255.0, 0.0], 0.0));
    })
}

#[test]
fn converting_cmyk_between_profiles_keeps_the_look() {
    without_installed_profiles(|| {
        let a = Engine::new(&settings("sRGB IEC61966-2.1", DEFAULT_SAMPLED, "Dot Gain 20%"), &CmsOptions::default());
        let b = Engine::new(&settings("sRGB IEC61966-2.1", "Japan Color 2001 Coated", "Dot Gain 20%"), &CmsOptions::default());
        let ink = [10.0, 70.0, 40.0, 5.0];
        let moved = a.cmyk_to_other(ink, &b);
        assert_ne!(moved, ink);
        assert!(close(b.cmyk_to_display(moved), { let d = a.cmyk_to_display(ink); [d.r, d.g, d.b] }, 8));
        assert_eq!(a.cmyk_to_other(ink, &a), ink);
    })
}

#[test]
fn registered_output_conditions() {
    assert_eq!(output_condition("U.S. Web Coated (SWOP) v2").map(|c| c.0), Some("CGATS TR 001"));
    assert_eq!(output_condition("Coated FOGRA39 (ISO 12647-2:2004)").map(|c| c.0), Some("FOGRA39"));
    assert_eq!(output_condition("My Press"), None);
}
