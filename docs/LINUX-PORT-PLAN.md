# AnyVector for Linux: assessment and roadmap

Status: assessment written 2026-11-23 (AnyVector 0.6.1, commit 7447911). **Phase 0 and Phase 1 done**
the same day (issue #51, PR #153) and **Phase 2 done** 2026-11-24 (printing, spell check, code pages,
eyedropper outside the window, scanners, save options): section 8 says what was built. Phase 3
(packages and updater) is built too; the beta comes with AnyVector 0.7.0. The audit of file names, the 79
closed issues and feature gaps is in [LINUX-AUDIT.md](LINUX-AUDIT.md).

First Linux build (2026-11-23, WSL 2 Ubuntu 24.04, Rust 1.99.1, AnyVector 0.6.2): the whole workspace
built with no code changes; 254 of 262 app tests and every engine test passed (the 8 failures: the UI
fonts read from `C:\Windows\Fonts`, and the Windows-only system information rows). The window was first
thought missing under Wayland; it was there (X11 tools cannot see Wayland windows), see section 8.

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
2. AppImage build in CI; `.deb` (built with `dpkg-deb`, not `cargo-deb`: no extra tool to install).
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

## 8. What Phases 0 to 3 built (from 2026-11-23, PRs #153 and #154)

The Linux code sits behind `cfg`, so the Windows program is unchanged, apart from the shared bug
fixes listed at the end of this section.

