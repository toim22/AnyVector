# Vector — Status

Last updated: 2026-11-08.

## Working (verified by build and tests)

- Full workspace compiles. Unit tests pass for:
  - common-shape geometry
  - tracing (two squares)
  - case conversion
  - core and IO tests
- Headless GPU snapshot tests render these screens without errors:
  - Home screen
  - a document
  - the Shape tool
  - the Options dialog
- The tool smoke test drags and clicks with every one of the 61 tools on a real canvas. No tool panics. The test checks two things:
  - dragging with the Rectangle and Ellipse tools creates shapes
  - typing after a Text tool click creates text
- Main window layout follows the screenshots:
  - menu bar
  - standard toolbar
  - property bar
  - toolbox with flyouts
  - Quick Customize
  - document tabs
  - rulers
  - page navigator
  - palette
  - document palette
  - status bar
- Options dialog: every category and page from the screenshots, with "CorelDRAW" renamed to "Vector" and VBA renamed to Scripts.
- Branding: the airship image is used for the app icon (embedded in the exe), the splash screen, the About box and the installer wizard images.

## Implemented, checked only by the smoke test (not hand-tested)

- **Pick:** select, marquee, Shift add, move, scale, rotate and skew handles, copy-on-right-click.
- **Shape:** nodes, control handles, node marquee, double-click adds a node, rectangle corner rounding, node operations from the property bar.
- **Edit shapes:** brushes (smooth, smear, twirl, attract/repel, smudge, roughen), crop, knife, virtual segment delete, eraser.
- **Curves:** freehand, 2-point, Bézier, Pen, B-Spline, Polyline, 3-point curve, LiveSketch, Smart Drawing (recognizes lines, triangles, rectangles, polygons and ellipses), Artistic Media and Painterly brush.
- **Shapes:** rectangle, 3-point rectangle, ellipse, 3-point ellipse, polygon, star, spiral, graph paper, common shapes.
- **Text and tables:** artistic and paragraph text with caret, selection, arrow, Home/End and word navigation, copy/cut/paste and coalesced undo; tables with cell selection.
- **Dimensions:** parallel, horizontal/vertical, angular, segment, callout.
- **Connectors:** straight, right-angle and rounded, all attaching to objects; edit anchor.
- **Effects:** drop shadow, inner shadow, block shadow, contour, blend, distort, envelope, extrude, transparency.
- **Fills and eyedroppers:** color and attributes eyedroppers, interactive fill, mesh fill, smart fill.
- **Snapping:** objects, page, guidelines and grid.
- **Rulers:** guidelines dragged from the rulers, moving the ruler origin.
- **Pages and display:** page sorter, full-screen preview, Pixels view.

## Known gaps

- **Gradient handles:** the handles of interactive fills and transparencies show but cannot be dragged after creation. Drag again to redefine them.
- **Mesh fill:** picking a patch works, but setting a patch's color from the palette is not wired.
- **Bitmap editor:** the "Double-click opens bitmap editor" option is stored but no external editor is launched.
- **Computer-use driving:** the desktop window was not driven interactively. Verification used headless egui_kittest renders instead.
