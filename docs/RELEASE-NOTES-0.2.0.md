# AnyVector 0.2.0

AnyVector 0.2.0 opens drawings saved by CorelDRAW, from CorelDRAW 1 (1989) to CorelDRAW 2026, as ordinary
AnyVector documents. There is no conversion step and nothing to export from CorelDRAW first: File ▸ Open or File ▸
Import with a .cdr file is enough. The original file is never saved over; File ▸ Save asks for a new name.

## What comes in from a CorelDRAW file

- Every version of the format: CorelDRAW 1 and 2, the RIFF files of CorelDRAW 3 to X3, the compressed files of
  X4 to X7, and the zipped files of 2017 to 2026.
- Pages with their sizes, layers with their visible, printable and locked states, and master layers.
- Rectangles with round, scalloped and chamfered corners, ellipses, arcs and pies, polygons, stars, curves,
  groups and PowerClips, in the position, size, rotation and skew CorelDRAW gave them.
- Uniform, fountain and bitmap fills; outlines with their width, color, dashes, line caps,
  corners and scaling; transparency.
- Two-color pattern fills with their own pattern, tile size and two colors.
- Texture fills and vector pattern fills, shown with the picture of the tile that CorelDRAW stored in the file.
- PostScript fills. Bricks, Circle Grid, Crosshatching, Fishscale, Hatching, Hexagons, Octagons, Squares, Star of David,
  Triangle and Waves, and the color Hatching, Crosshatching and Fishscale, are drawn
  again from their settings: frequency, line width, grays and angle. The other PostScript fills take the average
  tone they have in CorelDRAW.
- Colors in RGB, CMYK, HSB, HLS, Lab, grayscale and registration, and colors of the fixed palettes (PANTONE,
  TRUMATCH, Focoltone and the others), with their tints. CMYK colors look as they do on screen in CorelDRAW,
  through the drawing's own CMYK color profile when it is one of the 32 that CorelDRAW 2026 comes with (X4
  and later files record it).
- Artistic and paragraph text with its fonts, sizes, styles, colors, underline, strikethrough and overline, in every
  character set CorelDRAW used, including the one-byte Windows and Mac sets of old files.
- Paragraph text keeps its frame layout: each paragraph's own alignment and indents, and the frame's columns
  with their widths and gutters.
- Text fitted to a path stays live on its path: on curves at the same place (left, right, centered or at an
  offset), distance and side, and on ellipses centered on the same side.
- Embedded bitmaps, and lenses.
- Perspective and envelopes, applied to the shapes they hold.
- **Effects:**
  - Blends come in as live AnyVector blends between their two ends, with the same number of steps.
  - Extrusions come in as live extrusions toward the same vanishing point and to the same depth, with their
    colors and CorelDRAW's shading of their sides. This covers all six kinds: small and big, back and front,
    and parallel.
  - Drop shadows that recent CorelDRAW versions save as a live effect (CorelDRAW 2026, for example) come in as live
    shadows with the same offset, feathering, color and opacity. Files that hold a picture of the shadow instead
    (CorelDRAW X5, for example) keep that picture.
  - Contours come in as the shapes CorelDRAW stored for each step.
- Guidelines come in as AnyVector guides: horizontal, vertical and angled, on the master page or on their own
  page (CorelDRAW 6 and later).
- Arrowheads on lines keep their exact CorelDRAW shape and size. They are added to the document's arrowheads, so
  you can use them on other lines too. This includes CorelDRAW 2 drawings.

## Known limits

- The other PostScript fills (Birds, Leaves, Spokes and the rest that don't repeat) come in as a plain tone. The color
  fills use a fixed set of colors where CorelDRAW picks them at random.
- Bitmap, vector pattern and texture fills of files older than CorelDRAW X3 come in as a gray tone.
- Vector pattern and texture fills come in as a picture of their tile, so their objects and texture settings
  can't be edited.
- CMYK colors of files older than X4, and of drawings made with a CMYK profile CorelDRAW 2026 doesn't come with,
  use CorelDRAW's default color settings.
- In CorelDRAW 1 and 2 files, text and bitmaps are left out. Arrowheads come in from CorelDRAW 2 files, but
  not from CorelDRAW 1 files.

## Legal notices

The colors of the fixed palettes come from the libcdr project and are used under the Mozilla Public License 2.0.
Help ▸ About AnyVector ▸ Legal Notices says where their source and license are. Both are also in the `legal`
folder of the installation.
