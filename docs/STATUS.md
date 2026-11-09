# Vector — Status

Last updated: 2026-11-08.

## Version 0.1.0 (in progress)

Vector 0.1.0 is being built to match the CorelDRAW 2026 reference, in batches (see `docs/PLAN-0.1.0.md`).
`docs/COVERAGE.md` lists every menu item, dialog, docker, tool, toolbar and right-click menu of the reference with
its status in Vector.

**Batch 0 (foundation and branding), done:**

- The version is 0.1.0 everywhere (program, file version label "0.1 (2026.0)", installer).
- New icon and logo from the brand SVGs: the exe and window icon, shortcuts and file types use the icon; the splash
  screen, the About box and the installer's side picture use the logo.
- Help ▸ About Vector follows the reference: logo banner, version and release, copyright, Source ID (the commit the
  program was built from), System Info... and Legal Notices....
  - System Info has the reference's five categories (System, Display, Printing, Vector EXEs & DLLs, System DLLs),
    read from Windows, and Save writes them to a text file.
  - Legal Notices lists the 278 open-source components in vector.exe and their licenses.
- The installer (`dist/Vector-0.1.0-setup.exe`) has the Windows 11 wizard style with light and dark pictures,
  installs per user or for all users, adds Vector to "Open with" for `.cdr` (default program only if chosen), opens
  `.vct` and optionally SVG, and keeps the folder and choices on updates.
- Build helpers (`xtask`): brand assets, the coverage report, the comparison page and the Legal Notices list.

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
- Menu bar is 26 px tall so the standard toolbar no longer crowds it; disabled toolbar icons use a lighter gray.
- The Purchase menu is removed. Help ▸ Report a Bug opens a form that creates a GitHub issue on toim22/Vector through the `gh` CLI when it is signed in, or opens a prefilled issue page in the browser otherwise. Help ▸ Report a Bug ▸ View Reported Bugs opens the issue list.
- Create a New Document follows the reference photos:
  - presets grid (Print, Web, Devices, Social, Custom) with search, sort and size filter
  - + and − for custom presets
  - Templates tab
  - General, Dimensions, Layout and Colors sections
  - "Do not show this dialog again"
- Save Drawing is the native Windows dialog. It defaults to "CDR - CorelDRAW (*.cdr)" and has:
  - Title, Subject and Rating
  - Embed color profiles (with size) and the color profile summary
  - Embed fonts (with size)
  - Version
  - Advanced... (Optimize for speed/size, presentation attributes, bitmap and object compression, thumbnail)
  - These were checked by opening the dialog, clicking Advanced... and confirming OK returns the edited values.
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

- **.cdr is Vector's own container, not CorelDRAW's format.** It is a zip that holds:
  - the drawing
  - XMP metadata (title, subject, rating)
  - embedded fonts and ICC profiles
  - a thumbnail

  Vector opens and saves it, and the older `.vct` format still opens and saves. Files made by CorelDRAW (RIFF, or a zip with `content/riffData.cdr`) are detected and refused with a message. Vector cannot read them yet.
- **Color settings are stored, not applied.** The document's color mode, profiles and rendering intent are saved, and profiles can be embedded, but the canvas does not do color management.
- **Multipage view** is stored per document but the canvas still shows one page at a time.
- **Top print presets inferred.** The first rows of print presets in New Document (US sizes, Arch, ISO, envelopes, and so on) are not visible in the reference photos, so their order was inferred. The rest follows the photos exactly.

- **Gradient handles:** the handles of interactive fills and transparencies show but cannot be dragged after creation. Drag again to redefine them.
- **Mesh fill:** picking a patch works, but setting a patch's color from the palette is not wired.
- **Bitmap editor:** the "Double-click opens bitmap editor" option is stored but no external editor is launched.
- **Computer-use driving:** the desktop window was not driven interactively. Verification used headless egui_kittest renders instead.
