# Vector — Architecture

Vector is a Windows vector-graphics editor written in Rust. The UI follows the design screenshots in
`deepseek-harness/default-workspace/Vector/screens` (see `docs/UI-SPEC.md`).

## Workspace

| Crate | Role |
|---|---|
| `vector-core` | Document model: pages, layers, objects (rectangles, ellipses, polygons, spirals, curves, text, images, groups, PowerClips, tables, dimensions, connectors, blends, symbols), styles, effects, geometry (booleans, offsets, flattening), editable node paths, text layout, units, undo history and the editing operations (`ops`). |
| `vector-render` | CPU renderer on tiny-skia, used for the drawing window, export, printing and thumbnails. Handles gradients, patterns, mesh fills, transparency, merge modes, shadows, contours, extrusions, envelopes, distortions, bitmap effects and lenses. |
| `vector-io` | File formats: native `.vct`, SVG import/export, PDF export, PNG/JPEG/BMP/TIFF/WebP raster import/export. |
| `vector-app` | The egui/eframe application (binary `vector.exe`). |

## Application (`crates/vector-app/src`)

- `app.rs`: `VectorApp`, the window layout (menu bar, standard toolbar, property bar, toolbox, palette, dockers, status bar, document tabs, drawing window).
- `commands.rs`: the `Cmd` enum. Every menu item, button and shortcut produces a `Cmd`. `app.run(cmd)` queues it and `exec.rs` executes it. Commands are serializable, so the script recorder can replay them.
- `menus.rs`, `toolbar.rs`, `propbar.rs`, `toolbox.rs`, `statusbar.rs`, `palette.rs`, `tabs.rs`, `welcome.rs`: the UI chrome, following the screenshots.
- `canvas.rs`: the drawing window. It owns the rulers (drag from a ruler to create guidelines), the cached page render (re-rendered only when the history revision, view or size changes), mouse-wheel zoom and scroll, middle-button pan, the page navigator, the page sorter and full-screen preview.
- `tools/`:
  - `ids.rs`: tool list, flyout groups, names, icons and shortcuts.
  - `interact.rs`: pointer handling for every tool, plus snapping and text-editing keys.
  - `overlay.rs`: on-canvas feedback such as handles, nodes, guides, grid, previews, the caret and snap marks.
  - `shapes.rs`: the Common Shapes library.
  - `mod.rs`: tool state, node operations, corner rounding and text insertion.
- `dialogs/`: modal dialogs. `options.rs` is the Options dialog with every page from the screenshots; `forms.rs` holds New Document, Export, Print, Trace, Script editor, About and the others.
- `dockers/`: right-side dockers such as Properties, Objects, Transform, Align and Text.
- `trace.rs`: bitmap-to-vector tracing (k-means colour quantization, region cleanup, contour following, curve fitting).
- `scripting.rs`: macro recording and scripts.
- `ai.rs`: the optional AI Generate feature. It reads the API key from an environment variable.
- `settings.rs`: persisted application settings.

## Data flow

1. Input goes either to `shortcuts::handle` (keys → `Cmd`) or to `canvas::drawing_window` (pointer → `tools::handle_input`).
2. Tools change the document through `DocView::modify(label, |doc, selection| …)`, which records one undo step. Live drags (node editing, envelopes, brushes) record one checkpoint when the drag starts, then update the document directly.
3. The canvas re-renders when `history.revision` changes. The overlay is drawn every frame on top of the cached render.

## Coordinates

- Document units are points (1/72 in), with y pointing down.
- At 100% zoom, one point is 96/72 screen pixels.
- Each object stores its local geometry in the box (0,0)–(w,h) and is placed by its `transform`.

## Tests

```bash
cargo test --workspace
```

These are GPU snapshot tests: renders of the window, plus a smoke test that drives every tool with simulated mouse input. They write their images to `target/visual/`.

```bash
cargo test -p vector-app visual -- --ignored --test-threads=1
```

## Building the installer

```bash
cargo build --release -p vector-app
"$LOCALAPPDATA/Programs/Inno Setup 6/ISCC.exe" installer/vector.iss
```

The installer is written to `dist/Vector-1.0.0-setup.exe`.