**Build and CI.** `.github/workflows/build.yml` builds and tests on `windows-latest` and `ubuntu-22.04`
(the oldest supported Ubuntu, so the binary runs on newer glibc). Local Linux development and
testing: WSL 2 with Ubuntu 24.04 (WSLg shows the Linux window on the Windows desktop). On Linux the
test harnesses use Vulkan only (Mesa's lavapipe) when a Vulkan adapter exists: with GL as well,
every harness also made an EGL context, and Mesa's software GL on Ubuntu 22.04 crashed the test
program when several did that at once. Set `WGPU_BACKEND` to choose another backend.

**Window and graphics** (`main.rs`, `splash.rs`, `renderer.rs`).
- Wayland: the window does appear (WSLg, Weston 13, sway 1.9, mutter 46). Real bug fixed: winit 0.30
  keeps a non-resizable window's size as its limits, so after the 620×380 splash the main window still
  advertised a 620×415 maximum; on Linux the switch now re-sends min 480×320 and no maximum.
- X11: the start window is opaque, so the splash has square corners there (round on Wayland and
  Windows). Mesa's software Vulkan (lavapipe) in Ubuntu 22.04 (23.2) and Debian 12 (22.3) draws nothing
  into an X11 window with an alpha channel, so with no graphics driver the whole window stayed black;
  Ubuntu 24.04's (25.2) draws either way. CI's `smoke-test.sh` now fails on a blank window.
- X11, GNOME: mutter 46 un-maximizes the main window while it adds the title bar; AnyVector asks again
  (`MaximizeAgain`).
- OpenGL fallback: if wgpu cannot start, AnyVector restarts itself with eframe's glow renderer
  (`ANYVECTOR_RENDERER=gl` forces it).
- App id `io.github.toim22.AnyVector` (Wayland app_id, X11 WM_CLASS): the .desktop file must have this
  name (Phase 3). The title is sent only when it changes. Floating dockers and dialogs are drawn inside
  the main window, so Wayland's no-positioning rule does not affect them.

**Fonts** (`ui_fonts.rs`, vector-core `sysfonts.rs`, `fontsub.rs`).
- Interface: Segoe UI when installed, otherwise the bundled **Selawik** (Microsoft, OFL-1.1, Segoe UI's
  metrics within 1%, ~130 KB, embedded only in Linux builds). Fallbacks: Noto Sans, DejaVu, Liberation;
  symbol faces; CJK by interface language; egui's own fonts last, so no family is ever empty.
- Drawings: a missing Windows font uses its metric twin (Arial/Helvetica → Liberation Sans, Times New
  Roman → Liberation Serif, Courier New → Liberation Mono, Calibri → Carlito, Cambria → Caladea), and the
  substitution is reported. Fonts are read from all XDG folders; SVG import uses the same aliases.
- Recommended packages: fonts-liberation, fonts-crosextra-carlito, fonts-crosextra-caladea,
  fonts-dejavu-core, fonts-noto-core, fonts-noto-cjk (or fonts-wqy-microhei).

**Locale, System Info, updates, AI keys** (`i18n.rs`, `settings.rs`, `sysinfo.rs`, `updater.rs`, `ai.rs`).
- Language by gettext's rules (`LANGUAGE`, `LC_ALL`, `LC_MESSAGES`, `LANG`; C/POSIX = English) with
  Windows' language mapping. Units and paper (first run only) from the `LC_MEASUREMENT`/`LC_PAPER`
  territory, checked against glibc 2.39 (Letter in the Americas list, US units for US only).
- System Info: os-release, /proc, WSL detection, XDG desktop and session, xrandr or /sys/class/drm,
  `df`, CUPS `lpstat`, loaded libraries from /proc/self/maps; `sysinfo::platform_summary()` is the bug
  report's system line.
- Updates: see Updater below.
- AI keys: `~/.config/environment.d/anyvector.conf` (mode 0600) plus `systemctl --user
  import-environment`; AnyVector reads the file itself, so the key works at once.

**Desktop integration** (`platform.rs`).

| Action | Windows (unchanged) | Linux |
| --- | --- | --- |
| Open a page, file or folder | `cmd /C start`, `explorer` | `xdg-open` |
| Show in folder | `explorer /select,` | D-Bus `org.freedesktop.FileManager1.ShowItems`, else open the folder |
| Send To ▸ Mail Recipient | Simple MAPI | `xdg-email --attach`; on failure show the file and open `mailto:` |
| File ▸ Send To list | the SendTo folder | Desktop, Documents, Mail Recipient |
| Acquire Image | WIA | SANE (`scanimage`), see Scanning below |
| Insert Object ▸ Create New | ShellNew registry | XDG templates folder and KDE template entries |
| Colour profiles | Windows color folder | XDG ICC folders and colord, 3 levels deep |
| Temporary files, auto-backups | `%TEMP%` | `$XDG_RUNTIME_DIR/AnyVector` (0700), `~/.local/state/AnyVector/Backups` |

Runtime needs xdg-utils and dbus-send or gdbus (every desktop install has them).

**Clipboard and drag and drop** (`dialogs/clipboard/linux.rs`, `mime.rs`).
- Wayland data-control through wl-clipboard-rs, otherwise the X11 CLIPBOARD through x11rb (also used on
  GNOME and Weston through XWayland), chosen once. No new crates (already in the tree).
- Copy offers AnyVector's own format (`application/x-anyvector-graphic`, lossless between AnyVector
  windows), `image/svg+xml` and `image/png`. Paste takes, in Windows' order: own → SVG (also Inkscape's)
  → text → LibreOffice EMF → bitmap; copied files open or import like a drop.
- Drag and drop of files works on X11; native Wayland has none with winit 0.30 (it comes with winit
  0.31; until then `WAYLAND_DISPLAY= anyvector` runs under XWayland).
- Display tests: `cargo test -p anyvector clip_real_linux -- --ignored --test-threads=1` (needs xvfb,
  xclip, wl-clipboard, sway, weston).

**File names and stored paths** (`vector_io::paths`). Extensions match in any case; Linux file
dialogs get any-case patterns; links stored as Windows paths (bitmaps, symbol libraries, data sources,
OLE objects, colour profiles, DXF fonts) are found next to the drawing by file name; non-UTF-8 file
names no longer panic on the command line or lose the settings file.

