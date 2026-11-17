# AnyVector 0.5.3

AnyVector 0.5.3 fixes the bugs reported after 0.5.2 and makes PANTONE palettes easier to work with.

## Colors

- **Your own PANTONE colors.** The PANTONE color books are licensed, so AnyVector can't include them. Clicking a
  greyed-out PANTONE palette now says so and opens Open Palette, where you can open one you own. Open Palette now
  also reads `.ase` color books exported from Adobe programs. A spot color applied from a palette keeps its name in
  the document. Opening a CorelDRAW file tells you how many PANTONE colors are shown as grays.

## Fixes

- **Treat All Objects as Filled turns off again.** Its button now really switches it; a second checkbox in
  Options ▸ Document ▸ Layout kept it on and is gone. It starts off.
- **Ellipses stay closed after Convert to Curves.** Clicking inside a converted ellipse picks it again. Drawings saved
  with such open shapes are fixed when you open or import them.
- **Break Apart keeps text as text.** Breaking apart artistic text splits it into lines, then words, then single
  characters, each still editable and in the same place with the same formatting. Warped text still becomes curves.
- **A page-sized rectangle in one double-click.** Double-click the Rectangle tool to draw a rectangle the size of the
  page.
