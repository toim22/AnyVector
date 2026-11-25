# AnyVector 0.1.1 for Linux (beta)

This version fixes the AppImage on systems that lack some of the window's libraries. If AnyVector 0.1.0
opens on your computer, nothing else changes.

## Fixed

- **The AppImage starts on more systems.** It closed at once where `libxkbcommon-x11` was not installed,
  with "Library libxkbcommon-x11.so could not be loaded". It now carries this and the other keyboard
  and X11 libraries it needs, and uses them only where the system has none of its own.

The `.deb` package already installed these libraries, so it is the same as 0.1.0 otherwise.
