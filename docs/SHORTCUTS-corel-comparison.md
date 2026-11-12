# Keyboard shortcuts: AnyVector vs CorelDRAW 2026

Reference: the CorelDRAW 2026 (27.2.0.135) shortcut map extracted read-only from the
installed `Draw\UIConfig\DrawUI.xml` — 422 CorelDRAW bindings, 197 in the global table and 225
in 10 context tables. PHOTO-PAINT, Font Manager and CAPTURE are out of scope.

## Languages

Corel's key chords are one table with no locale: they are the same in every UI language. Only
the Alt mnemonics (the `&` in captions) move with a translation. AnyVector is the same: chords
come from `commands::default_shortcut` / `alt_shortcuts` / `context_shortcuts` and the `keys`
of `ui/data.rs`, none of which go through the translator, so all 15 UI languages share them.
Translated captions carry their own `&` mnemonic per language in `assets/i18n/<code>.tsv`; only
English Corel strings were available, so foreign mnemonics were not checked against Corel.

## Where the chords live

- `ui/data.rs` `keys`: what menus show. Generated from Corel's UIConfig, so already the
  reference's chords (not edited).
- `commands::default_shortcut`: the chord dispatch uses (one per command).
- `commands::alt_shortcuts`: the reference's further chords for a command (Ctrl+Insert,
  Shift+Insert, ...).
- `commands::context_shortcuts`: the reference's context tables (text editing, Shape tool
  nodes, Table tool cells).
- A test (`commands::tests::menu_keys_match_dispatch`) checks that every chord a menu item shows
  is the default or an alternate of the command the item runs.

## Result (per reference binding)

| Category | Bindings |
|---|---|
| Global, already the same | 81 |
| Global, same through the hard-coded arrow nudges | 12 |
| Global, added now | 35 |
| Global, chord corrected | 2 |
| Global, alternate chord (needs multi-chord dispatch) | 12 |
| Global, command does not exist in AnyVector | 54 |
| Context, mapped (`context_shortcuts`, needs context dispatch) | 48 |
| Context, handled by the text editor itself | 35 |
| Context, command does not exist in AnyVector | 78 |
| Dialog-local tables (Print Preview, image view/edit, Configure Data Source) | 64 |
| Reference entry without a key | 1 |
| **Total** | **422** |

## Changed

Corrected:

- Align Centers Horizontally: E → **C**; Align Centers Vertically: C → **E**. The menus
  already showed C and E on those items (and the reference's icons agree: C lines the centers
  up on a vertical line), but the keys ran the other command.
- Zoom In: F2 → **Ctrl++** (F2 is Corel's Zoom One-Shot, a different command; Zoom In was not
  dispatched at all). Alternates Ctrl+Num +, Ctrl+=.

Added (the command existed, the chord did not): Painterly Brush J, LiveSketch S, Paste in View
Ctrl+Shift+V, Center to Page P, Distribute Left/Centers H/Right/Spacing H/Top/Centers V/Bottom/
Spacing V Shift+L/E/R/P/T/C/B/A, Join Curves Ctrl+Shift+J, Lens Alt+F3, Contour Ctrl+F9,
Envelope Ctrl+F7, Transform docker Rotate/Scale and Mirror/Size Alt+F8/F9/F10, Align to
Baseline Alt+F12, Convert (text) Ctrl+F8, Spell Check Ctrl+F12, Bullets Ctrl+M, Light Ctrl+B,
Color Balance Ctrl+Shift+B, Hue/Saturation/Lightness Ctrl+Shift+U, Create New Symmetry Alt+S,
Remove Symmetry Alt+Shift+S, Break Symmetry Link Alt+X, Edit Symmetry Alt+Shift+Y, Finish
Editing Symmetry Alt+Shift+E, Exit Focus Mode Shift+Esc, Sample Mesh Fill Color Ctrl+Shift+E,
Toggle Pick State Ctrl+Space, Visual Studio Editor Alt+Shift+F12 (had a chord, was not
dispatched).

New commands (small, reuse existing code): `TransformTab` (Transform docker on a page),
`TogglePickState` (what Space already does), `NodeToggle` (Shape tool C/S/L: toggle cusp/smooth,
smooth/symmetrical, line/curve on the selected nodes).

