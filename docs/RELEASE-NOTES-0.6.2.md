# AnyVector 0.6.2

AnyVector 0.6.2 closes three reports from CorelDRAW users. You can now Ctrl+click into groups,
select and drag a blend's start and end objects, and place a dimension's text where you want it.

## Selecting inside groups

- **Ctrl+click selects inside a group (#151).** With the Pick tool, Ctrl+click picks the object
  under the pointer inside its group. Each Ctrl+click goes one level deeper through nested groups.
- **Ctrl+Shift+click** adds an object inside a group to the selection, or removes it.
- The picked object moves, duplicates and deletes on its own. Deleting the last object of a group
  removes the empty group too. The status bar shows it as "Child … on Layer".
- Ctrl still constrains a drag, and Ctrl+click still enters a PowerClip. An object in a locked
  group can be selected but not moved.

## Blends

- **Select a blend's start and end (#149).** A selected blend shows grips on its start, end and
  split objects, and its path is drawn dashed. Click a control or its grip to select it, or
  Ctrl+click it to go straight there.
- Drag a control to reshape the blend. The steps follow live while you drag, and Undo puts it back.
- Clicking a step gives you the whole blend again. This also works for blends inside groups.

## Dimensions

- **Text Position works (#150).** The Dimension property bar's Text Position options now place the
  value above, through or below the dimension line, centred or not, forced horizontal, and boxed.
  Text placed through the line breaks the line around it.
- Select one or more dimensions and pick an option to change them in one undo step. With nothing
  selected, the options set the defaults for the next dimensions you draw.
- PDF, SVG, EMF and CMX exports follow the text position. Clicking a dimension's value now selects
  the dimension, and its bounds include the value.
- Existing linear dimensions look the same as before. On angular dimensions the value now sits just
  outside the arc, on its middle.
