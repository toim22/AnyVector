# AnyVector 0.6.1

AnyVector 0.6.1 closes four reports from CorelDRAW users. You can now type with an input method
on the canvas, run blends along a path, hold a palette swatch for its color variations, and
arrange the dockers the way you like.

## Text

- **Input methods work on the canvas (#147).** Japanese, Chinese, Korean, Vietnamese and other
  input methods now type into text objects and table cells. The candidate window opens next to the
  caret, and the text you are composing shows in place, underlined, until you confirm it.
  Shortcuts wait while you compose, and Esc cancels only the composition.
- Characters the chosen font doesn't have, such as Japanese typed in Arial, now display in a
  system font that has them instead of as empty boxes. The text keeps its own font name.
- Rename and preset fields in the dockers and dialogs accept input-method text properly.
- Words in Chinese, Japanese, Korean or Thai are no longer marked as misspelled by a Western
  spelling dictionary.

## Blends

- **Blends along a path (#145).** In the Blend property bar, Path Properties ▸ New Path lets you
  click any curve or shape, circles included, for the blend to follow. Show Path and Detach from
  Path work too.
- **Blend along full path** and **Rotate all objects** work from the Options flyout and the Blend
  docker, and both always show the blend's current state.
- **Show Start** and **Show End** select that end of the blend, so you can drag it along the
  path. **New Start** and **New End** swap in another object.
- **Split**, **Fuse start**, **Fuse end**, **Map nodes** and fixed spacing along a path now work.
- The path is kept when you save the file, and PDF, SVG, EMF and CMX exports follow it.

## Color palette

- **Hold a swatch for its variations (#146).** Press and hold a color in the docked palette or
  the document palette for half a second to get a grid of its tints, shades and nearby hues.
  Click a variation for the fill or right-click it for the outline, or release the held button
  over one.
- Variations keep the color model of the swatch, so CMYK colors stay CMYK. Grays offer the
  gray scale, and spot colors offer tints of their ink.
- A quick click still applies the swatch straight away, and dragging swatches works as before.

## Dockers

- **Arrange the dockers (#144).** Drag a docker tab to reorder it or move it into another group.
  Dock it on the left or right of the drawing window, stack groups above each other, or float it
  in its own window and drop it back onto a tab strip. Double-clicking a tab floats it and docks it
  back.
- Right-click a tab for Dock Left, Dock Right, Float, Close and Close Others.
- Your arrangement is saved with your settings and with each workspace.
  **Window ▸ Dockers ▸ Reset Docker Layout** puts everything back in one group on the right.
