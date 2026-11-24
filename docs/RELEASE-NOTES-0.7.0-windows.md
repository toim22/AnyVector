# AnyVector 0.7.0 for Windows

AnyVector now runs on Linux too, with its own version numbers: see AnyVector 0.1.0 for Linux. This
version fixes problems on Windows that came up while bringing it there.

## New

- **Versions per system.** AnyVector for Windows and AnyVector for Linux now have their own version
  numbers. You are only told of new versions for your system.
- **Reports name the system.** Help ▸ Report a Bug and Submit Ideas and Feedback always say which
  AnyVector they come from: Windows, Linux or macOS.
- **File names.** The setup's name says what it is for: `AnyVector-0.7.0-windows-x64-setup.exe`.

## Fixed

- **Status bar.** In a narrow window the tool hint no longer covers the fill and outline. It is cut
  short, and the object information stays whole as long as possible.
- **Moved files are found.** Symbol libraries, Print Merge data, linked files and color profiles that
  a drawing names in a folder of another computer are looked for next to the drawing.
- **Images linked from an SVG** are read from the SVG's own folder.
- **Extensions in capitals.** Drawings such as `LOGO.CDR` no longer export as `LOGO.CDR.pdf`, and
  print styles ending in `.PRS` are listed.
- **Print Merge** Sync reads CSV files saved in the Windows code page, as Excel saves them.
- **Web addresses and file names with `&`** open whole, in bug reports and elsewhere.
- **Settings** are no longer damaged when the computer stops while they are being saved.
- **AI settings.** Save Key no longer stops AnyVector when the variable's name contains `=`.
