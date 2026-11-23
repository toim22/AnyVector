# Linux audit: file names and paths, Windows bug fixes, feature gaps

Part of the Linux port (issue #51, plan in `docs/LINUX-PORT-PLAN.md`). Three checks:

1. code that assumes Windows file-name rules (case-insensitive names, `\` as a separator, Windows
   paths stored in drawings and settings);
2. the 79 closed issues: whether each fix also works on Linux;
3. what works on Windows but is missing or weaker on Linux and is not yet in section 2 of the plan.

"Phase" refers to the roadmap in section 6 of the plan. Owners A to E are the other parts of the
port: A window and Wayland, B fonts, C locale, system info, updater and AI, D desktop integration,
E clipboard; F is this audit. Line numbers in other owners' files are those of the 0.6.2 sources.

Tests: `cargo test --workspace` on Windows: 633 passed, 0 failed, 76 ignored. On Linux (WSL 2,
Ubuntu 24.04, llvmpipe): 623 passed, 10 failed, 71 ignored. The 10 Linux failures all fail on
unchanged main too: 7 visual tests that panic in epaint `fonts.rs:806` (fonts, agent B),
`sysinfo::tests::every_category_has_rows` (agent C), and the two font-metric tests in the B row of
the hand-off table. Every new test in this audit passes on both systems, and so do all issue
regression tests that do not hit the epaint font panic (`issue_124_alt_drag…` and
`issue_151_ctrl_click_keeps_its_other_meanings` do hit it).

## 1. File names and paths

Windows compares file names without case and takes both `\` and `/` as separators. Linux compares
names byte for byte and only knows `/`; a file name need not even be UTF-8. Drawings and settings
also keep paths written on the computer that made them (`C:\Users\ana\Logos.csl`).

The new module `vector-io/src/paths.rs` holds the helpers: `has_extension` and `strip_extension`
(any case), `foreign_file_name` and `foreign_components` (a path from any system taken apart at
both separators, without the drive), `find_in_dir` / `find_under` (a name looked up without case,
the way Windows finds it), `resolve_stored_path` (a stored path, else its end looked for in the
drawing's folder and other folders, longest first) and `dialog_extensions` (file dialog patterns
that match any case on Linux).

### Fixed

| Where | Problem | Fix | Test | Windows |
| --- | --- | --- | --- | --- |
| `vector-io/src/dxf_import/mod.rs` (STYLE table) | The font file of a text style is often a Windows path (`C:\WINDOWS\Fonts\ARIALBD.TTF`); on Linux `Path::file_stem` returned the whole string, so the style's font was wrong | Take the file name with `foreign_file_name` | `dxf_import::tests::text_style_fonts_named_by_windows_paths` | unchanged |
| `anyvector/src/docview.rs` `stem()` | Only a lower-case `.cdr`/`.vct` was removed, so `LOGO.CDR` exported as `LOGO.CDR.pdf` and Send To saved `LOGO.CDR.cdr` | `strip_extension` in any case | `docview::tests::the_stem_drops_the_drawing_extension_in_any_case` | fixed too (shared bug) |
| `anyvector/src/dialogs/print.rs:139` | Print to File name: same as above | same | (covered by the stem test) | fixed too |
| `anyvector/src/printing.rs` print styles | `.PRS` files were left out of the list (both systems); a style was found by its exact file name only (Linux) | `has_extension`; look up the style by name without case, exact name first | `printing::tests::print_styles_with_capital_extensions` | `.PRS` styles now listed |
| `anyvector/src/dialogs/export.rs` user presets | A preset file `Web.JSON` was not found on Linux | `find_in_dir` | `dialogs::export::tests::user_presets_with_capital_extensions` | unchanged |
| `anyvector/src/dialogs/object.rs` Update from Link | A linked symbol keeps the library path of the computer it was made on; on Linux (or another PC) the library was "not found" | Stored path, else a library of that name (any case) next to the drawing, then in the Symbols folder | `dialogs::object::tests::linked_symbol_libraries_found_by_name` | also finds a moved library |
| `anyvector/src/dialogs/merge.rs` Sync with data source | Same for the Print Merge CSV. Sync also used `read_to_string`, so a CSV saved in the Windows code page (as Excel saves it) silently did nothing, while Import read it | Look next to the drawing when the stored path is gone (searched once, not every frame); Sync reads the file like Import | `dialogs::merge::tests::data_source_found_next_to_a_moved_drawing` | also finds a moved CSV; code-page CSVs sync |
| `vector-io/src/svg_import.rs` | Images an SVG links (`<image href="images/logo.png">`) were looked for in the working folder, so they were lost on both systems unless AnyVector was started from the SVG's folder; on Linux also `images\logo.png`, `C:\…`, `file:///C:/…` and names in another case | Read links from the SVG's folder (`resources_dir`); a link that is not found is looked for by name beside the SVG | `paths::tests::mixed_case_files_open_and_import`, `svg_import::tests::image_links_give_paths` | relative links now load |
| File dialogs (`rfd`) in `dialogs/forms.rs`, `merge.rs`, `object.rs`, `options.rs`, `palette.rs` (3), `print.rs`, `text.rs`, `workspace.rs`, `dockers/find.rs`, `fx_dockers.rs` (2), `ostyles.rs`, `props.rs` | The Linux dialogs (portal, GTK, zenity) match `*.cdr` case-sensitively, so `LOGO.CDR` is not listed | `dialog_extensions`: on Linux each extension is followed by an any-case pattern (`[cC][dD][rR]`, as the portal documentation advises); the plain extension stays first for KDE | `paths::tests::dialog_patterns_match_any_case`, `any_case_globs_accept_mixed_case_names` | unchanged (list returned as given) |
| `anyvector/src/menus.rs` File ▸ Send To | The entries come from `%APPDATA%\Microsoft\Windows\SendTo`; on Linux the submenu was empty | Off Windows: Desktop, Documents, Mail Recipient (the commands AnyVector already has) | `menus::tests::send_to_has_entries_on_every_system` | unchanged |
| `anyvector/src/tools/interact.rs:2908` comment author | Only `USERNAME`, which Linux does not set: comments were signed "User" | `dockers::login_name()` (`USERNAME`, `USER`, `LOGNAME`) and `dockers::comment_author()` | `dockers::tests::comments_signed_by_the_login_name` | unchanged |
| `anyvector/src/app.rs` command line | `std::env::args()` panics on a file name that is not UTF-8, so AnyVector crashed at start when the file manager opened such a file | `args_os()` | `app::tests::command_line_files_with_any_name` (a Latin-1 name on Linux) | unchanged |
| `xtask/src/ui.rs` | Reference and CorelDRAW folders joined with `r"Draw\UIConfig\…"` | Forward slashes (Windows accepts them) | (build tool) | unchanged |
| vector-io open, import, thumbnails | Already compared extensions in lower case | Test added | `paths::tests::mixed_case_files_open_and_import` | unchanged |

Checked, nothing to change: `vector_io::extension` (lower case) and the open/import dispatch; palette
files (read by content); the bitmap list of the fx dockers, the PDF name check in Export and the WFN
font names (lower-cased); profile files in `vector-core/src/cms` (lower-cased extensions, Linux
folders already listed). No `to_str().unwrap()` on paths. Hard-coded `C:\` only in Windows-only code,
test data and the xtask defaults (which `ANYVECTOR_COREL` / `ANYVECTOR_REFERENCE` override). The
names in `include_str!` / `include_bytes!` and the test fixtures match their files' case (the Linux
build compiles and the tests find their fixtures).

### For the other owners

| Owner | File:line | Problem | Suggested fix | Phase |
| --- | --- | --- | --- | --- |
| D | `exec.rs:444-455` `open_filters` | File ▸ Open / Import filters are case-sensitive on Linux | `t.dialog_extensions()` per type and `vector_io::paths::dialog_extensions(&all)` for All File Formats | 1 |
| D | `exec.rs:2553` Run Script, `dialogs/bitmaps.rs:1283, 1493, 2506, 2593`, `dockers/misc.rs:1283`, `bugreport.rs:362-365` | Same, other open dialogs | `vector_io::paths::dialog_extensions(&[...])` | 1 |
| D | `exec.rs:419` | `strip_suffix(".cdr")` / `".vct"` misses `.CDR` | `vector_io::paths::strip_extension(name, &["cdr", "vct"])` | 1 |
| D | `exec.rs:663-665` | Linux Save dialog: the name is always `stem.cdr` and the type is the first filter, also for a `.vct` drawing | Put the chosen type's filter first and use `types[type_index].1` in the name | 1 |
| D | `dockers/misc.rs:431` (`== "csl"`), `:1600` (`== "rhai"`) | Libraries and scripts with capital extensions are not listed | `vector_io::paths::has_extension(p, &["csl"])` | 1 |
| D | `dockers/misc.rs:1304` `sync_links` (Bitmaps ▸ Update Link uses it, `dialogs/bitmaps.rs:236`) | Linked bitmaps are read from the stored path only; a drawing from Windows has `C:\…` paths | `vector_io::paths::resolve_stored_path(path, &[drawing_dir])` before reading | 1 |
| D | `dialogs/symmetry.rs:930` `open_ole` | Linked OLE object: `PathBuf::from(path)` of a Windows path | Same resolver | 1 |
| D | `vector-core/src/cms/mod.rs:262` `find_profile` | A profile stored as a Windows path is compared whole with the file names, so it is never found on Linux | Compare with the last part of the name split at `/` and `\` (vector-core cannot use vector-io) | 1 |
| D | `dockers/misc.rs:159` | Comment author from `USERNAME` | `crate::dockers::comment_author(&app.settings.user_name)` | 1 |
| D | `dockers/assets.rs:102` | `metadata.created()` is unsupported on some Linux file systems; every file then sorts as created in 1970 | Fall back to `modified()` | 1 |
| C | `settings.rs:703` | Default user name from `USERNAME` only | `crate::dockers::login_name()` | 1 |
| C | `settings.rs:927` (save) | serde cannot write a `PathBuf` that is not UTF-8 (`recent_files`, `last_dir`, …), so the whole settings file is not saved | Leave such paths out (or store them lossily) before saving | 1 |
| C | `sysinfo.rs:297` | AnyVector's own file is recognised as `anyvector.exe` / `*.exe` | Compare with `std::env::current_exe()` | 1 |
| B | `vector-core/src/text.rs:1307-1321` | Fallback font names for CJK, Thai and Devanagari are Windows and macOS families | Add Noto Sans CJK SC/TC/JP/KR, WenQuanYi Zen Hei, Droid Sans Fallback, Noto Sans Thai, Lohit Devanagari | 1 |
| B | text measuring in `vector-core/src/text.rs` | Two tests fail on Linux on unchanged main too, because text is measured with other metrics than Arial: `dialogs::object::tests::rollover_states_and_codes` (barcode caption wider than the 95 pt bars, `object.rs:999`) and `vector_io::corel::build::tests::a_coreldraw_4_chain_keeps_its_empty_frames_linked` (the overflow never reaches the second frame, `corel/build.rs:2255`). fontconfig maps Arial to Liberation Sans here, but the layout does not use that alias | Map Arial, Times New Roman and Courier New to their metric-compatible Liberation faces when they are missing; then both tests should pass unchanged | 2 |

## 2. The closed issues on Linux

Class **a**: the fix is in shared code, so Linux has it. Class **b**: the fix (or part of it) is
Windows-only code; the Linux side is described. Most issues have regression tests in
`anyvector/src/visual_tests.rs` (named after the issue) or `vector-core/tests`; they run in
`cargo test --workspace` on Linux too (results above).

| # | Title | Class | Linux |
| --- | --- | --- | --- |
| 33 | Help > Updates issue | b | Update check and notes are shared; downloading and running `*-setup.exe` is Windows-only (`updater.rs`, C). Linux: plan Phase 3 (AppImage replaces itself, packages only announce) |
| 34 | Report a bug without a GitHub account | a + b | Relay (HTTPS) is shared. The system lines say `Windows:` and run `cmd /C ver` (`bugreport.rs:170, 182`, D); plan Phase 1 item 5 |
| 47 | Test report from the issue relay | – | Not a defect |
| 52 | Only dark mode / impact tool | a + b | Light theme shared. Title-bar colour (DWM, `theme.rs`, A) has no Linux counterpart; plan row "Window border colour" |
| 55 | Guide lines | a | Shared |
| 56 | Opening Corel 18 files | a | Shared (ungroup speed) |
| 58 | Search option | a | Shared |
| 63 | Startup canvas size | a | The first document is fitted when the canvas gets its real size, which also covers Wayland/X11 resizing the window after start |
| 64 | Unicode text as squares | a | Font matching by every localized family name is in shared code (fontdb). Linux has other fonts installed (B) |
| 65 | Freehand tools | a | Shared |
| 66 | CorelDRAW 2022 type 31 | a | Shared |
| 67 | Fonts embedded in .cdr | a | Loaded from memory into fontdb; no system API |
| 68 | Colour management off | a | Shared |
| 69 | Import PDF | a | Pure Rust |
| 70 | Import EPS / PS | a | Pure Rust |
| 71 | JPEG XL | a | Pure Rust |
| 72 | More AI providers | a + b | Providers are HTTP (shared). The key comes from the Windows user environment and Save Key writes it there (`ai.rs`, C); plan row "AI settings"; see gap 5 below |
| 73 | ICC colour management | a + b | Engine shared; `vector-core/src/cms` already searches `/usr/share/color/icc` and `~/.local/share/icc`. `color_profiles.rs:19` still uses `WINDIR` (D, plan row) |
| 74 | Cutter output | a | Shared (files; device scripts in the config folder) |
| 75 | Paste into Office as SVG/EMF | b | Windows clipboard formats (`dialogs/clipboard.rs`, E); plan row "Clipboard" |
| 76 | Font manager | – | Became AnyFont, a separate Windows app; nothing in AnyVector |
| 79 | Rotate centre cannot be moved | a | Shared |
| 82 | Popup windows can't be moved | a | egui windows, shared |
| 83 | Rectangle corner rounding by dragging | a | Shared |
| 84 | Palette swatches dull with proofing off | a | Shared |
| 85 | Mouse wheel zoom steps | a | Checked: egui-winit gives one wheel notch as a line delta on X11 and Wayland as on Windows, so the step is the same; touchpads give pixel deltas on both |
| 88 | SVG export: text as text | a | Shared |
| 89 | DXF import | a | Shared; text-style font files given as Windows paths fixed for Linux here (section 1) |
| 90 | Colour management issues | a | Shared |
| 91 | Object rotation | a | Shared |
| 95 | Pantone | a | Palettes shared |
| 96 | Treat objects as filled toggle | a | Shared |
| 97 | Selection after block shadow | a | Shared |
| 98 | Break apart artistic text | a | Shared |
| 99 | Rectangle to page | a | Shared |
| 102 | Raster image tracing hang | a | Shared |
| 103 | Default outline properties | a | Shared |
| 104 | Polygon nodes | a | Shared |
| 105 | Break drop/block shadow | a | Shared |
| 106 | Click a shadow | a | Shared |
| 107 | Shadow property bar | a | Shared |
| 108 | Ellipse nodes | a | Shared |
| 109 | Default text units | a | Shared |
| 110 | Text spacing handles | a | Shared |
| 111 | Alt+click in a PowerClip | a | Shared, but see gap 6: on desktops where Alt+click/drag moves windows the click never reaches AnyVector |
| 112 | PowerClip options | a | Shared |
| 113 | Ruler subdivisions | a | Shared |
| 116 | Pick tool (treat as filled) | a | Shared |
| 117 | Virtual Segment Delete | a | Shared |
| 118 | Trim | a | Shared |
| 119 | Keypad + duplicates | a | Checked: egui-winit takes the key from the physical key code (`NumpadAdd`) on every platform, which is what AnyVector reads |
| 120 | Units and paper from regional settings | b | `GetLocaleInfoEx` (`settings.rs`, C); plan row "Units and paper size" (`LC_MEASUREMENT`, `LC_PAPER`) |
| 121 | Tab selects the field | a | Shared |
| 122 | Shape tool abnormalities | a | Shared (cursors are egui cursor icons, mapped by winit) |
| 123 | Shape tool add nodes | a | Shared |
| 124 | Workspace customization | a | Shared; Alt+drag of buttons meets the same desktop conflict as #111. Its test `issue_124_alt_drag_moves_copies_and_removes_buttons` stops in the epaint font panic on Linux until B's fix |
| 125 | Assign shortcut keys | a | Shared; some chords belong to the desktop (gap 7) |
| 126 | Centre of rotation | a | Shared |
| 127 | Hairline shown as no outline | a | Shared |
| 129 | Bitmap in wireframe | a | Shared |
| 130 | Space while dragging | a | Shared |
| 132 | Save in older CorelDRAW versions | a | Shared |
| 133 | Hairline outline | a | Shared |
| 134 | Freehand with Ctrl | a | Shared |
| 135 | Double-click Rectangle tool with a selection | a | Shared |
| 136 | Customization text garbled | a | Shared layout; Linux UI font metrics differ (B) |
| 137 | Outline units | a | Shared |
| 138 | Outline width in the status bar | a | Shared |
| 139 | Alt+click selects behind | a | Shared; desktop Alt conflict as #111 |
| 140 | Right-drag drop menu | a | Shared |
| 141 | Dark vs light, white desktop | a + b | Colours shared; window frame colour Windows-only (A) |
| 142 | Nudge segments | a | Shared |
| 144 | Rearrangeable dockers | a | Checked: dockers, also floating ones, are drawn inside the main window, not as separate OS windows, so Wayland's rules for window positions do not apply |
| 145 | Blend advanced operations | a | Shared |
| 146 | Colour variations on a held swatch | a | Shared (timer) |
| 147 | IME | a | egui's IME events come from winit on X11 (XIM) and Wayland (text-input-v3) as on Windows; still to try with IBus and Fcitx (plan section 3). CJK fallback fonts: B (section 1) |
| 149 | Blend start and end selection | a | Shared |
| 150 | Dimension text positions | a | Shared |
| 151 | Ctrl+click inside a group | a | Shared; six of its seven tests pass on Linux, `issue_151_ctrl_click_keeps_its_other_meanings` stops in the epaint font panic until B's fix |

Count: 74 class a (5 of them with a Windows-only part, marked a + b: #34, #52, #72, #73, #141),
3 class b (#33, #75, #120), 2 not code (#47, #76). Every Windows-only part is in plan section 2 or
in the gaps below.

## 3. Feature gaps not in plan section 2

Found by searching for `cfg(windows)`, `windows::`, `clipboard_win`, registry access, `%APPDATA%`,
`WINDIR`, `ProgramData`, `cmd`, `explorer` and `powershell`, and by checking `docs/COVERAGE.md` and
the CorelDRAW dossier (`Corel-ida`, feature catalogue F1-F20) for features that depend on the
system.

| # | Feature | Windows | Linux today | Suggestion | Owner | Phase |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | File ▸ Acquire Image (Select WIA Source, Acquire) | WIA through PowerShell (`exec.rs:187-220, 1350-1380`) | Runs `powershell` anyway and fails; Select TWAIN Source talks about Windows | SANE: `scanimage -L` for the device list, `scanimage --format=png` to scan (sane-utils), or hide the items when SANE is missing; TWAIN message for Linux | D | 2 |
| 2 | File ▸ Send To links | User's Send To folder | Was empty | Fixed here: Desktop, Documents, Mail Recipient. Mail needs the plan's `xdg-email` | – / D | 1 |
| 3 | Open in the default program | `cmd /C start` (`dialogs/bitmaps.rs:254` Edit Bitmap, `dockers/assets.rs:541`, `dockers/misc.rs:1393`, `exec.rs:1453` mailto); `explorer <dir>` (`dialogs/new_doc.rs:993`) | Nothing happens | `xdg-open` (as `dialogs/symmetry.rs:947` already does) | D | 1 |
| 4 | Linked files stored with Windows paths | Found where they were saved | Not found | `vector_io::paths::resolve_stored_path` (section 1: bitmaps, OLE, ICC names) | D | 1 |
| 5 | AI ▸ Save Key | Writes the key to the user's environment (registry) | Nowhere to write | A file under `~/.config/environment.d/` (read by systemd user sessions) or AnyVector's own settings / the Secret Service keyring | C | 1 |
| 6 | Alt+click / Alt+drag (#111, #124, #139) | Works | Xfce, MATE, Cinnamon and KDE Plasma 5 use Alt+drag to move windows and take the click (GNOME and Plasma 6 use Super) | Say so in the Linux notes; optionally a setting for another modifier | F (tools) | 4 |
| 7 | Shortcuts the desktop keeps | – | Chords the desktop uses never reach the app, for example Ctrl+Alt+T (terminal on Ubuntu), Alt+F7 / Alt+F8 (move / resize window), Ctrl+Alt+arrow keys (workspaces on several desktops), Super+… | List them in the Linux notes; warn in Customization ▸ Commands when such a chord is assigned | F | 4 |
| 8 | File names that are not UTF-8 | – | Start-up crash (fixed here); settings not saved (C, section 1) | – | C | 1 |
| 9 | Linux build warnings | – | Unused or dead code behind `cfg(windows)`: `bugreport.rs:191`, `exec.rs:312`, `dialogs/symmetry.rs:632, 654` (D); `sysinfo.rs:8, 12` (C); `printing.rs:664`, `save_dialog.rs:17-53` (F, until printing and the save options get their Linux versions) | Clean up before CI uses `-D warnings` | C, D, F | 1 |
| 10 | `.desktop` entry | – | – | `Exec=anyvector %F`: AnyVector takes file paths, not URIs | packaging | 3 |
| 11 | Legal notices | Text points to `legal\palettes.rs in the install folder` (`xtask/src/licenses.rs:13`) | Wrong place once packaged | Use the Linux package's documentation folder | F | 3 |
| 12 | Print to File message | Points to "Windows Settings > Bluetooth & devices > Printers & scanners" (`dialogs/print.rs:133`) | Shown when no printer is set up | Reword with the CUPS work | F | 2 |
| 13 | EMF playback test | `vector-io/src/emf_tests.rs:272-338` plays EMF with GDI | No reference player | Keep it Windows-only (as the plan says for such tests); the structural EMF tests run everywhere | – | – |

Nothing to port: AnyVector has no single-instance or jump-list code, no screen capture, no
command-line print modes and no COM/DDE automation (the dossier's F1, F12, F17); scripting (Rhai)
and pen pressure (simulated from speed) are platform-neutral.