**Printing** (Phase 2; `printing.rs`, `printing/cups.rs`, `dialogs/print.rs`). File ▸ Print, Print Preview
and Print Merge print through CUPS with its command-line programs only (no libcups): `lpstat` lists the
printers and the default one, `lpoptions -l` gives the choices (PPD or IPP names), and the PPD the
scheduler serves over HTTP gives the paper sizes and printable areas (without it, the size comes from the
name, with quarter-inch margins).
- The job is a PDF with one page image per sheet, made from the same bands Windows sends to GDI, so
  marks, tiles, mirror, invert and grayscale are identical. `lp` gets the copies, collation, the chosen
  options and `print-scaling=none`; the status message shows the CUPS job.
- Preferences... opens AnyVector's own Printing Preferences (paper size, orientation, both sides, color,
  resolution, copies, as the printer offers), because CUPS drivers have no dialog. The choices are kept
  as `key=value` lines where Windows keeps a DEVMODE, until the Print dialog closes.
- Print to file writes that PDF (Windows writes the driver's `.prn`).
- Limits: raster pages, as on Windows; CUPS on a socket only (no `localhost:631`) gives no printable
  areas; a `ServerName` in `client.conf` is not read for the PPD (`CUPS_SERVER` is).
- Real job test: `cargo test -p anyvector print_job_to_cups_pdf -- --ignored` (needs cups and
  printer-driver-cups-pdf).

**Save options** (Phase 2, `dialogs/save_options.rs`). On Windows, Save As is the system dialog with
AnyVector's controls in it (`save_dialog.rs`). rfd cannot add controls, so on Linux Save As has two
steps: the system dialog (the desktop portal, or zenity without one) picks the folder, name and type,
then AnyVector's **Save Drawing** dialog shows the rest, with the Windows labels, defaults and rules.
- File name, folder and type. Browse... goes back to the system dialog; the type can be changed here,
  because rfd does not say which filter was chosen (the extension gives the type).
- Title, Subject and Rating for every type. For CDR and VCT: Embed color profiles, Embed fonts, Version
  and Advanced... (Advanced Save Options); for CMX, PDF, SVG and EMF the "saves a copy" note instead.
- A name the system dialog did not ask about (extension added, type changed) is not replaced unasked.
- Save runs the same code as the Windows result (`save_with`); Cancel saves nothing.
- The dialog is drawn over frames, so the save returns `SaveOutcome::Pending` and the dialog carries
  what comes next (`AfterSave`): Close and Close All go on after Save, Exit saves the next drawing and
  then quits, Cancel stops them. The main window does not close while the dialog is open.
- File ▸ Export needs nothing: it is AnyVector's own dialog on both systems.
- Tests give the file names instead of opening rfd (`give_picked_paths`), so the flow is tested on
  Windows too; `visual_save_options_dialog` renders the dialogs.

**Spell check and code pages (Phase 2, `spell/hunspell.rs`, `spell/codepages.rs`).**
- Dictionaries: the system's Hunspell ones, read by `spellbook` (pure Rust, MPL-2.0; no libhunspell),
  looked for in `$DICPATH`, `~/.local/share/hunspell`, `hunspell` in each `$XDG_DATA_DIRS` folder and
  the old MySpell folders. A language tag takes its own dictionary (pt-BR → `pt_BR`), else the
  language's (`pt`), else another country's (`pt_PT`; for English `en_US` first). Other character sets
  (`SET ISO8859-1`, KOI8-R...) are converted on loading; each dictionary is loaded once.
- Checking stops where the Windows checker does (its answers, asked on Windows 11, are a test): words
  with digits, words in capitals, addresses, #tags and file names are not checked; of a hyphenated word
  only the wrong part is marked; a repeated word is offered for deletion.
- Add keeps the word in `~/.config/AnyVector/Spelling/<language>/default.dic` and Auto Replace the pair
  in `default.acl` (Windows: `%AppData%\Microsoft\Spelling\<language>`); Skip All lasts until AnyVector
  closes. Without a dictionary the Spell Checker's "Not found" line names the package to install
  (`hunspell-en-us`, `hunspell-pt-br`, `hunspell-fr`... as Debian and Ubuntu call them).
- Text ▸ Encode lists 64 of Windows' code pages with Windows' names and order: the ANSI, OEM, Mac,
  ISO 8859 and KOI8 pages and the East Asian ones, by `encoding_rs` and, for the OEM and Mac pages it
  lacks, `spell/upper.rs`. Not there: EBCDIC, ISCII, the 7-bit pages, Johab and the old Taiwanese and
  Mac double-byte pages. A Windows test compares the names and conversions with Windows' own.
  Characters a page lacks become "?" (Windows first tries a look-alike, such as "Ł" → "L").
- The ANSI code page is the one Windows gives the locale's language (`LC_ALL`, `LC_CTYPE`, `LANG`): 1250
  for Czech and Polish, 1251 for Russian, 932 for Japanese, 1252 for C and Western languages.
- The Legal Notices now also list the crates only the Linux build links.

**Eyedropper outside the window (Phase 2)** (`screen_pick.rs`). The Color Eyedropper's Select from
Desktop button.
- Wayland: the desktop's own color picker (xdg-desktop-portal `Screenshot.PickColor`, version 2, through
  zbus, already in the tree) opens as soon as the button is pressed. X11: the pixel clicked is read from
  the root window (`GetImage`, any visual) while the button is polled, as Windows does with GDI. Chosen as
  the clipboard is: the portal under Wayland, else X11. Under XWayland only X11 programs' windows can be
  sampled, and AnyVector says so; with neither way, a message says what to install.
