# AnyVector 0.7.0

AnyVector now runs on Linux, as a beta. This version also fixes problems on Windows that came up
while bringing it to Linux.

## AnyVector for Linux (beta)

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

## Fixed on Windows and Linux

- **Status bar.** In a narrow window the tool hint no longer covers the fill and outline. It is cut
  short, and the object information stays whole as long as possible.
- **Updates.** AnyVector offers the newest version made for your system, so a fix for one system can
  be released without an update for the other.
- **Moved files are found.** Symbol libraries, Print Merge data, linked files and color profiles that
  a drawing names in a folder of another computer are looked for next to the drawing.
- **Images linked from an SVG** are read from the SVG's own folder.
- **Extensions in capitals.** Drawings such as `LOGO.CDR` no longer export as `LOGO.CDR.pdf`, and
  print styles ending in `.PRS` are listed.
- **Print Merge** Sync reads CSV files saved in the Windows code page, as Excel saves them.
- **Web addresses and file names with `&`** open whole, in bug reports and elsewhere.
- **Settings** are no longer damaged when the computer stops while they are being saved.
- **AI settings.** Save Key no longer stops AnyVector when the variable's name contains `=`.
