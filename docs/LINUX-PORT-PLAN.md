# AnyVector for Linux: assessment and roadmap

Status: assessment written 2026-11-23 (AnyVector 0.6.1, commit 7447911); Phase 0 and Phase 1 started the
same day (issue #51).

First Linux build (2026-11-23, WSL 2 Ubuntu 24.04, Rust 1.99.1, AnyVector 0.6.2): the whole workspace
builds with no code changes; 254 of 262 app tests and every engine test pass (the 8 failures: the UI
fonts read from `C:\Windows\Fonts`, and the Windows-only system information rows). The program runs and
its window works through X11 (XWayland); under Wayland the transparent splash window never appears.

## 1. Short answer

- **Yes, Rust runs on Linux.** Linux is a first-class Rust platform (Tier 1, the same level as
  Windows). The same source code compiles for both; there is no reason to switch languages.
- **Stay in Rust.** About 259,000 lines of Rust already exist, and almost all of them are
  platform-independent. A rewrite would throw that away for nothing.
- **AnyVector is already close to portable.** The earlier design choices were good ones:
  - The three engine crates (`vector-core`, `vector-render`, `vector-io`) are pure Rust with **no
    Windows dependency at all**. Drawing, rendering, every import/export (CDR, SVG, PDF, PS/EPS, DXF,
    EMF, images, JPEG XL...) and colour management work the same on Linux.
  - The user interface uses **egui/eframe on wgpu**, which runs on Linux (Vulkan or OpenGL, X11 and
    Wayland).
  - The other building blocks are cross-platform: `rfd` (file dialogs), `arboard` (clipboard),
    `dirs` (settings folders), `fontdb`/`rustybuzz` (fonts and text shaping), `ureq` (network),
    `rhai` (scripting).
  - The Windows-only code (the `windows` crate) sits in the `anyvector` crate only, in about 15 files,
    and is already behind `#[cfg(windows)]` with a fallback for other systems in most places (for
    example Save Drawing already falls back to the `rfd` dialog off Windows).

So the work is not "port the program" but "fill in about a dozen Windows-backed features with Linux
equivalents, then package and test it".

## 2. What is Windows-specific today

| Area | Windows today (file) | Linux today (fallback) | Linux plan | Effort |
| --- | --- | --- | --- | --- |
| UI fonts | Loads Segoe UI and friends from `C:\Windows\Fonts` (`theme.rs`, `icons.rs`) | Falls back to egui's built-in font; layout/metrics will differ | Bundle an open UI font (for example Noto Sans or Inter) or look it up through fontconfig; check that dialogs sized for Segoe UI still fit | S–M |
| Printing | Win32 printing, printer list, paper, DEVMODE preferences (`printing.rs`) | "Printing is available on Windows only" | Render the job to PDF (export already exists) and send it to CUPS (`lp` / IPP); printer list from `lpstat`; GTK print dialog optional later | M–L |
| Spell checking | Windows Spell Checking API, COM (`spell.rs`) | Not supported | Hunspell dictionaries from `/usr/share/hunspell` with a pure-Rust checker (for example `spellbook`) | M |
| Code pages (Text ▸ Encode) | Windows code page list (`spell.rs`) | Shorter list | Use `encoding_rs` (already a dependency) for the list | S |
| Clipboard, rich formats | CF_DIB, EMF, named formats, Office formats (`dialogs/clipboard.rs`) | SVG-as-text and images through `arboard` | Use MIME types (`image/svg+xml`, `image/png`) on X11 and Wayland; turn on arboard's Wayland support; EMF on the clipboard does not exist on Linux, so skip it | M |
| Save dialog with extras | Custom IFileSaveDialog with title, subject, rating, version, embed fonts (`save_dialog.rs`) | Plain `rfd` save dialog, extras keep their previous values | Keep `rfd` (it uses the xdg portal); put the extras in AnyVector's own small dialog after the file is chosen, if wanted | S–M |
| UI language | `GetUserDefaultUILanguage` (`i18n.rs`) | Default language | Read `LANGUAGE` / `LC_ALL` / `LC_MESSAGES` / `LANG` | S |
| Units and paper size | `GetLocaleInfoEx` (`settings.rs`) | Defaults | Read `LC_MEASUREMENT` / `LC_PAPER` (or decide from the locale's country) | S |
| Window border colour | DWM border/caption colour (`theme.rs`) | Nothing | Nothing needed; check how the title bar looks on GNOME (Wayland client-side decorations) and KDE | S |
| Colour profiles folder | `C:\Windows\System32\spool\drivers\color` (`color_profiles.rs`, `vector-core/src/cms`) | None found | `/usr/share/color/icc`, `~/.local/share/icc` (optionally ask colord for the monitor profile) | S |
| Eyedropper outside the window | Reads a screen pixel (`tools/interact.rs`) | Not available | X11: possible; Wayland forbids it, use the xdg-desktop-portal Screenshot/PickColor call | M (optional) |
| Send by e-mail | MAPI (`exec.rs`) | Not available | `xdg-email --attach` | S |
| Show in folder | `explorer /select,` (`exec.rs`, `bugreport.rs`) | Opens the parent folder in one place, nothing in another | D-Bus `org.freedesktop.FileManager1.ShowItems`, else `xdg-open` the folder | S |
| New file templates (symmetry dialog) | Registry `ShellNew`, `ProgramData` (`dialogs/symmetry.rs`) | Empty list | Use `~/Templates` (`dirs::template_dir`, already in the code) | S |
| AI settings from user environment | Registry (`ai.rs`) | None | Plain environment variables | S |
| System info (About / bug report) | Memory, GDI objects, monitors, printers, loaded DLLs (`sysinfo.rs`) | Just the OS name | `/proc/meminfo`, `/etc/os-release`, kernel, desktop session (X11/Wayland), GPU from wgpu | S |
| Updater | Downloads `*-setup.exe` from GitHub and runs it on exit (`updater.rs`) | Would report "no setup for Windows" | Depends on packaging (section 4): AppImage replaces its own file; .deb / Flatpak leave updates to the package manager and only show "a new version is out" | M |
| Installer | Inno Setup (`installer/AnyVector.iss`) | — | See section 4 | M |
| Windows resources (icon, version info) | `build.rs` with `winresource` | Already skipped off Windows | `.desktop` file and PNG/SVG icons instead | S |

Effort: S = hours to a day, M = a few days, L = a week or more.

## 3. Things that are not Windows code but still need checking on Linux

- **Wayland vs X11.** winit (under eframe) supports both. Test both: window decorations, HiDPI and
  fractional scaling, drag and drop from the file manager, multiple windows/dockers that float.
- **Keyboard and IME.** Shortcuts should match (Ctrl-based). Check the Alt key (menu mnemonics are
  handled differently by some desktops) and the IME added in 0.6.1 with IBus and Fcitx.
- **Case-sensitive file names.** Windows ignores case, Linux does not. Look for comparisons of file
  names and extensions that assume `.CDR == .cdr`, font file lookups by name, and paths built with
  `\`.
- **GPU drivers.** wgpu needs Vulkan or OpenGL ES; very old or virtual machines may only have
  software rendering (llvmpipe). Decide whether to allow eframe's `glow` (OpenGL) fallback.
- **Fonts available to drawings.** `fontdb` reads the system fonts through the standard folders; a
  CorelDRAW file that names Arial or Times New Roman will need a fallback (Liberation fonts are
  metric-compatible and present on most distributions).
- **Visual tests.** The `egui_kittest` snapshots were made on Windows. Font rendering differs, so
  Linux will need its own snapshots or looser thresholds. Headless CI needs Mesa's software Vulkan
  (lavapipe).
- **Tests with `#[cfg(windows)]`.** A few tests only make sense on Windows (real browser/Office
  clipboard formats, EMF); keep them Windows-only.

## 4. Packaging and distribution

Recommended order:

1. **AppImage** first: one file that runs on most distributions, no install step, easy to put on the
   GitHub release next to the Windows setup, and it can update itself (the updater downloads the new
   AppImage and replaces the old one). Build it on the oldest distribution to support (for example
   Ubuntu 22.04) so its glibc works on newer systems.
2. **.deb** (Ubuntu, Debian, Mint) with `cargo-deb`: installs a `.desktop` entry, icons in
   `/usr/share/icons/hicolor`, and MIME types so `.cdr`/`.svg`/AnyVector files open with a double click.
3. **Flatpak (Flathub)** later: the best reach and sandboxing, but it needs the portals (file
   dialogs, printing, screenshot/colour picking) to be used everywhere, which the plan above already
   prefers.
4. `.rpm` (Fedora) with `cargo-generate-rpm` if users ask.

Each package needs: a `.desktop` file, icons (16 to 512 px, plus SVG), MIME type definitions for the
file associations, AppStream metadata (`.metainfo.xml`) for software centres, and the third-party
licence file.

## 5. Build and test setup

- **Development machine.** Easiest on this PC: install **WSL 2 with Ubuntu** (WSLg shows Linux GUI
  windows on the Windows desktop, so AnyVector can be run and clicked). Add a real Linux machine or a
  VM later for Wayland/GNOME/KDE checks that WSLg does not cover. (None of this is installed yet.)
- **Quick compile check from Windows** (no Linux needed for the first pass):
  `rustup target add x86_64-unknown-linux-gnu` then `cargo check --target x86_64-unknown-linux-gnu`.
  This finds code that does not compile off Windows; it cannot link or run the program. Some
  dependencies with C code may need a Linux C compiler, in which case do this step in WSL instead.
- **CI.** Add an `ubuntu-latest` job to GitHub Actions: `cargo build`, `cargo test`, `cargo clippy`,
  later `cargo test -- --include-ignored` with lavapipe for the visual tests, and the AppImage/.deb
  build on release tags.
- **System packages the build needs on Ubuntu** (typical for eframe/wgpu/rfd/arboard):
  `build-essential pkg-config libxkbcommon-dev libwayland-dev libx11-dev libxcursor-dev libxrandr-dev
  libxi-dev libgtk-3-dev` (rfd's GTK backend; or use its xdg-portal backend and skip GTK) and
  `libvulkan1 mesa-vulkan-drivers` to run.

## 6. Roadmap

**Phase 0 – Does it build? (1–2 days)**
1. Set up WSL 2 + Ubuntu (or a Linux VM) and the build packages above.
2. `cargo build` and `cargo test` for the whole workspace on Linux; fix whatever does not compile
   (expected to be small, since the Windows code is already behind `cfg`).
3. Start AnyVector under WSLg; note what looks or behaves wrong.
4. Add the Linux job to CI so the build never breaks again silently.

**Phase 1 – Usable on Linux (about 1 week)**
1. UI fonts (bundle or fontconfig) and checking dialogs still fit.
2. Language, units and paper size from the locale.
3. Show in folder, send by e-mail, open URL, templates folder, AI environment variables.
4. Colour profile folders.
5. System info rows for the About box and bug reports (the bug reporter should say "Linux, Wayland,
   GNOME..." so Linux reports are useful).
6. Case-sensitivity and path audit.
7. Clipboard check on X11 and Wayland (copy/paste between AnyVector windows and with Inkscape,
   LibreOffice, browsers).

**Phase 2 – Feature parity (2–3 weeks)**
1. Printing through CUPS (PDF route).
2. Spell checking with Hunspell dictionaries.
3. Code page list from `encoding_rs`.
4. Eyedropper outside the window through the portal (X11 direct read as a bonus).
5. Optional: AnyVector's own "save options" dialog for the extras the Windows save dialog shows.
6. Linux visual-test snapshots and the test scenarios for every item above (per the "add tests for
   every feature" rule).

**Phase 3 – Packaging and release (about 1 week)**
1. `.desktop` file, icons, MIME types, AppStream metadata.
2. AppImage build in CI; `.deb` with `cargo-deb`.
3. Updater: find the asset for the running platform (`-setup.exe` on Windows, `.AppImage` on Linux)
   and replace the AppImage; packaged installs only announce new versions.
4. Release notes and README section "Installing on Linux"; publish the first Linux build as a beta
   next to a Windows release.

**Phase 4 – Polish (ongoing)**
- Test on Ubuntu (GNOME, Wayland), Fedora, KDE Plasma, and an X11 session; HiDPI and fractional
  scaling; tablets/pens.
- Flatpak on Flathub.
- Native print dialog (GTK/portal), colord monitor profile, desktop-theme following.

Rough total for a first public Linux beta: **4–6 weeks** of focused work, most of it in Phases 1–2.

## 7. Decisions to make

1. **Which distributions and desktops to support officially?** Suggestion: Ubuntu 22.04+ and Fedora
   (latest), GNOME and KDE, Wayland first with X11 still working.
2. **Which package first?** Suggestion: AppImage, then .deb, then Flatpak.
3. **Linux machine for testing:** WSL 2 on this PC only, or also a real Linux PC/VM?
4. **Printing scope for the first beta:** full CUPS printing, or "Print = export to PDF and open it"
   as a stopgap?
5. **Code structure:** keep `#[cfg(windows)]` blocks spread across files (as now), or gather the
   platform code into one `platform/` module with `windows.rs` and `linux.rs` (cleaner once there are
   two real implementations, and it prepares a future macOS port). Suggestion: the `platform/`
   module, done gradually as each feature gets its Linux version.
