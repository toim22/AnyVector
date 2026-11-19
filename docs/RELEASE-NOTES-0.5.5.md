# AnyVector 0.5.5

AnyVector 0.5.5 fixes the picking regression from 0.5.3 and makes the Shape tool, Virtual Segment Delete, Trim,
the shadow sliders and the rotation center work the way they do in CorelDRAW.

## Fixes

- **Clicking inside a shape selects it again (#116).** "Treat all objects as filled" is back on by default, as it
  was before 0.5.3, so an outline-only shape can be picked from its inside. Settings saved by 0.5.3 or 0.5.4 are
  switched back once; the property-bar button still turns it off.
- **Virtual Segment Delete works on every shape (#117).** It used to work only on curves. Rectangles, ellipses,
  polygons and spirals are now cut at the clicked segment; the shape becomes a curve in the same undo step.
- **Trim no longer deletes a shape (#118).** Trim keeps the object you trim with, as CorelDRAW's property bar does.
  New **Leave original source** and **Leave original target** buttons on the property bar change that for Trim,
  Intersect and Front/Back Minus. When the objects don't overlap nothing is changed and the status bar says so.
  Weld still joins its pieces into one object.
- **Shape tool (#104, #108, #122, #123):**
  - Polygon and star nodes can be dragged; all matching nodes move together and the shape stays a polygon.
  - Drag an ellipse's node inside it to make a pie, or outside to make an arc; pies and arcs get start and end nodes.
  - Click a segment to select it; drag a straight segment to move it, or a curved one to bend it.
  - Double-click a node to delete it; `+` and `-` (keypad too) add and delete nodes, and Add puts the new node
    between the two selected neighbours.
  - Curved segments show both control handles, and the pointer changes over nodes and segments.
- **Shadow sliders work (#107).** Opacity, feathering and the other shadow sliders can be dragged, change the
  shadow live, and one drag is one undo step. Changing them with the Drop Shadow tool active adds a shadow.
- **The rotation center stays where you put it (#126).** Duplicate, Clone, Paste and Step and Repeat keep a moved
  rotation center, undo and redo restore it, and dragging it snaps to the selection's own corners, midpoints,
  center and nodes.