Alternates (in `alt_shortcuts`): Delete Backspace; Undo Alt+Backspace; Cut Shift+Delete; Copy
Ctrl+Insert; Paste Shift+Insert; Zoom In Ctrl+Num +, Ctrl+=; Zoom Out Ctrl+-, Ctrl+Num -,
Ctrl+_; To Front Ctrl+Home; To Back Ctrl+End (AnyVector has one To Front/To Back command, so
"of Page" and "of Layer" run the same thing).

Dispatch order: `shortcut_commands` now lists the most specific chords first. egui matches a
chord when extra Shift/Alt are held, so before this Ctrl+Shift+S ran Save, Ctrl+Shift+Z ran
Undo, Ctrl+Shift+D ran Duplicate and Alt+Shift+R ran Align Right.

Spelling: chords use the reference's menu spelling (Shift+PgUp, Ctrl++, Ctrl+Num -, Ctrl+_,
Shift+Esc), which `parse_shortcut` now reads.

## Extra AnyVector chords

- Ctrl+Y = Redo (hard-coded in `shortcuts.rs`). Corel has no Ctrl+Y; nothing collides.
- F2 = Zoom In: removed (collided with Corel's Zoom One-Shot).
- E = Color Eyedropper (`tools::TOOLS`): removed from dispatch; Corel has no key for that tool
  and E is Align Centers Vertically. `tools/ids.rs` still lists it (help page).
- No other AnyVector chord is outside the reference.

## Missing commands (chord not bound because AnyVector has no such command)

Global: Define/Add to/Recall Selection Group 0–9 (Ctrl+n, Alt+n, n: 30 bindings), Pan
Left/Right/Up/Down (Alt+arrows), Navigator (N), Bring up Property Bar (Ctrl+Enter), Toggle View
(Shift+F9), Zoom One-Shot (F2), Fountain Fill (F11), Uniform Fill (Shift+F11), Outline Pen
(F12), Outline Color (Shift+F12), What's This? (Shift+F1), Font Size Increase/Decrease/Next/
Previous (Ctrl+Num 8/2/6/4), Vertical/Horizontal Text (Ctrl+. / Ctrl+,), Snap to Self
(Ctrl+Shift+H), Toggle preview colors (Ctrl+Shift+I), Show unselected nodes with fill
(Ctrl+Shift+G), Symmetry Show full preview (Alt+Shift+P), Mirror Symmetry (Alt+Shift+M).

Text editing: Font List, Font Size, Style List, Font Style, Small Capitals, HTML Font Size List,
Text docker (Ctrl+F10), Find and Replace Text (Alt+F3), Delete Word to Right, paragraph/frame/
text-start/end caret moves (Ctrl+Up/Down, PgUp/PgDn, Ctrl+PgUp/PgDn, Ctrl+Home/End and their
Shift variants), font size steps. Their chords are reserved in the text context so the global
command does not run while typing.

Table tool: cell navigation and selection with the arrows, Delete (&Table), Expand Selection
(Ctrl+A). Connector tool: anchor nudges (13). Dialog-local tables: Print Preview (30), image
view/edit zoom (22), Configure Data Source (12).

## Dispatch changes still needed in `shortcuts.rs`

1. Alternate chords: in `bindings`, when the user has not overridden a command, also push
   `alt_shortcuts(c)` (parsed) for it, then sort all bindings by number of modifiers, most
   first.
2. Context tables: before the global loop, when text is being edited use
   `context_shortcuts(KeyContext::Text)`; with the Shape tool and nodes selected
   `KeyContext::Nodes`; with Table tool cells selected `KeyContext::TableCells`. A matching
   chord runs its `Some(cmd)` (or nothing for `None`) and the global binding with that chord is
   skipped.
3. Nudges: the arrow loop tries `Modifiers::NONE` first, which also matches Shift+arrow, so
   Super Nudge never runs. Try SHIFT and CTRL before NONE. Alt+arrows (Pan) are also eaten as
   plain nudges.

Other notes: the Align and Distribute docker labels its buttons "Align centers horizontally
(E)" and "Align centers vertically (C)" (`dockers/arrange.rs`), which no longer matches the
keys. The help page lists tool keys from `tools::TOOLS`, which has no key for Painterly Brush
(J) and LiveSketch (S) and still has E for the Color Eyedropper.
