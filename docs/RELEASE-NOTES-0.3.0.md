# AnyVector 0.3.0

AnyVector 0.3.0 is the second round of CorelDRAW import. More drawings come in looking like CorelDRAW drew them, and
fills keep what they're made of so you can edit them. When a drawing comes in with differences, AnyVector says what
they are. You can also report bugs and send ideas without a GitHub account.

## CorelDRAW import

- **What came in differently.** File ▸ Open and File ▸ Import show a message listing what a drawing came in
  without, for example:
  - objects of a kind AnyVector doesn't read yet
  - a texture style that isn't in CorelDRAW's library
  - a font that isn't installed, and the font shown instead
- **Missing fonts.** A font that isn't installed is shown in one that looks like it:
  - CorelDRAW's own font names (Avant Garde, Switzerland, Dutch, Bedrock and others) map to the Windows fonts they
    copy
  - narrow fonts stay narrow, and "Arial Narrow" really uses Arial Narrow
- **Texture fills.** CorelDRAW 12 texture fills come in as textures, at the tile size saved in the file (they were
  gray). Cosmic Clouds and Cosmic Minerals come in as textures too.
- **Full-color pattern fills.** These come in with their drawing in CorelDRAW X5 to 2026 files and in Corel clipart
  (they were gray).
- **PostScript fills.** 37 of the 38 fills that came in as a plain tone are now drawn, sized to each object as
  CorelDRAW does. Fills with random parts (scattered shapes or colors) are always the same for a file, but they
  scatter differently from CorelDRAW.
- **Corel clipart (CMX).** CMX files open, including the clipart files named `.cdr`. That covers shapes, groups,
  layers, solid and gradient fills, outlines, simple text and bitmaps. CMX is listed in the File ▸ Open and
  File ▸ Import file types.
- **Text.**
  - Empty paragraph frames are kept, so templates keep the frames you type into.
  - CorelDRAW 3 text uses the drawing's own font when the drawing has a single font family.

## Fills you can edit

- **Full-color patterns:**
  - Patterns keep their objects. Properties ▸ Fill ▸ **Edit fill** opens the tile as its own document.
  - When you close it, the change reaches every object using that pattern. Edit ▸ Undo takes it back.
- **Old texture fills:** these keep their style and settings. The Properties docker shows them (texture number,
  settings, colors, brightness), and a change redraws the fill.
- **PostScript fills:** these keep their number and settings, shown in the Properties docker.
- **Object ▸ Create ▸ Pattern Fill:** a vector pattern made this way keeps its objects too.
- **Convert to bitmap pattern:** this turns any of these fills into a plain picture on purpose.
- **Saving:** all of these survive saving and reopening, as `.vct` or `.cdr`.

## Help menu

- **Report a Bug and Submit Ideas and Feedback** no longer need a GitHub account. AnyVector sends the report
  itself, and it shows up on the AnyVector project on GitHub. If that service can't be reached, GitHub's form opens in
  the browser, filled in, as before.
- **Help ▸ Updates** shows **Install Now** while a new version downloads in the background; it installs as soon as
  the download ends. The buttons also stay in view when the release notes are long.

## Known limits

- **Texture fills:**
  - Files older than CorelDRAW X3 come in as soft clouds in the texture's colors, not as CorelDRAW's exact drawing.
  - Texture fills of CorelDRAW 7 to 11 files haven't been checked on a real file.
- **Full-color pattern fills:**
  - CorelDRAW 5 files still come in gray.
  - CorelDRAW 6 to X4 files use the same code as X5 but haven't been checked on a real file.
- **PostScript fills:** their settings can be seen but not changed yet. One Motifs setting (the Corel logo) stays a
  plain tone.
- **Text:**
  - Text in a missing font isn't stretched to that font's widths, so lines can come out wider or narrower.
  - CorelDRAW 3 drawings with several fonts show their text in the default font.
- **CMX clipart:** texture, PostScript and bitmap fills, arrowheads and lenses come in gray or are left out.
- **Copying:** copying a pattern-filled object to another document drops its pattern.
- **Colors:** CMYK colors of files older than X4, or made with a CMYK profile CorelDRAW 2026 doesn't come with, use
  CorelDRAW's default color settings.
- **Arrowheads:** CorelDRAW 1 files don't bring their arrowheads.
- **Objects left out:** a few objects of CorelDRAW 5 to 10 files (some linked bitmaps and two rare kinds of object).
  The import message now names them.

## Legal notices

The colors of the fixed palettes come from the libcdr project and are used under the Mozilla Public License 2.0.
Help ▸ About AnyVector ▸ Legal Notices says where their source and license are. Both are also in the `legal`
folder of the installation.