- Clicks inside the window stay with the drawing's eyedropper. The eyedropper dragged out of the drawing
  and let go outside the window samples the screen too.
- The portal is asked from a thread; calls it should answer at once have a 5 s limit (8 s in all until
  its picker is open), then it counts as missing. Only the user's click is waited for without limit;
  turning the button off, or leaving AnyVector, closes the connection, and with it the picker.
- Tests with a display or a bus of their own (Xvfb; a fake portal on dbus-daemon) run with the clipboard's:
  `cargo test -p anyvector clip_real_linux -- --ignored --test-threads=1`. All of these are named
  `clip_real_linux_*`, as several change the environment of the whole test program: the visual set leaves
  them out (`cargo test -p anyvector visual_ -- --ignored --skip clip_real`).

**Scanning (Phase 2)** (`dialogs/scan.rs`, `dialogs/sane.rs`). File ▸ Acquire Image through SANE's
`scanimage` (sane-utils).
- Select WIA Source and Select TWAIN Source both list the scanners (`scanimage -L`); the chosen one is
  kept in the settings. Acquire asks for the resolution and color mode, from the scanner's own options
  (`scanimage -A`), scans in the background to a PNG in the private temporary folder and imports it as
  the WIA scan is. Without `scanimage` or a scanner a message says what to install, and mentions Document
  Scanner (simple-scan) when it is there.
- A real scan with SANE's virtual test scanner is one of the `clip_real_linux` tests (needs sane-utils).

**Packages (Phase 3, `packaging/linux`).** `build.sh` writes both packages to `dist/`; the `linux` job of
`build.yml` builds, checks, installs and starts them and keeps them as the artifact
`AnyVector-linux-packages`, and `linux-install` starts the .deb in Ubuntu 22.04, 24.04 and Debian 12
containers. Nothing is published by CI: both files are added to a release by hand.
- `AnyVector-<version>-x86_64.AppImage`: a squashfs behind the official static runtime (pinned by its
  checksum). AppRun is the program itself, with no script and no `LD_LIBRARY_PATH`, so the programs
  AnyVector starts keep the system's environment. It needs `fusermount3`, or
  `--appimage-extract-and-run`; it registers no file types.
