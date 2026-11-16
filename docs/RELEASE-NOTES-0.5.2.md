# AnyVector 0.5.2

AnyVector 0.5.2 opens DXF drawings, can keep text as text in SVG files, and fixes the bugs reported after 0.5.1.

## Files

- **Open and import DXF drawings.** File ▸ Open and File ▸ Import read DXF files from AutoCAD R12 to 2018, at their
  real size and with their layers, colors and line widths. Lines, polylines and their arcs, circles, arcs, ellipses,
  curves, text, blocks, dimensions and solid fills come through. The import messages list anything left out, such as
  paper space or 3D objects. A drawing that doesn't say its units is read in millimeters. To open a DWG drawing, save
  it as DXF in your CAD program first.
- **Text as text in SVG files.** File ▸ Export as SVG has a new Export text choice. Text keeps the same font, size,
  color and spacing, and stays editable in other programs. Text on a path, vertical, warped or filled with anything
  but one color is still exported as curves.

## Fixes

- **Rotate around the moved center everywhere.** After moving an object's rotation center, the angle on the property
  bar, Rotate and the Transform docker turn the object around it too, not only the rotate handles. The center also
  stays with the object when you move it.
- **Bright palette colors with color management off.** The default palette is now CorelDRAW's CMYK palette, with the
  same 100 colors in the same order. Its swatches follow color management: bright with Simulate Color Management
  Off, and like CorelDRAW with the default settings. If you saved your own default palette, use Reset in the
  palette editor to get the new one.
