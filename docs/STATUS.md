# Vector — Status

Last updated: 2026-11-09.

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

**Batch 1 (program frame), done:**

- The whole interface is now drawn from the reference interface definition (`cargo run -p xtask -- ui` writes
  279 bars and 4006 items to `crates/vector-app/src/ui/data.rs`): captions with access keys, shortcuts, tooltips,
  separators and order exactly as CorelDRAW 2026 has them.
- All 12 menus and their submenus, each item enabled for the same 17 selection states as the reference. All 462
  reference menu screenshots have the same number of rows in Vector. Without a document (or on the Welcome Screen
  tab) the menu bar shrinks to File, Tools, Window and Help.
- Right-click menus in the 24 places the reference has them (objects of each kind, text being edited, curve nodes,
  table cells, page, desktop, rulers, page tab, toolbars, status bar, toolbox), including Customize.
- The Standard toolbar, the status bar and the 8 optional toolbars of Window ▸ Toolbars.
- The toolbox with its 15 flyouts and Quick Customize.
- The property bar of each of the 60 tools and of the Pick tool for 18 kinds of selection, field by field in the
  reference order and look (one box for X/Y and for width/height, "%" and "°" units, lead icons only where the
  reference has them, the open padlock while a lock is off). Fields Vector does not have yet are drawn in place,
  disabled.
- Icons: all 985 reference icons used by menus, toolbars, flyouts, property bars and dockers have a Vector drawing
  with the same meaning (`crates/vector-app/src/icons.rs`, mapped in `assets/ui/icon-map.tsv`). Corel's artwork is
  not copied.
- The Welcome Screen tab: Get Started (New Document, New from Template, Open Document, recent documents),
  Workspace (the six workspaces) and What's New, with Ideas and feedback and User Guide below. Help ▸ What's New
  opens it on What's New, as in the reference. Corel's training, store, trial and support entries are left out.
- Fixes found on the way: Window ▸ Workspace ▸ Lite applied Default and Page Layout / Adobe Illustrator did nothing
  (the list now has all six in menu order); the size lock is off by default, as in the reference; Smart Drawing's
  Shape Recognition Level and Smart Smoothing Level lists now change how strokes are recognized and smoothed.

**Batch 2 (File, Edit and View menus), done:**

- Every File, Edit and View item now runs. Only View ▸ Pattern Preview and Snap To ▸ Perspective Lines / Symmetry
  Lines are left: they appear with the pattern, perspective and symmetry modes (batches 3 and 5). File ▸ Document
  Management is left out: it needs an enterprise document server, like Corel Cloud.
- File:
  - New from Template opens Create a New Document on its Templates tab. Save as Template writes `.cdrt`.
  - Print has the General, Color, Composite, Layout, Prepress and Issues tabs, print styles, tiling and marks.
    Print Preview and the Print Merge wizard work too: create or load fields, edit them, print, or make a merged
    document.
  - Publish to PDF has the PDF Settings tabs (General, Color, Document, Objects, Prepress, Security, Issues) and the
    nine presets plus your own. It supports PDF/X-1a, PDF/X-3 and PDF/A-1b, LZW, ZIP and JPEG compression, ASCII85,
    bookmarks, thumbnails, the start view, printer's marks and bleed, and 128-bit passwords with print, edit and copy
    permissions.
  - Export opens the format's own settings (PDF, SVG, PNG, JPEG and other bitmaps). Export For Office, Web (with a
    preview and size estimate) and WordPress all work. WordPress uploads to the site's media library; the password
    is asked for each time and never saved.
  - The Export docker takes the selection, the current page or all pages.
  - Send To ▸ Mail attaches the drawing to a new mail message. Send To Links lists the Windows Send To folder.
  - Acquire Image scans from a WIA scanner or camera and imports the image.
  - Document Properties shows all of the reference's sections.
- Edit:
  - Paste Special lets you choose the clipboard format.
  - Paste in View pastes at the center of the window.
  - Copy Properties From has the reference dialog: Outline Pen, Outline Color, Fill and Text Properties, then you
    click the object to copy from.
  - Bring into Focus and Exit Focus Mode dim everything else, and only the objects in focus can be picked.
