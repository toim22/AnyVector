# AnyVector — Status

Last updated: 2026-11-12.

## Version history

| Version | Date | What it brings |
| --- | --- | --- |
| [0.4.1](RELEASE-NOTES-0.4.1.md) | 2026-11-12 | A file can be attached to Help ▸ Report a Bug and Submit Ideas and Feedback, with the types and sizes GitHub accepts; it goes to the issue. |
| [0.4.0](RELEASE-NOTES-0.4.0.md) | 2026-11-12 | Keyboard shortcuts as in CorelDRAW 2026, cusp nodes with C, double-click to edit text on the page, Ctrl+C / Ctrl+V and pictures pasted from the web, Break Apart on the property bar, deleting a guideline, a Light theme and 13 more languages. |
| [0.3.1](RELEASE-NOTES-0.3.1.md) | 2026-11-11 | Help ▸ Updates shows the release notes formatted (headings, lists, bold) instead of their raw text. |
| [0.3.0](RELEASE-NOTES-0.3.0.md) | 2026-11-11 | CorelDRAW import, second round: import messages, look-alike fonts, CorelDRAW 12 textures, full-color pattern and PostScript fills drawn, Corel clipart (CMX), editable pattern tiles and texture settings; bug reports and ideas without a GitHub account; Install Now while an update downloads. |
| [0.2.0](RELEASE-NOTES-0.2.0.md) | 2026-11-10 | Opens CorelDRAW drawings natively, from CorelDRAW 1 to 2026: pages, layers, shapes, fills (uniform, fountain, bitmap, two-color, texture, vector pattern, PostScript), outlines and arrowheads, text with its layout, bitmaps, lenses, perspective, envelopes, PowerLines, blends, extrusions, drop shadows, contours, guidelines and CMYK through the drawing's profile. |
| [0.1.0](RELEASE-NOTES-0.1.0.md) | 2026-11-09 | The CorelDRAW 2026 interface item by item: menus, toolbars, tools, property bars, dockers and dialogs; the AnyVector name; Brazilian Portuguese; the installer. |

## Version 0.2.0 (released)

