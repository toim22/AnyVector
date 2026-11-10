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
- Colors in RGB, CMYK, HSB, HLS, Lab, grayscale and registration, and colors of the fixed palettes (PANTONE,
  TRUMATCH, Focoltone and the others), with their tints.
- Artistic and paragraph text with its fonts, sizes, styles, colors and alignment, in every character set
  CorelDRAW used, including the one-byte Windows and Mac sets of old files.
- Embedded bitmaps, and lenses.
- Perspective and envelopes, applied to the shapes they hold.
- **Effects:**
  - Blends come in as live AnyVector blends between their two ends, with the same number of steps.
  - Extrusions come in as live extrusions toward the same vanishing point and to the same depth, with their
    colors. This covers all six kinds: small and big, back and front, and parallel.
  - Drop shadows that recent CorelDRAW versions save as a live effect (CorelDRAW 2026, for example) come in as live
    shadows with the same offset, feathering, color and opacity. Files that hold a picture of the shadow instead
    (CorelDRAW X5, for example) keep that picture.
  - Contours come in as the shapes CorelDRAW stored for each step.
- Arrowheads on lines keep their exact CorelDRAW shape and size. They are added to the document's arrowheads, so
  you can use them on other lines too.

## Known limits

- PostScript fills, vector pattern fills and texture fills come in as a plain tone, and two-color patterns as a
  mix of their two colors.
- Guidelines are not imported.
- Text on a path and the frame layout of paragraph text are approximate, and some text of CorelDRAW 9 files is
  missing.
- CMYK colors are converted without CorelDRAW's color profile, so they can look a little more saturated than in
  CorelDRAW.
- In CorelDRAW 1 and 2 files, only shapes, fills and outlines come in. Their bitmaps, text and arrowheads are
  left out.
- The sides of extrusions use AnyVector's own lighting, which can look different from CorelDRAW's shading.

## Legal notices

The colors of the fixed palettes come from the libcdr project and are used under the Mozilla Public License 2.0.
Help ▸ About AnyVector ▸ Legal Notices says where their source and license are. Both are also in the `legal`
folder of the installation.
