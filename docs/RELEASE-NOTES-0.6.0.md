# AnyVector 0.6.0

AnyVector 0.6.0 closes 27 reports from CorelDRAW users. You can now save a copy CorelDRAW opens,
select behind objects and inside PowerClips, click shadows, set your default outline in a new
Outline Pen dialog, work in inches with fractional rulers, and customize the toolbars by dragging.

## Working with CorelDRAW files

- **Save a copy CorelDRAW opens (#132).** File ▸ Save As offers **CMX - CorelDRAW-compatible
  copy**, plus PDF, SVG and EMF copies. The CMX copy keeps pages, layers, shapes, curves, solid and
  gradient fills (RGB and CMYK) and outlines with dashes, caps and joins. Text becomes curves. After
  the save, AnyVector lists anything CMX can't hold, such as bitmaps, shadows and transparency.
  AnyVector's own `.cdr` is now labelled as such. The first time you save one, a note explains that
  CorelDRAW can't read it, and Options ▸ Save turns that note off. Opening a `.cdr` that no one
  recognises now explains why.

## Selecting and editing

- **Alt+click selects the object behind (#139).** Each Alt+click goes one level deeper and wraps
  back to the top. Shift+Alt+click adds the next object to the selection.
- **PowerClips (#111, #112):**
  - Alt+click selects a shape inside a PowerClip without entering it.
  - Ctrl+click or a double-click enters the PowerClip to edit its contents, and Ctrl+click
    outside, Esc or Finish leaves it.
  - Selecting a PowerClip shows a floating toolbar under it: Edit, Select Contents, Extract,
    Lock Contents, and the contents placement options.
- **Shadows (#105, #106):**
  - Click a drop or block shadow to select it and get its property bar. Delete then removes only
    the shadow.
  - Break Apart (Ctrl+K) separates a shadow from its object: a block shadow becomes a curve, and
    a drop or inner shadow becomes a bitmap.
- **Right-drag menu (#140).** Drag a shape with the right mouse button onto another one to get
  Move Here, Copy Here, Copy Fill Here, Copy Outline Here, Copy All Properties and PowerClip Inside.
- **Space while dragging (#130)** leaves a copy where the object is and keeps the drag going.
  **Keypad +** duplicates in place (#119).
- **Shape tool:**
  - The arrow keys bend a selected curved segment (#142).
  - Text gets CorelDRAW's character and line spacing handles; hold Shift for word and paragraph
    spacing (#110).
  - Virtual Segment Delete highlights the piece it will remove and shows its own pointer (#119).
- **Freehand with Ctrl (#134)** draws a straight line constrained to the angle steps.
- **Double-click the Rectangle tool (#135)** with objects selected to draw a rectangle around
  them.

## Outlines, units and rulers

- **Hairline outlines show as "Hairline" (#127, #133)** in the status bar, the property bar and
  the dockers, instead of "None".
- **The status bar shows the outline thickness (#138).** Double-click it to open the new
  **Outline Pen** dialog (F12). With nothing selected, the dialog sets the default outline for
  new objects (#103).
- **Outline widths follow the outline unit you pick (#137)**, and font sizes follow the text unit
  in Options ▸ Text (#109). You can type a value in any unit.
- **Inch rulers are divided into fractions (#113):** 1/2, 1/4, 1/8 and 1/16.
- **A new install takes its unit and page size from the Windows regional settings (#120):**
  millimetres and A4, or inches and Letter.

## Workspace

- **Customize the toolbars by dragging (#124).**
  - Alt+drag a button to move it, or drag it off its bar to remove it. Ctrl+Alt+drag copies it.
  - Drag commands from Options ▸ Customization ▸ Commands onto any bar.
  - Right-click ▸ Customize offers Reset, Add Separator and the button styles.
  - Layouts are saved with the workspace.
- **The Command Bars page is readable (#136)**, and New, Delete and Reset work.
- **Shortcut keys (#125).** Every command, tool and shortcut-only command can get a key on the
  Commands page. It shows conflicts and has Assign, Delete and Reset.
- **Tab selects a field's value (#121)**, so you can type the new number straight away.
- **The page border stays visible on any desktop colour (#141)**, including a white page on a
  white desktop. The Window border colour now colours the title bar and frame.
- **Wireframe view shows bitmaps as a light ghost (#129)** instead of an empty box.
