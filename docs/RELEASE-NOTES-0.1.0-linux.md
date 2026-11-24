# AnyVector 0.1.0 for Linux (beta)

The first AnyVector for Linux, as a beta. It is made from the same code as AnyVector 0.7.0 for Windows.

## AnyVector for Linux

- **AnyVector now runs on Linux (#51).** It has the same menus, tools, dockers, dialogs, languages and
  file formats as the Windows version, on X11 and Wayland.
- **Two ways to install it.** The AppImage runs on any 64-bit distribution with glibc 2.35 or later
  and updates itself. The `.deb` package for Debian and Ubuntu adds AnyVector to the applications
  menu and to "Open with".
- **Printing** goes through CUPS, with AnyVector's own Printing Preferences.
- **Spell check** uses the system's Hunspell dictionaries.
- **Scanning** uses SANE, and the Color Eyedropper picks colors outside the window.
- **Fonts.** Drawings that use Arial, Times New Roman, Courier New, Calibri or Cambria keep their
  layout with the free fonts of the same widths when those are installed.

Known limits of the beta:

- Files can't be dragged into the window on Wayland. They can on X11.
- File dialogs need `xdg-desktop-portal` or `zenity`.
- Copied objects are lost when AnyVector closes on Wayland, unless a clipboard manager is running.
- Printing and scanning haven't been tried with every device, and KDE hasn't been tried yet. Please
  report what doesn't work.

## Versions

- **Versions per system.** AnyVector for Windows and AnyVector for Linux now have their own version
  numbers. You are only told of new versions for your system.
- **Reports name the system.** Help ▸ Report a Bug and Submit Ideas and Feedback always say which
  AnyVector they come from: Windows, Linux or macOS.
- **Coming from the 0.7.0 beta.** The first Linux files, published on 2026-11-24 as 0.7.0, were
  replaced by this version. Download it once by hand; `apt` needs `--allow-downgrades` for it.