- View:
  - Preview Selected Only shows the selection in the full-screen preview.
  - Multipage View puts the pages side by side; clicking a page makes it current.
  - Proof Colors shows colors as a U.S. Web Coated (SWOP) press would print them, or in grayscale.
  - Simulate Overprints shows overprinting objects multiplied with what is below them.
  - Rasterize Complex Effects is stored as a setting.
  - Page ▸ Printable Area outlines what the default printer can print.
  - Grid ▸ Pixel Grid and Snap To ▸ Pixels work in Pixels view. Grid ▸ Baseline Grid draws lines 14 pt apart, and
    Snap To ▸ Baseline Grid snaps to them.
  - Alignment Guides (Alt+Shift+A) line objects up with the edges and centers of other objects and the page.
    Dynamic Guides (Alt+Shift+D) add 0°, 45°, 90° and 135° guides through nearby snap points.
  - Document Navigator and Scrollbars can be hidden. The scrollbars have the Navigator at their corner: press it
    to see the whole drawing and drag to move the view.
  - Rulers, Snap Off, Snap To ▸ Document Grid and Snap To ▸ Objects got their reference shortcuts (Alt+Shift+R,
    Alt+Q, Alt+Y, Alt+Z).

**Batch 3 (Layout, Object and Table menus), done:**

- Every Layout and Table item runs. In Object only Insert ▸ Object (OLE embedding, a Windows COM feature) is left
  out, and Symmetry and Perspective ▸ Draw in Perspective / Edit Perspective Group come with the drawing modes in
  batch 5.
- Layout:
  - Insert Page, Duplicate Page, Rename Page, Delete Page and Go to Page have the reference dialogs. Insert Page
    takes a count, before or after, the page size and orientation; Duplicate Page puts the copy before or after the
    page or in a new document, with the layers only or with their contents; Delete Page takes one page or a range.
  - Insert Page Number puts a page number on the active layer, on all pages, or on the odd or even pages; the
    number follows the page when pages move. Page Number Settings sets the first number, the page it starts on and
    the style (1, I, i, A, a). On All Pages puts one on each existing page; the master layer that also covers pages
    added later comes with the Layers docker (batch 6).
  - Border and Grommet makes a banner document from the page or the selection: a border (stretched or
    mirrored edges, the page itself, or a color) and grommets placed by count or spacing, with a preview.
- Table:
  - Create New Table, Convert Text to Table and Convert Table to Text have the reference dialogs (commas, tabs,
    paragraphs or a character of your own).
  - With the Table tool you click a cell, Shift+click or drag to select a block; Select ▸ Cell, Row, Column and
    Table do the same from the menu.
  - Insert Rows / Columns (one, or a number with the dialog), Delete Row / Column / Table, Distribute Rows /
    Columns Evenly (all, or the selected ones), Merge Cells, Unmerge Cells and Split into Rows / Columns all work.
    Merged cells keep their text and draw and export as one cell. Typing in a cell comes with the Text menu
    (batch 4).
- Object:
  - Create ▸ Arrowhead turns the selected object into an arrowhead (name, length, width, proportional, preview).
    It is added to the Arrowhead selectors of the property bar and the Properties docker, and travels with the
    documents that use it.
  - Create ▸ Pattern Fill: choose Vector, Bitmap or Two color (with Low, Medium or High resolution), then drag a
    marquee. The pattern is added to the Properties docker's pattern list. Vector patterns are kept as 600 dpi
    tiles; View ▸ Pattern Preview is not there yet.
  - Insert ▸ QR Code makes an editable QR code (change its text in the Properties docker). Insert ▸ Barcode opens
    the Barcode Wizard (Code 128, Code 39, EAN-13 and UPC-A, check digits added for you). Validate Barcode checks
    the selected barcode.
  - AI Generate: with nothing selected the submenu opens the AI Generate docker on Create vector graphic, Create
    image or Remix image; with a selection it shows Create Vector Graphic / Create Image Guided by Selection and
    Remix Selection, and the selection is sent as the reference. Images and remixes are placed as bitmaps next to
    the reference.
  - Symbol ▸ Edit Master edits the symbol in place with a Finish button at the top left, as in the reference.
    Break Link, Update from Link and Export Library (a `.csl` library) work for symbols linked to a library.
  - Rollover: Create Rollover (Normal, Over and Down states), Extract Rollover Objects, Edit Rollover (opens the
    Internet toolbar with Active Rollover State, Duplicate State and Delete State), Finish Editing Rollover and
    Rollover Live Preview (the states follow the pointer).
  - Clone Effect copies the effect; the clone does not stay linked to its master yet.
- Fixes found on the way: the object being edited in focus mode (PowerClip contents, a rollover) is no longer
  dimmed with everything else; the interface generator read `*Not(...)` around the wrong binding and hid the three
  AI Generate items the reference shows with nothing selected. All 462 reference menus again have the same number
  of rows in Vector.

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
- **Placeholder fields:** some property bar fields are drawn disabled because Vector does not have the feature yet
  (for example LiveSketch's timer and proximity, the table bar's fill and outline lists, the dimension tool's
  Dynamic dimensioning, Show units and Show leading zero buttons). They are filled in with their tools in batch 5.
- **Items with no command yet:** menu and toolbar items the reference has but Vector cannot do yet are shown and
  say so when chosen; `docs/COVERAGE.md` lists them.