AnyVector 0.2.0 is the CorelDRAW import release (pull requests #16 to #31). Every version of the .cdr format
opens as an ordinary AnyVector document; the full list and the known limits are in
`docs/RELEASE-NOTES-0.2.0.md`. Open items for a later version: texture fills drawn exactly as CorelDRAW does,
vector pattern fills of CorelDRAW 5 to X2, CorelDRAW 1 arrowheads, CorelDRAW 3 font widths, and the few objects
still left out of CorelDRAW 5 to 10 files.

## Version 0.3.0 (released 2026-11-11): CorelDRAW import, second round

Merged to `main` on 2026-11-10 and 2026-11-11 (pull requests #35 to #48); the release notes are in
`docs/RELEASE-NOTES-0.3.0.md` and the manual test plan, all passed, in `docs/TEST-PLAN-corel-import-round2.md`. The plan and research behind it are in
`C:\Projetos\Vector` (not part of the repository).

- **Import notes (#35).** File > Open and File > Import say what a CorelDRAW drawing came in without: objects left
  out by type, texture styles that aren't in CorelDRAW's library, fonts the drawing doesn't list, fonts shown in a
  substitute. Before this, the count of left-out objects was kept but never shown.
- **CorelDRAW as a reference again (#36).** `xtask/corel_render.py` renders drawings with the installed CorelDRAW
  2026. Its automation had refused to open files because CorelDRAW sat on its Welcome screen with no document.
  CorelDRAW 2026 doesn't open CorelDRAW 3 and older files, so those are compared with their stored previews.
- **Textures (#37).** CorelDRAW 12 texture fills come in as textures instead of gray (their names are stored in
  UTF-16), with the tile size from the file. Cosmic Clouds and Cosmic Minerals come in as gray textures instead of a
  flat tone; styles that aren't in CorelDRAW's library keep their name and are drawn as a neutral texture.
- **Text (#38).** Empty paragraph frames are kept (36 frames came back in the CorelDRAW 5 templates). CorelDRAW 3 text
  no longer takes the drawing's first font when its font number isn't in the font list: a drawing that uses one font
  family gets that family, others get the default font and a note.
- **Font substitution (#39).** A missing font gets a look-alike: CorelDRAW's own font names (Bedrock, Switzerland,
  Dutch, Avant Garde...) map to the installed fonts they copy, narrow names get a narrow font, and the import notes
  say which font was used. "Arial Narrow" now really uses Arial Narrow.
- **CMX (#40).** Corel clipart saved as CMX (often named `.cdr`, form `CDRX`) and `.cmx` files open: shapes, groups,
  layers, solid and gradient fills, outlines, simple text and bitmaps.
- **Fills keep what they are made of (#41).** Vector pattern tiles keep their objects and can be edited (the change
  reaches every object using the tile); old texture fills keep their style and settings, editable in the Properties
  docker; PostScript fills keep their number and settings. Object > Create > Pattern Fill keeps its objects too.
  "Convert to bitmap pattern" flattens a fill on purpose.
- **PostScript fills (#42).** 37 of the 38 fills that came in as a plain tone are drawn by AnyVector's own code, sized to
  each object as CorelDRAW does. The random ones are always the same for a file but differ from CorelDRAW's scatter
  and colors. One Motifs setting, the Corel company logo, stays a tone.
- **Vector patterns (#43).** Full-color pattern fills come in with their drawing in CorelDRAW X5 to 2026 files (they
  were gray) and in CMX clipart. CorelDRAW 6 to X4 files use the same code but are not verified: no such file exists
  here, and CorelDRAW 2026 can't save older than X5.

Checked on all samples at hand: 216 of 218 drawings open (CorelDRAW 1/2, 3, 5, 12, CMX clipart and the test
fixtures); the two that don't are damaged CorelDRAW 3 files that 0.2.0 refused as well.

Also in this version: Help > Report a Bug and Help > Submit Ideas and Feedback send the issue through a small relay
(`services/issue-relay`, a Cloudflare Worker acting as the AnyVector Reports GitHub App), so nobody needs a GitHub
account. When the relay can't be reached, GitHub's filled-in form opens in the browser as before (#46, issue #34).
Help > Updates shows Install Now while an update downloads in the background, and its buttons stay in view (#48,
issue #33).

## CorelDRAW import: known issues to review later

These are outside the current round of work and stay as they are until they are picked up again. The research
behind each one is in `C:\Projetos\Vector\issues\` (not part of the repository).

- **Texture fills drawn exactly as CorelDRAW does (research issue 1).** Files older than X3 keep only a texture's
  settings, and AnyVector draws soft clouds in the texture's colors. An exact drawing needs CorelDRAW's texture
  generators, and the only source for them is CorelDRAW's own program: a licensing decision comes first.
- **CorelDRAW 1 arrowheads (issue 5).** No CorelDRAW 1 file with an arrow has been found to settle the record.
- **Linked bitmaps (issue 7a)** and **object types 0x20 and 0x0a (issue 7c)** in CorelDRAW 5 to 10 files: each needs a
  sample file that has one. AnyVector now names these when a drawing opens.
- **Sample 33.cdr's black ovals (issue 8).** Needs the file; the research has the steps to diagnose it.
- **CMYK of files older than X4, or made with a profile CorelDRAW 2026 doesn't ship (issue 12).** Older files carry no
  profile, so the colors depend on a choice; a profile embedded in the file (`iccd`) is not read yet.
- **Left over from the second round:**
  - Texture fills of CorelDRAW 7 to 11 and vector patterns of CorelDRAW 5 to X4 are not verified on a real file; vector
    patterns of CorelDRAW 5 files stay gray.
  - What CorelDRAW 3's text font numbers point to is unknown; drawings with several fonts get the default font.
  - The "ON THE RIGHT TRACK!" drawing (Bedrock) was not measured; text is not stretched to the missing font's widths.
  - PostScript fill settings are read-only in the Properties docker; CMX texture, PostScript and bitmap fills, arrowheads
    and lenses come in gray or are left out; copying a pattern-filled object to another document drops its pattern.

## Version 0.1.0 (released)

AnyVector 0.1.0 is being built to match the CorelDRAW 2026 reference, in batches (see `docs/PLAN-0.1.0.md`).
`docs/COVERAGE.md` lists every menu item, dialog, docker, tool, toolbar and right-click menu of the reference with
its status in AnyVector.

**Batch 0 (foundation and branding), done:**

- The version is 0.1.0 everywhere (program, file version label "0.1 (2026.0)", installer).
- New icon and logo from the brand SVGs: the exe and window icon, shortcuts and file types use the icon; the splash
  screen, the About box and the installer's side picture use the logo.
- Help ▸ About AnyVector follows the reference: logo banner, version and release, copyright, Source ID (the commit the
  program was built from), System Info... and Legal Notices....
  - System Info has the reference's five categories (System, Display, Printing, AnyVector EXEs & DLLs, System DLLs),
    read from Windows, and Save writes them to a text file.
  - Legal Notices lists the 278 open-source components in anyvector.exe and their licenses.
- The installer (`dist/AnyVector-0.1.0-setup.exe`) has the Windows 11 wizard style with light and dark pictures,
  installs per user or for all users, adds AnyVector to "Open with" for `.cdr` (default program only if chosen), opens
  `.vct` and optionally SVG, and keeps the folder and choices on updates.
- Build helpers (`xtask`): brand assets, the coverage report, the comparison page and the Legal Notices list.

**Batch 1 (program frame), done:**

- The whole interface is now drawn from the reference interface definition (`cargo run -p xtask -- ui` writes
  279 bars and 4006 items to `crates/anyvector/src/ui/data.rs`): captions with access keys, shortcuts, tooltips,
  separators and order exactly as CorelDRAW 2026 has them.
- All 12 menus and their submenus, each item enabled for the same 17 selection states as the reference. All 462
  reference menu screenshots have the same number of rows in AnyVector. Without a document (or on the Welcome Screen
  tab) the menu bar shrinks to File, Tools, Window and Help.
- Right-click menus in the 24 places the reference has them (objects of each kind, text being edited, curve nodes,
  table cells, page, desktop, rulers, page tab, toolbars, status bar, toolbox), including Customize.
- The Standard toolbar, the status bar and the 8 optional toolbars of Window ▸ Toolbars.
- The toolbox with its 15 flyouts and Quick Customize.
- The property bar of each of the 60 tools and of the Pick tool for 18 kinds of selection, field by field in the
  reference order and look (one box for X/Y and for width/height, "%" and "°" units, lead icons only where the
  reference has them, the open padlock while a lock is off). Fields AnyVector does not have yet are drawn in place,
  disabled.
- Icons: all 985 reference icons used by menus, toolbars, flyouts, property bars and dockers have an AnyVector drawing
  with the same meaning (`crates/anyvector/src/icons.rs`, mapped in `assets/ui/icon-map.tsv`). Corel's artwork is
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
  of rows in AnyVector.

**Batch 4 (Text menu), done:**

- Every Text item runs except Line-Breaking Rules (East Asian rules; the reference hides it outside East Asian
  languages). Text and Glyphs open their dockers, which are checked in batch 6.
- Paragraph layout was rewritten:
  - Tabs: left, right, center and decimal stops with dot leaders (Tab Settings and Leader Settings dialogs).
  - Columns: equal or custom widths and gutters (Column Settings).
  - Bullets and Numbering: any glyph of a font, 16 numbering styles and nine levels.
  - Drop Cap: plain or with a hanging indent.
  - Automatic hyphenation (US English patterns, with hot zone and capitals rules), Optional Hyphens and Custom
    Optional Hyphens.
  - Align to Baseline Grid, and frames in the shape of an object (Create Empty Text Frame).
  - Text that does not fit no longer draws below the frame. The frame turns red, as in the reference.
- Linked frames: Link makes the text flow from one frame into the next. Unlink leaves each frame with the text it
  shows. Typing in any frame of a chain edits the shared text. Column/Frame Break sends the rest to the next column
  or frame.
- Writing Tools use the spell checker built into Windows:
  - Spell Check: Replace, Auto Replace, Skip Once, Skip All, Add, Undo, Options and the language.
  - Grammatik: spelling, repeated words, capitals, a/an, spacing and punctuation.
  - Thesaurus: synonyms from WordNet. Antonyms and the other relations are not available, so Thesaurus stays
    partial.
  - Language: the reference's 125 languages; tags the selected characters.
  - Words the dictionary does not know get a red zigzag on the page (Options ▸ Text ▸ Spelling).
- QuickCorrect works as you type: replacement list, capitals, day names, two initial capitals, and typographic
  quotes in the styles chosen in Options.
- Other Text items:
  - Edit Text: font, size, B/I/U, alignment, non-printing characters, zoom, Import and Change Case.
  - Change Case, Text Statistics, Encode (every Windows code page, with preview), Fit Text to Frame (paragraph
    text and tables), Insert Placeholder Text (frames and table cells) and Align to Baseline.
  - Show Non-Printing Characters, Show Text Frames and Make Text Web-compatible. Make Text Web-compatible is kept
    with the text; AnyVector has no HTML export yet.
- Tables: with the Table tool, typing goes into the chosen cell. Tab and Shift+Tab move between cells.

**Batch 5 (Tools), done:**

- Every tool works with every option of its property bar. The four Bitmap buttons (Bitmap Mask, Crop Bitmap, Edit
  Bitmap, Remove Background) came with the Bitmaps menu in batch 7.
- Pick and Free Transform:
  - Free Transform rotates, reflects, scales and skews about the point you click, and can apply to a duplicate.
  - Crop with a rotation angle crops along the rotated frame.
- Shape and node editing:
  - Shape tool stretch and rotate modes work on the selected nodes, and so does reflecting nodes horizontally or
    vertically.
  - Pen: auto add/delete nodes and the rubber-band preview.
  - Polyline closes itself when it ends on its first node.
- Curves:
  - LiveSketch: timer, proximity, single curve, smoothing and Include curve.
  - Painterly Brush: brush, size, opacity, smoothing and color variation.
  - Artistic Media: scale with object.
- Cutting and erasing:
  - Knife: 2-point, freehand and Bézier cuts, with gap or overlap width, auto-close and outline handling.
  - Eraser: round or square nib.
- Shapes:
  - Impact tool: radial and parallel lines, line widths and spacings (fixed, series or random), inner and outer
    boundaries. It stays editable: change it on the property bar and it redraws.
  - 2-Point Line: perpendicular and tangent lines.
- Dimensions: units, leading zero, dynamic or frozen text, extension line gap and length, arrows, successive
  dimensions, and callout shapes and gaps.
- Effects and fills:
  - Envelope: straight, single-arc, double-arc and unconstrained modes, mapping modes, keep lines, and envelope from
    a shape.
  - Interactive Fill and Transparency: gradient handles can be dragged after the gradient is made.
  - Mesh Fill: patches take palette colors and sampled colors.
  - Smart Fill: fill and outline from the default or a chosen color.
  - Copy Effect: fill, transparency, variable outline and mesh as well.
- Eyedroppers:
  - Color Eyedropper samples 1×1, 2×2 or 5×5 pixels, and can sample anywhere on the desktop.
  - Attributes Eyedropper copies the properties, transformations and effects chosen on its property bar.
- Variable Outline: add width nodes and drag them, one side or both locked; the width runs smoothly between nodes.
- Anchor Editing: add, move and delete anchors, with automatic placement; connectors follow their anchors.
- Brushes: Attract/Repel and Twirl reverse with Ctrl, Smear has a pointed falloff, Roughen keeps a fixed
  direction.
- Tables: rows grow to fit their text, and separated cells have spacing and margins.
- Object ▸ Symmetry:
  - Create New Symmetry, Edit, Finish, Remove and Break Symmetry Link.
  - The floating toolbar has mirror lines (1 to 12), angle of rotation, the center X/Y, Show symmetry lines,
    Finish and Remove.
  - The mirror copies follow the objects while you draw. Snap to symmetry lines is on while editing.
  - Double-click a symmetry to edit it.
- Object ▸ Perspective:
  - Draw in Perspective and Edit Perspective Group.
  - One-point, two-point and three-point (worm's-eye and bird's-eye) fields.
  - Orthographic, Top, Left, Right and Side planes, and the horizon height.
  - Right-click ▸ Move to Plane.
  - Objects drawn flat are projected onto their plane on Finish. Edit brings back the flat objects.
  - Snap to perspective lines is on while drawing in perspective.
- Object ▸ Insert ▸ Object follows the reference dialog:
  - Create New lists the document types Windows can create.
  - Create from File has Link and Display As Icon.
  - Double-click opens the object in its program. When the file is saved there, the object updates.
  - Embedded objects are stored in the drawing; linked ones point to their file.

**Batch 6 (Dockers), done:**

- Every docker of Window ▸ Dockers was compared with the reference captures and their inner states, and rebuilt
  with the same layout, order, icons and wording. A visual test opens each docker and saves a render next to its
  reference (`target/visual/dk_NNN.png`, listed in `docs/reference-map.tsv`). Learn is left out (Corel content).
- The docker host has the vertical tabs on the right edge, as in the reference.
- Arrange dockers: Transform, Coordinates, Live Guides, Guidelines, Align and Distribute, Fit Objects to Path, Step
  and Repeat, Shape, Corners and Join Curves.
- Effects dockers: Artistic Media, Bevel, Bitmap Mask, Blend, Contour, Envelope, Extrude (its five pages), Lens (every
  lens kind), PhotoCocktail and Pointillizer.
- Properties: Outline, Fill, Transparency, Bitmap Effects, object, Painterly Brush and Summary sections, tabbed or in
  one folding column. Fountain fill nodes, blend direction, acceleration and smooth transitions; vector, bitmap and
  two-color pattern fills and transparencies; transparency targets; the calligraphic nib; dash alignment; the Edit
  Line Style dialog.
- Objects, Pages, Comments, Object Data, Object Styles, Symbols, Find and Replace, Export and AI Generate.
- Text (Character, Paragraph and Frame), Font Sampler and Glyphs.
- Color, Palettes, Color Styles, Color Proofing and Assets:
  - Palettes is a palette manager: Document Palette, Color Styles Palette, My Palettes and the Palette Libraries,
    each with a check box that opens the palette as a column at the window edge.
  - Color Styles has color styles and harmonies (Analogous, Complementary, Monochromatic, Tetrad, Triad...), the
    Harmony Editor, gradients, merge, swap, select unused, and style sheets that import and export.
  - Color Proofing: Proof colors, the gamut warning in its color and transparency, the simulated environment from
    the installed color profiles, presets, Export soft proof and Print proof.
  - Assets: local folders and the document's linked files, with search, sorting, categories, favorites, a folder
    tree, list or thumbnails, and Import with link options.
- Tray, Links and Rollovers, Sources, Views, History and Scripts. Links and Rollovers sets links and bookmarks,
  draws the hotspots on the canvas, and creates and edits rollovers.

**Batch 7 (Effects and Bitmaps menus), done:**

- Every bitmap effect of the Effects menu (120 of them) opens its page in the Properties docker (Bitmap Effects),
  with the reference's controls, order, ranges, lists and defaults. The pages are generated by
  `python xtask/fxgen.py` into `crates/anyvector/src/fxdata.rs` from the CorelDRAW interface definition and the
  reference screenshots. A visual test renders each page next to its reference (`target/visual/fx_NNN_*.png`).
- The effects stay editable in the object's effect list and are drawn live on the canvas: 3D Effects, Adjust, Art
  Strokes, Blur, Camera, Color Transform, Contour, Correction, Creative, Custom, Distort, Noise, Sharpen, Texture and
  Transform. Flatten Effects turns them into a bitmap. Presets: Default restores the effect's settings, (+) saves
  the current settings as a preset and (−) deletes it.
- Effects ▸ Adjust ▸ Auto Adjust corrects the color and tone of the selected bitmaps.
- Bitmaps menu, with the reference's dialogs:
  - Convert to Bitmap: resolution, color mode, dithering, Always overprint black, anti-aliasing, transparent
    background and the file size.
  - Straighten Image (lens distortion, rotation, vertical and horizontal perspective, grid, crop) and the Image
    Adjustment Lab (temperature, tint, saturation, brightness, contrast, highlights, shadows, midtones, Auto adjust,
    white and black points, rotation, snapshots, undo and redo, histogram).
  - Remove JPEG Artifacts, Remove Background and Crop Bitmap. Edit Bitmap opens the bitmap in the default image
    editor, and saving it there updates it in AnyVector (also on double-click when Options ask for it).
  - Resample: size in eight units, resolution, bicubic or nearest neighbor, file and pixel sizes.
  - Mode: Convert to 1 bit (Line Art, Ordered, Jarvis, Stucki, Floyd-Steinberg, Halftone screens,
    Cardinality-Distribution), Grayscale, Duotone (monotone to quadtone, ink curves, overprints, load and save),
    Convert to Paletted (palettes, dithering, colors, range sensitivity, presets), RGB, Lab and CMYK.
  - Inflate Bitmap (Auto, and Manually with sizes in pixels or percent), Break Link and Update from Link.
  - PowerTRACE: Quick Trace, Centerline Trace (Technical Illustration, Line drawing) and Outline Trace (six
    profiles), with the reference's preview modes, Image handling, Shape detail, Shape structure and Color sections
    (color modes and palettes, remove a color, merge colors, edit, eyedropper), undo and redo. Repeat Last Trace and
    Open Last Trace Settings use the last trace's settings.
- The Bitmap property bar buttons (Bitmap Mask, Crop Bitmap, Edit Bitmap, Remove Background) work.

**Batch 8 (Window, Help and release), done:**

- Every Window and Help item runs; `docs/COVERAGE.md` has no item left to do.
- Window arrangements:
  - New Window opens a second window on the same drawing, named `name.cdr:1`, `name.cdr:2`; a change in one
    window shows in the others, and only the last window of a document asks to save.
  - Cascade, Tile Horizontally and Tile Vertically arrange the document windows, each showing its whole page;
    Combine Windows (and Dock Window) bring back the tabs. Refresh Window redraws every window.
- Workspaces, as in the reference:
  - Each workspace keeps its own toolbars, dockers, docker width, menu bar and shortcut keys; switching saves the
    layout of the workspace you leave.
  - Window ▸ Workspace ▸ New (and Duplicate in Options) has the New Workspace dialog: name, Base on, description,
    Set as Current. Your workspaces follow the built-in ones in Window ▸ Workspace.
  - Delete asks with the reference warning, then Default becomes current.
  - Export Workspace picks dockers, toolbars, menus and shortcut keys in a tree and writes a `.cdws` file; Import
    Workspace reads it into a new or the current workspace.
  - Tools ▸ Options ▸ Workspaces lists them with Set as Current, Duplicate, Delete, Export and Import, and the
    description. Cancel keeps workspace changes, as in the reference.
- Color palettes:
  - The Palette Editor has the reference layout: palette list (Document, Color Styles, My Palettes), New, Open,
    Save and Save As, the 16-point swatches, Edit Color, Add Color, Delete Color, Sort Colors (Reverse, Hue,
    Brightness, Saturation, RGB value, HSB value, Name), Reset Palette and Selected color (name, Process or Spot,
    components). Changes to several palettes are kept until OK.
  - Edit Color and Add Color open Select Color: Color viewers or Color palettes, the palette list with its headings,
    Show color names, old and new swatches, the eyedropper, and RGB, hex, CMYK, Lab and HSB values.
  - Palettes are custom palette files (`*.xml`), the format CorelDRAW reads and writes, with spot colors as named
    color spaces; the edited default palette is kept in My Palettes. Open Palette, Create Palette from Selection or
    Document and the Palettes docker use the same files. Close All Palettes closes the opened, default, document and
    color styles palettes.
- View ▸ Pattern Preview: Properties ▸ Vector Pattern ▸ Edit fill opens the pattern as its own document (one tile on
  the page). There View ▸ Pattern Preview appears, as in the reference, and shows the tile repeated across the
  window. Closing the document makes the pattern again from the page, as one undo step of the drawing. Patterns now
  keep the objects they were made from.
- Help:
  - Product Help, Quick Start Guide, User Guide and Keyboard Shortcuts are HTML pages that open in the browser and
    need no internet connection. The User Guide and the shortcut list are built from the program's own menus,
    tools and dockers, so they always match it.
  - Updates opens the Updates dialog (see below); its Release Notes link opens the release page.
  - Submit Ideas and Feedback opens a new idea in the AnyVector issue tracker; Report a Bug is unchanged.
  - Message Settings is left out with Corel's other online services (it sets Corel's in-product messages).
- The 0.1.0 installer is rebuilt (`dist/AnyVector-0.1.0-setup.exe`) and the release notes are in
  `docs/RELEASE-NOTES-0.1.0.md`. Nothing is tagged or published.

**After batch 8 (AnyVector name, languages, updates), done:**

- The program is renamed AnyVector everywhere: window title, About box, menus (Help ▸ About AnyVector, Options ▸
  AnyVector), file types, the installer (`AnyVector-0.1.0-setup.exe`, `anyvector.exe`, Start menu) and the
  settings folder (`%APPDATA%\AnyVector`, moved from `%APPDATA%\Vector` on the first start). The banner, icon and
  logo are unchanged; the installer's side picture now says AnyVector.
- Brazilian Portuguese interface (`assets/i18n/pt-BR.tsv`, 5,524 of 5,777 strings; the rest are names and codes),
  aligned with the `corel-i18n` pt-BR guide. The language is chosen in Options ▸ Global ▸ General and changes at
  once. The installer has a language page (English or Português Brasileiro), and "Ask me the next time the
  application starts" shows a language chooser on the next start.
- Help ▸ Updates and Options ▸ Global ▸ General ▸ Allow product updates: keep up to date (background download,
  install when AnyVector closes, then start again), check and let me choose, or never. Updates come from the
  GitHub releases of the project; while the repository is private they need `gh` signed in.
- Submit Ideas and Feedback opens a form like Report a Bug and sends the idea as a GitHub issue labeled
  `enhancement`.
- The Welcome Screen tab has a close button (×, on the right like the document tabs; middle-click also closes it).
- AI Generate without an API key says where to add it (Tools ▸ Options ▸ Global ▸ Privacy ▸ AI Generate) and has an
  Open AI Settings button; Save Key writes the key to the user's Windows environment variable.
- The menu bar captions are centered vertically in a 26-pixel bar.

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
- Options dialog: every category and page from the screenshots, with "CorelDRAW" renamed to "AnyVector" and VBA renamed to Scripts.
- Menu bar is 26 px tall so the standard toolbar no longer crowds it; disabled toolbar icons use a lighter gray.
- The Purchase menu is removed. Help ▸ Report a Bug opens a form that creates a GitHub issue on toim22/AnyVector through the `gh` CLI when it is signed in, or opens a prefilled issue page in the browser otherwise. Help ▸ Report a Bug ▸ View Reported Bugs opens the issue list.
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

- **.cdr is AnyVector's own container, not CorelDRAW's format.** It is a zip that holds:
  - the drawing
  - XMP metadata (title, subject, rating)
  - embedded fonts and ICC profiles
  - a thumbnail

  AnyVector opens and saves it, and the older `.vct` format still opens and saves. Files made by CorelDRAW (RIFF, or a zip with `content/riffData.cdr`) are detected and refused with a message. AnyVector cannot read them yet.
- **Color settings are stored, not applied.** The document's color mode, profiles and rendering intent are saved, and profiles can be embedded, but the canvas does not do color management.
- **Multipage view** is stored per document but the canvas still shows one page at a time.
- **Top print presets inferred.** The first rows of print presets in New Document (US sizes, Arch, ISO, envelopes, and so on) are not visible in the reference photos, so their order was inferred. The rest follows the photos exactly.

- **Computer-use driving:** the desktop window was not driven interactively. Verification used headless egui_kittest renders instead.
- **Effects and Bitmaps (batch 7):**
  - Bitmap color modes: Lab and CMYK record the mode, but the pixels stay RGB. Always overprint black (Convert to
    Bitmap) marks the bitmap to overprint.
  - Duotone: no PANTONE library; the inks are approximate colors under their reference names.
  - Remove Background and PowerTRACE's Remove image background find the background color at the borders; they are
    not AI. PowerTRACE's Upsample modes are a plain 2× resample, and Resample's Illustration and Photorealistic modes
    are disabled, as in the reference without AI.
  - Effect pages: Levels, Tone Curve and Color Balance have no eyedroppers; the Sample & Target default targets are
    approximated; the Lens Flare center and the Lighting Effects light cone use a simplified preview; Art Style uses
    local filters; frames, canvas textures, bump and displacement maps are drawn by AnyVector (no CorelDRAW content
    files). Presets saved with (+) are named Preset 1, 2...
- **Perspective field handles:** the field covers the page (Fill page), and the horizon is set on the floating
  toolbar. Vanishing points cannot be dragged on the canvas, and there are no camera lines.
- **Symmetry preview:** the mirror copies show as outlines while you edit. Show full preview and Fuse open curves
  are not available.
- **Dockers (batch 6):**
  - Properties: no skew of fills, no texture or PostScript fills and transparencies, no bitmap tone adjustments, no
    pattern X/Y and row/column offsets; the (+) Save as new button is disabled.
    The eye button's tooltip ("Show or hide the summary.") is not in the reference strings.
  - The ruler, the rotation center and the Step and Repeat vertical offset count downward (y-down); CorelDRAW counts
    upward.
  - Objects: no drag and drop of layers and objects; the Desktop layer is not a real layer (Keep Desktop Objects is
    only stored); no wireframe per layer; master layers are stored with page 1.
  - Pages: the Custom multipage arrangement is horizontal; the three view tooltips are not in the reference strings.
  - Object Styles: the style editor is simpler; Character, Paragraph and Frame styles are disabled; Default Object
    Properties change the graphic defaults only; style sheets are AnyVector's own `.vstyles` file.
  - Find and Replace: Pantone queries find nothing (colors keep no palette names); Color model or palette replace is
    disabled.
  - Export: the export list cannot be reordered by dragging. AI Generate: the terms page is in AnyVector's own words.
  - Text: Character background fill, horizontal offset, angle, overline, capital spacing, annotation forms, OpenType
    features, synthesized sub/superscripts and small-caps options are disabled; Paragraph Before paragraph spacing,
    language spacing, text direction and Justification are disabled; Frame background, vertical alignment and
    right-to-left columns are disabled; underline and strikethrough are Single Thin only.
  - Font Sampler: samples cannot be dragged into the drawing; Copy puts SVG on the clipboard. Glyphs: OpenType and
    Language info are empty; Show all glyph variations is disabled.
  - Transform: Scale portion is measured with two clicks instead of a drag. Live Guides: the intelligent spacing and
    dimensioning marks are not drawn. Guidelines: Show at scale is only stored. Align and Distribute: Use outline is
    not shown.
  - Blend: fixed spacing, Map nodes, Split and Fuse are disabled. Bevel: Emboss is approximate. Envelope: the presets
    are approximated with 8 handles. Extrude: Shared vanishing point is disabled and Copy VP From copies the whole
    extrusion. Lens: Frozen is only stored and the Bitmap Effect lens does nothing.
  - Color: Add to Custom Spot Colors lasts for the session only.
  - Palettes: licensed palette books (PANTONE, TRUMATCH, HKS, DIC, FOCOLTONE, SpectraMaster, TOYO, ONYX, Roland,
    Universal Laser Systems) and the themed CMYK and RGB subfolders are not included; the Lab, Uniform, CMYK and
    Percent Gray palettes are generated approximations; the open state of library palettes is not saved.
  - Color Styles: styles link to objects only by equal colors; New from Selected and New from Document skip their
    dialogs; New Gradient uses fixed options (5 colors, 50%); the harmony wheel is display-only; Add Linked Style
    Sheet is disabled; converting to CMYK keeps the RGB values.
  - Color Proofing: every CMYK environment is simulated with the built-in U.S. Web Coated (SWOP) approximation (no
    color engine) and RGB environments leave the colors unchanged; Preserve numbers and the rendering intent are only
    stored; the gamut test is approximate; Export soft proof writes PNG, JPG or TIFF.
  - Assets: Cloud and Get More are disabled; categories are subfolders; linked assets are the document's linked files
    (no symbol libraries); Properties shows a message box; Edit opens the file in its default program or in AnyVector.
  - Links and Rollovers: the hotspot is always the bounding box, even with Shape; Target and Alt comments are only
    stored.
  - Tray: trays last for the session only, and files cannot be dragged in from Explorer. Sources: only bitmaps can
    be linked. History: no Save as a script. Scripts are Rhai. Comments: no anonymous mode. Object Data: no
    Application Defaults, field formats or Print. Symbols: Network Symbols is empty. Views are not listed in the
    zoom list.
- **Window, workspaces and palettes (batch 8):**
  - Document windows never float, so Dock Window does what Combine Windows does. Inactive tiled windows show no
    ruler ticks.
  - Workspace files: AnyVector reads and writes its own `.cdws` content (`content/workspace.json`); workspace files saved
    by CorelDRAW cannot be imported.
  - Help ▸ Updates shows this version's release notes; there is no online update check.
  - Pattern documents edit vector patterns made in AnyVector 0.1.0 or later as objects; older patterns and bitmap
    patterns open as their tile image.
- **Items with no command yet:** menu and toolbar items the reference has but AnyVector cannot do yet are shown and
  say so when chosen; `docs/COVERAGE.md` lists them.