- `anyvector_<version>_amd64.deb`, made with `dpkg-deb`. The window, graphics and D-Bus libraries are
  opened at run time, so `control.in` lists them by hand with the reason for each. It depends on
  `xdg-desktop-portal | zenity`, because file dialogs need one of them.
- File types: AnyVector owns only `.vct`. The other formats it opens are in a hidden second desktop file
  (`zz-...-open.desktop`) that sorts last, so "Open with" offers AnyVector but it never becomes the
  default for another program's files (as with the Windows setup).
- The legal files are in `/usr/share/doc/anyvector/legal`. `cargo run -p xtask -- brand` draws the icons.
- A new version needs its line (version and date) in `io.github.toim22.AnyVector.metainfo.xml`; the test
  `metainfo_lists_this_version_first` fails until it is there.

**Updater (Phase 3, `updater.rs`, `updater/appimage.rs`).** The AppImage replaces itself; a .deb, a plain
program or an AppImage the user can't write is only told of new versions, with **Open Release Page**.
AnyVector never uses sudo and never downloads the .deb.
- `APPIMAGE` and `APPDIR` are inherited by child programs, so they are not trusted alone: the program must
  be inside `$APPDIR`, `$APPIMAGE` must be a type 2 AppImage for this processor outside it, and the runtime
  must have made that folder (a FUSE mount, or the parent process when extracted).
- The new version is the highest one in the release list with files for the system (Windows: the setup;
  Linux: an AppImage or a `.deb`), so a release may be for one system only; it still takes the next
  number. Publish one without a setup with "Set as the latest release" off: AnyVector up to 0.6.2 reads
  only the latest release and needs a setup in it.
- Only `AnyVector-<version>-<arch>.AppImage` of the release is taken. Its size, GitHub's sha256 digest and
  its header are checked after the download, when an earlier download is reused, and while it is copied. A
  release without a digest is only announced.
- It is replaced when AnyVector closes, after the drawings are saved: a copy in the same folder, fsync, an
  atomic rename. Links, the user's file name and the permissions stay. On any failure the old file is left
  alone and the next AnyVector window says why.
- The new one starts when the old process has ended, with the AppImage variables removed.
  `ANYVECTOR_RENDERER_FALLBACK` marks a renderer AnyVector chose itself, so the new version tries wgpu again.
- Downloads are kept in `~/.cache/AnyVector/Updates`. The end-to-end tests use a local HTTP server.

**Shared bug fixes (also change Windows).**
- `cmd /C start` targets with `&` (bug-report URLs, file names) were split and could run commands; the
  target now goes through an environment variable, and the mail subject is fully percent-encoded.
- Extensions in capitals (`.CDR`, `.PRS`, `.JSON` presets) are accepted; moved symbol libraries, data
  sources, linked files and profiles are found next to the drawing; code-page CSV files sync.
- `settings.json` is written through a temporary file, so a crash cannot truncate it; a variable name
  with `=` or NUL no longer crashes AI Save Key.

**Known limits after Phase 3.** No native-Wayland drag and drop (winit 0.30); clipboard contents vanish
on exit under Wayland without a clipboard manager; a program AnyVector opened (such as the browser) keeps
the old AppImage's mount until it closes. "Open with" lists AnyVector twice for the formats of the second
desktop file (GNOME: Recommended and Other Apps). Not tried yet: a physical printer or scanner, KDE.

**Checked on a real desktop** (2026-11-24, Ubuntu 24.04.5 in a QEMU virtual machine, GNOME 46, the packages CI
built): the `.deb` installed by apt, its menu entry and icon; on Wayland and X11 opening a `.cdr`, the Open
dialog, the color picker, copied objects read by a GTK program and the AppImage (mounted); on Wayland also the
save dialog, pasting text and opening files from Files; on X11 also dragging a file into the window. It found
the two X11 window bugs above.
