# AnyVector — UI specification (transcribed from `screens/`)

Source of truth: the 73 screenshots in
`C:\Users\anton\Documents\deepseek-harness\default-workspace\Vector\screens`.
Everything here is copied from those images; the only systematic change is the
product name: wherever the screenshots say the original product name, AnyVector says
**AnyVector** (e.g. `Tools > Options > AnyVector...`, `About AnyVector...`, `AnyVector Community`).

Theme: dark. Window/desktop background `#262626`, bars `#2b2b2b`/`#262626`,
separators `#3c3c3c`, text `#d4d4d4`, disabled text `#6e6e6e`, accent/selection blue
`#1f4e8c` (dialog OK button `#1d4a85`, hover rows `#3a3a3a`), focused field highlight `#0a6fd6`.
A Light theme (Tools > Options > Customization > Appearance > Theme, as in the reference: Light /
Dark) swaps these for light-gray bars `#f0f0f0`, white fields, near-black text `#1e1e1e`, separators
`#c8c8c8` and pale-blue selection `#cce2f8`; the palette lives in `theme.rs` (`theme::LIGHT` / `theme::DARK`).
Fonts: Segoe UI 9pt-ish (≈12.5 px).

## 1. Main window layout (top → bottom)

1. **Menu bar** (File mode): `File` `Tools` `Window` `Help` (the `Purchase` menu was removed at the product owner's request)
2. **Standard toolbar**: New(green +) · Open · ▾(Open Recent) │ Save · Open from Cloud · Save to Cloud · Print │ Cut · Copy · Paste │ Undo ▾ · Redo ▾ │ Import · Export · Publish to PDF │ Zoom combo `100%` ▾ · Full-Screen Preview │ Show Rulers · Show Grid · Show Guidelines │ Snap Off(magnet) · `Snap To ▾` │ `✦ AI Generate ▾` │ Options ⚙ (menu)
3. **Property bar** (no selection / no document): Page size combo `Letter ▾` │ Page width / Page height (two stacked spin fields) │ Portrait · Landscape │ All Pages · Current Page │ `Units:` combo │ Nudge distance `0.0 "` (move icon, spin) │ Duplicate distance X `0.25 "` / Y `0.25 "` │ Treat as filled (toggled, blue) │ two transform-option toggles │ `+` (customize)
4. **Left toolbox** (flyout triangle = has group): Pick ◢ · Shape ◢ │ Crop ◢ · Zoom ◢ │ Freehand ◢ · Artistic Media ◢ · Rectangle ◢ · Ellipse ◢ · Polygon ◢ │ Text ◢ │ Parallel Dimension ◢ · Connector ◢ │ Drop Shadow ◢ · Transparency │ Color Eyedropper ◢ · Interactive Fill ◢ │ Painterly Brush │ `+` (Quick Customize)
5. **Document tab strip**: Home (Welcome screen) · documents… · `+` (new document)
6. **Drawing window** (rulers when a document is open), right edge: **color palette** column (scroll ▲, eyedropper, No-Color well, swatches, ▼, `»`), dock `>` at top.
7. **Document palette** strip: placeholder text *Drag colors (or objects) here to store these colors with your document*, `>` `»` buttons at the right.
8. **Status bar**: ⚙ (menu: ● Tool Hints / Object Details / Cursor Coordinates / Document Color Settings) · `Tool Hints` · `Object Information` … right side: fill icon + swatch `Fill Color` · pen icon + swatch `Outline Color` · │ document color settings icon.

### Quick Customize (toolbox `+`) popup
Check-list grouped by flyout, blue rule between groups; bottom buttons `Reset toolbar`, `Customize...`.
- **Pick Tools**: Pick, Freehand Pick, Free Transform
- **Edit Shape Tools**: Shape `F10`, Smooth, Smear, Twirl, Attract and Repel, Smudge, Roughen
- **Crop Tools**: Crop, Knife, Virtual Segment Delete, Eraser `X`
- **Zoom Tools**: Zoom `Z`, Pan `H`
- **Curve Tools**: Freehand `F5`, 2-Point Line, Bézier, Pen, B-Spline, Polyline, 3-Point Curve, LiveSketch, Smart Drawing `Shift+S`
- **Artistic Media Tools**: Artistic Media `I`
- **Rectangle Tools**: Rectangle `F6`, 3-Point Rectangle
- **Ellipse Tools**: Ellipse `F7`, 3-Point Ellipse
- **Object Tools**: Polygon `Y`, Star, Spiral `A`, Graph Paper `D`, Common Shapes
- **Text Tools**: Text `F8`, Table
- **Dimension Tools**: Parallel Dimension, Horizontal or Vertical Dimension, Angular Dimension, Segment Dimension, 3-Point Callout
- **Connector Tools**: Straight-Line Connector, Right-Angle Connector, Rounded Right-Angle Connector, Edit Anchor
- **Interactive Tools**: Drop Shadow, Inner Shadow, Block Shadow, Contour, Blend, Distort, Envelope, Extrude
- **Transparency Tools**: Transparency
- **Eyedropper Tools**: Color Eyedropper `E`, Attributes Eyedropper
- **Interactive Fill Tools**: Interactive Fill `G`, Mesh Fill `M`, Smart Fill
- **Painterly Brush Tools**: Painterly Brush

(The first three groups are visible in the screenshot; the rest of the list is below the
fold and follows the same toolbox order.)

## 2. Menus

### File
| Item | Shortcut | Icon | Notes |
|---|---|---|---|
| New... | Ctrl+N | page+green plus | |
| New from Template... | | template | |
| Quick AI Generate... | | sparkle | |
| Open... | Ctrl+O | folder | |
| Open from Cloud... | | folder+cloud | |
| Open Recent ▸ | | | |
| — | | | |
| Close | | page+x | disabled w/o doc |
| Close All | | pages+red x | |
| — | | | |
| Save... | Ctrl+S | disk | |
| Save As... | Ctrl+Shift+S | disk | |
| Save to Cloud... | | disk+cloud | |
| Save as Template... | | template disk | |
| Revert | | revert | |
| — | | | |
| Acquire Image ▸ | | | Select TWAIN Source... · Select WIA Source... · — · Acquire... |
| — | | | |
| Import... | Ctrl+I | import | |
| Export... | Ctrl+E | export | |
| Export For ▸ | | | |
| Export Docker ▸ | | | |
| Send To ▸ | | | |
| Publish to PDF | | PDF | |
| — | | | |
| Print Merge ▸ | | | |
| — | | | |
| Print... | Ctrl+P | printer | |
| Print Preview... | | preview | |
| — | | | |
| Document Properties... | | doc info | |
| — | | | |
| Exit | Alt+F4 | power | |

### Tools
- Options ▸ — `AnyVector... Ctrl+J` · `Customization...` · `Tools...` · `Global...` · `Workspaces...`
- Save Settings as Default... (gear icon, disabled w/o doc)
- —
- Color Management...
- —
- Scripts ▸ — `Script Editor... Alt+F11` · `Run Script...` · — · `Visual Studio Editor... Alt+Shift+F12` · — · `Start Recording` · `Pause Recording` · `Stop Recording Ctrl+Shift+O` · `Cancel Recording` · — · `Record Temporary Script Ctrl+Shift+R` · `Run Temporary Script Ctrl+Shift+P` · — · `Scripts Alt+Shift+F11`

### Window
- New Window · Refresh Window `Ctrl+W` · Close Window `Ctrl+F4` · Close All
- — Cascade · Tile Horizontally · Tile Vertically · Combine Windows · Dock Window
- — Workspace ▸ · — · Dockers ▸ · Toolbars ▸ · Color Palettes ▸ · — · Window List
- **Workspace ▸**: Lite · ● Default · Touch ▸ · Specialty ▸ · — · New... · Delete · Import... · Export...
- **Dockers ▸**: Properties `Alt+Enter` · Objects · Pages · Comments · Object Data · Object Styles `Ctrl+F5` · Symbols `Ctrl+F3` · Find and Replace `Ctrl+F` · Export · AI Generate │ Transform `Alt+F7` · Coordinates │ Live Guides · Guidelines │ Align and Distribute `Ctrl+Shift+A` · Fit Objects to Path · Step and Repeat `Ctrl+Shift+D` │ Shape · Corners · Join Curves │ Text `Ctrl+T` · Font Sampler · Glyphs `Ctrl+F11` │ Effects ▸ │ Color · Palettes · Color Styles `Ctrl+F6` · Color Proofing │ Assets · Tray │ Links and Rollovers · Sources │ Views `Ctrl+F2` · History · Scripts `Alt+Shift+F11` │ Learn
- **Toolbars ▸**: Default Button Size ▸ (● Small · Medium · Large) · — · ✓Menu bar · ✓Status bar · ✓Standard · ✓Property bar · ✓Toolbox · Zoom · Text · Layout · Transform · Macros · Internet · Project timer · Parallel Drawing · — · ✓Lock Toolbars
- **Color Palettes ▸**: Add Colors from Selection... · Add Colors from Document... · — · Create Palette from Selection... · Create Palette from Document... · Open Palette... · Close All Palettes · — · Palette Editor... · Palettes · — · Recent ▸ · — · ✓Default Palette · ✓Document Palette · Color Styles Palette

### Help
Product Help... · — · Welcome Screen · Video Tutorials... · Learn · Quick Start Guide... · User Guide... · — · What's New?... · — · Updates... · Message Settings · AnyVector Community · AnyVector Support... · Submit Ideas and Feedback... · Report a Bug ▸ (Report a Bug... · View Reported Bugs...) · — · Refresh your Account · Account Settings... · — · About AnyVector...

### Purchase
Removed at the product owner's request (2026-11-08).

### Options ⚙ (standard toolbar) menu
AnyVector... `Ctrl+J` · Customization... · Tools... · Global... · Workspaces...

## 3. Options dialog
Title `Options`, close ✕. Header row: category title (left, bold) and six category
buttons (right): Document · AnyVector · Customization · Tools · Global · Workspaces.
Left list, right page, bottom: `?` · `OK` (blue) · `Cancel`.

### AnyVector (application)
- **General** — Getting started: *On AnyVector start-up:* [Welcome Screen ▾]; ☑ Show New Document dialog box │ Undo levels: Regular [150]
- **Tablet Mode** — ☑ Auto - switch between Tablet and Desktop workspaces; When switching to: Desktop mode [Default ▾], Tablet mode [Touch ▾]
- **Display** — ☐ Proof colors │ Other controls: ☑ Interruptible refresh, ☐ Manual refresh, ☑ Auto-panning, ☑ Show tooltips, ☑ Antialias bitmaps in enhanced view, ☐ Hide bounding box for curve tools, ☐ Highlight outline for selected objects │ Full-screen preview: ○ Use normal view ● Use enhanced view; ☑ Show page border │ Navigation: Default action for mouse wheel [Zoom ▾]; Zoom rate [15 %] Reset; Alternate Zoom (Ctrl+Shift) rate [100 %] Reset
- **Edit** — Constrain angle [15.0] degrees; Drawing precision [3] decimal places; ☐ Double-click to edit bitmaps in an image editor │ Visibility of inactive objects: slider [20] % Reset; *When editing objects, this setting controls the visibility of objects that are not currently active.* │ ☑ Double-click to activate Focus Mode
- **Nodes and Handles** — Node size [Small ▾] │ Node shape: Cusp [□▾] Smooth [○▾] Symmetrical [◇▾] Reset; ☑ Show curve direction when editing; ☐ Enable node tracking │ Colors: Color scheme [Default ▾]; Main color [blue] Reset; Secondary color [red] Reset; ☑ Show highlight; ☐ Show node types in different colors
- **PowerClip** — Drag content inside PowerClip: [PowerClip with content ▾]; ● Ignore PowerClip frame ○ Add content to PowerClip frame ○ Replace existing content │ Auto-center new content: ● When content is completely outside frame ○ Always ○ Never; note │ ☑ Show lines in empty PowerClip frames: ● Always (including print and export) ○ On screen only │ note
- **PowerTRACE** — Quick Trace method: [Clipart ▾]
- **Snapping** — ☑ Snap to objects (Alt+Z); ☑ Snap objects to themselves (Ctrl+Shift+H); ☐ Snap to page; Snapping radius [10] pixels; ☑ Show Snap location marks; ☑ Show snapping mode in screen tip │ Modes list: Node, Intersection, Midpoint, Quadrant, Tangent, Perpendicular, Edge, Center, Text Baseline (each ☑ with symbol) │ Select All · Deselect All
- **Save** — Backup: ☑ Back up original file before saving; Back up to: ● Same folder as original file ○ Choose location: [path] Browse... │ Auto-backup: ☑ Back up every [20 ▾] minutes; Back up to: ● Temporary folder ○ Choose location [path] Browse... │ Save documents to previous versions: ☑ Ask when saving; ● Keep appearance (suitable for limited edits) ○ Keep editable (may change appearance) │ ☑ Embed fonts when saving files │ Set default version [1.0 ▾]
- **Text** — tabs `Display` `Paragraph Text` `Fonts` `Spelling` `QuickCorrect`
  - Display: ☑ Show handles while editing; ☐ Auto-keyboard switching; ☐ Match font when inserting text in another language; Keyboard text increment [1] points; Default text units [points ▾]
  - Paragraph Text: ☑ Show linking of text frames; ☑ Show text frames; ☐ Expand and shrink paragraph text frames to fit text; When editing, apply text frame formatting to: ○ All linked frames ○ Selected frames only ● Selected and subsequent frames
  - Fonts: Font list contents; Number of most recently used fonts displayed [5]; ⟳ Update remote fonts; ☐ Enable remote fonts; [Panose Font Matching...]
  - Spelling: ☑ Perform automatic spell checking; Show errors in: ○ All text frames ○ Selected text frame only; Spelling suggestions [0]; ☐ Add corrections to QuickCorrect; ☐ Show errors which have been ignored
  - QuickCorrect: ☐ Capitalize first letter of sentences; ☑ Correct two initial, consecutive capitals; ☑ Capitalize names of days; ☑ Automatically create hyperlinks when typing web addresses │ Language [English (United States) ▾]; ☑ Change straight quotes to typographic quotes; ☐ Use straight quotation marks after numbers (e.g. 6'); Single Quotes Open [‘▾] Close [’▾]; Double Quotes Open [“▾] Close [”▾] │ Replacement text: ☑ Replace text while typing; Replace [ ] With [ ] Add/Delete/Replace; list `(c ©`, `(c) ©`, `(r ®`, `(r) ®`, `-- –`, `--- —`
- **Pen Settings** — stroke test area; *Apply at least three strokes with your stylus using a full range of pressure, and click Save.*; Interface: ○ WinTab - not installed ● Real-Time Stylus - installed; explanation; [Save]
- **Warnings** — *Show warnings when:* checklist (≈70 items, all ☑, alphabetical)
- **VBA** (here: **Scripts**) — Security: [Security Options...] (dialog with tabs Security Level / Trusted Publishers); ☑ Trust all installed script modules │ ☑ Delay Load scripting engine
- **Plug-ins** — Plug-ins Folders list with checkboxes; Add... · Edit... · Remove

### Customization
- **Appearance** — ☑ Center dialog boxes │ Size: slider [100 %] Reset to Default; explanation │ Color: Theme [Light / Dark ▾]; Window border [■▾]; Desktop [■▾]
- **Command Bars** — Global button size [Small ▾]; checklist (Menu bar, Context menu bar, Status bar, Project timer, Layout, Zoom, Internet, Property bar, Toolbox, Macros, Standard, Parallel Drawing, Text, Transform); New · Reset │ Button properties: Size [Small ▾], Style [Default ▾] │ Toolbar properties: ☑ Show title when toolbar is floating; ☑ Lock toolbars │ Menu bar mode: ○ Normal mode ● File mode
- **Commands** — category combo [File ▾]; Search…; command list with icons; tabs `General` `Shortcut Keys`; Caption [&Acquire...]; Icon (preview) Browse... / Restore Defaults; Tooltip; Available on: …
- **Color Palette** — ☑ Automatically update the document palette │ Palette options: Maximum palette rows when docked [1] │ Color wells: ☐ Wide borders ☐ Large swatches ☑ Show "No Color" well │ Secondary mouse button: ○ Context menu ● Set outline color │ hint

### Tools
Pick · Eraser · Zoom/Pan · Freehand/Bézier · Smart Drawing · Rectangle · Ellipse · Graph Paper · Spiral · Table · Dimension · Angular Dimension · Connector · Mesh Fill
- Pick: ☐ Crosshair cursor; ☑ Treat all objects as filled; Ctrl and Shift keys: ● Traditional AnyVector: Ctrl = Constrain, Shift = Transform from center ○ Windows standard: Ctrl = Duplicate/Leave Original, Shift = Constrain
- Eraser: Thickness [0.25 "]; ☑ Auto-reduce nodes of resulting objects
- Zoom/Pan: Zoom tool secondary button ● Zoom out ○ Context menu; ☑ Zoom relative to 1:1; ☐ Center mouse when zooming with the mouse wheel │ Pan tool secondary button ● Zoom out ○ Context menu │ [Calibrate Zoom...] note
- Freehand/Bézier: Freehand smoothing slider [50]; Corner threshold [5] px; Straight line threshold [5] px; Auto-join [5] px
- Smart Drawing: Drawing assistance delay slider [0.0] second(s)
- Rectangle: corner type buttons (round/scallop/chamfer); Corners: four radii [0.0 "] with lock; ☐ Scale corners
- Ellipse: Type ● Ellipse ○ Pie ○ Arc; Starting angle [90.0]; Ending angle [90.0]; Direction ○ Clockwise ○ Counterclockwise (disabled for Ellipse)
- Graph Paper: Number of columns [1], rows [1]
- Spiral: Type ● Symmetrical ○ Logarithmic; preview; Number of revolutions [4]; Expansion slider [100] (disabled for symmetrical)
- Table: Rows [3]; Columns [4]; When you edit a table, pressing Tab lets you: ○ Insert a tab character ● Move to the next cell; Tab order [Left to right, top to bottom ▾]
- Dimension: Style [Decimal ▾]; Precision [0.00 ▾]; Units [" ▾]; Prefix; Suffix
- Angular Dimension: Precision [0.00 ▾]; Units [degrees ▾]; Prefix; Suffix
- Connector: ☐ Use geometric anchor points as snapping points; Route distance from object [0.0 "]
- Mesh Fill: Number of columns [2]; rows [2]

### Global
General · Printing · File Locations · File Formats · Color · User ID · Privacy
- General: ☑ Enable background tasks; User interface language [English-US ▾]; ☐ Ask me the next time the application starts; ☑ Use hardware acceleration [GPU ▾]; ☑ Display ☑ Effects ☐ Art Styles ☐ Resampling ☐ JPEG Artifact Removal; *You need to restart the application to apply these changes.*
- Printing: tabs Print Style / Driver Compatibility; Default print style [AnyVector Defaults ▾]; ☐ Print only current page; Special settings (Spot color separations warning, Preview color, Preview separations, Preview image, Overprint black threshold (PS) 95, Send large bitmaps in chunks No, Bitmap output threshold (K) 4096, Bitmap chunk overlap 32 pixels, Small fonts (Preflight) 7, Image resolution too low (Preflight) 96, Composite crop marks (PS) Output on all plates, PostScript 2 stroke adjust (PS) On, Many fonts (Preflight) 10, NT double download workaround (PS) On, NT bookman download workaround (PS) On, Render to bitmap resolution Automatic dpi)
- File Locations: table Content Type / Location (Fills, Adjustment Presets, Artistic Media, Bitmap Presets, Blend, Cloud, Contour, Distortion, Documents, Envelope, Export Presets, Find and Replace, Fonts, Image Lists, Images, Palettes, Photo Frames, Print Styles, Shadow, Symbols, Templates, Trays, Vector Extrude, Writing Tools); Reset · Edit
- File Formats: Available file types tree (Raster, Vector, Text, Animation); Add · Remove · Move Up · Move Down; Active filters list; Reset to Default
- Color: Spot color palette [..▾]; Color palette mapping: Spot colors [..▾]; ☑ Show Choose Color Palette dialog box
- User ID, Privacy
