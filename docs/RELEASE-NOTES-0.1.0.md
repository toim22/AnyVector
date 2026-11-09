# AnyVector 0.1.0

AnyVector 0.1.0 is the first release that follows the CorelDRAW 2026 interface item by item: the same menus in the
same order, the same toolbars, tools, property bars, dockers and dialogs, with AnyVector's own icons that mean the same
thing. Corel's online services (Corel Cloud, accounts, Learn, Support and the community pages) are left out.

## The workspace

- All 12 menus and their submenus, each item enabled in the same situations as in CorelDRAW, with the same
  captions, access keys, shortcuts and tooltips.
- Right-click menus for objects of every kind, text, curve nodes, table cells, the page, the rulers and the
  toolbars.
- The Standard toolbar, the property bar of every tool and of every kind of selection, the status bar, the toolbox
  with its 15 flyouts, and the 8 optional toolbars of Window ▸ Toolbars.
- The Welcome Screen with Get Started, Workspace and What's New.
- Workspaces: Default, Lite, Touch, Illustration, Page Layout and Adobe Illustrator, plus your own. Window ▸
  Workspace ▸ New, Delete, Import and Export, and Tools ▸ Options ▸ Workspaces. Each workspace keeps its toolbars,
  dockers and shortcut keys.

## Drawing and editing

- Every tool of the toolbox works with every option of its property bar: Pick and Free Transform, the Shape tool
  and its brushes, Freehand, 2-Point Line, Bézier, Pen, B-Spline, Polyline, 3-Point Curve, LiveSketch, Smart
  Drawing, Artistic Media and Painterly Brush, the shape tools, Impact, Text, Table, the dimension and connector
  tools, Blend, Contour, Distort, Drop Shadow, Envelope, Extrude, Transparency, the eyedroppers, Interactive Fill,
  Mesh Fill and Smart Fill.
- Object ▸ Symmetry and Object ▸ Perspective drawing modes.
- Paragraph text with tabs, columns, bullets and numbering, drop caps, hyphenation and linked frames; spell check,
  Grammatik, Thesaurus and QuickCorrect with the Windows spell checker.
- Tables: create, select cells, insert and delete rows and columns, merge and split cells, type in cells.

## Effects and bitmaps

- The 120 bitmap effects of the Effects menu, editable in the Properties docker and drawn live on the canvas.
- The Bitmaps menu: Convert to Bitmap, Straighten Image, Image Adjustment Lab, Resample, the color modes,
  PowerTRACE, Remove Background, Crop Bitmap, and Edit Bitmap in your default image editor.

## Dockers

- Every docker of Window ▸ Dockers with the CorelDRAW layout, from Properties and Objects to Color Styles, Color
  Proofing, Assets, History and Scripts.

## Files, printing and export

- CorelDRAW-compatible .cdr files, templates (.cdrt), SVG and the bitmap formats; Publish to PDF with PDF/X and
  PDF/A, passwords and permissions; Export For Office, Web and WordPress.
- Print with the General, Color, Composite, Layout, Prepress and Issues tabs, Print Preview and Print Merge.
- Acquire Image from WIA scanners and cameras; Send To Mail.

## Windows, palettes and help (new in this batch)

- Window ▸ New Window opens a second view of the same drawing; both windows show every change. Cascade, Tile
  Horizontally, Tile Vertically, Combine Windows and Dock Window arrange the document windows.
- Window ▸ Color Palettes: the Palette Editor with Edit Color, Add Color, Delete Color, Sort Colors and Reset
  Palette, and the Select Color dialog with color viewers, color palettes and the eyedropper. Palettes are saved as
  custom palette files (*.xml), the format CorelDRAW reads and writes. Open Palette, Create Palette from Selection
  and Create Palette from Document use the same files.
- View ▸ Pattern Preview shows how a pattern tile repeats while you edit it.
- Help: Product Help, Quick Start Guide, User Guide and Keyboard Shortcuts open in your browser and work without an
  internet connection. The User Guide is built from the program, so it lists exactly the tools, menu commands and
  dockers of this version. Submit Ideas and Feedback and Report a Bug open a form inside AnyVector and send it to
  the AnyVector issue tracker (ideas are labeled "enhancement", bugs "bug").

## AnyVector name, languages and updates

- The program is now called AnyVector (`anyvector.exe`). The banner, icon and logo are unchanged. Settings move
  from `%APPDATA%\Vector` to `%APPDATA%\AnyVector` on the first start. Installing over Vector moves the program
  from its `Vector` folder to an `AnyVector` folder and replaces the Vector shortcuts in the Start menu and on the
  desktop.
- The interface is available in English (United States) and Brazilian Portuguese. Choose the language in
  Tools ▸ Options ▸ Global ▸ General ▸ User interface language; it changes at once, without restarting. The
  installer asks which language AnyVector starts in.
- AnyVector looks for a new version each time it starts, in the background, without slowing the start. When one is
  found, a message says so: OK closes AnyVector (asking you to save your drawings), installs the new version and
  starts AnyVector again; Later asks again at the next start. Help ▸ Updates checks whenever you ask. Tools ▸
  Options ▸ Global ▸ General ▸ Allow product updates chooses between keeping the product up to date (the update
  downloads in the background, so OK installs at once), checking and letting you choose, or never checking.
- The Welcome Screen tab has a close button like a document tab; Window ▸ Welcome Screen opens it again.
- The AI Generate message about a missing API key says where to add it, and Open AI Settings goes straight to
  Tools ▸ Options ▸ Global ▸ Privacy, where Save Key stores the key in your Windows account's environment variable.
- The menu bar captions are centered vertically.

## Known limits

- Licensed palette books (PANTONE, TRUMATCH and others) are not included.
- Workspaces exported from CorelDRAW (.cdws) cannot be imported; AnyVector's own .cdws files can.
- Some dockers and dialogs have controls that are shown but not active yet; `docs/STATUS.md` and
  `docs/COVERAGE.md` list them item by item.
