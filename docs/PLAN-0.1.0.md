# AnyVector 0.1.0 — plan

Goal: make AnyVector match the CorelDRAW 2026 reference: menus, order, shortcuts, which items are enabled
for each selection, dialogs, dockers, tools, toolbars and right-click menus. Nothing is invented: what is not
in the reference is not added.

## Sources

| What | Where |
|---|---|
| Screenshots of every menu, dialog, docker, tool, toolbar and right-click menu, by selection state | `C:\Users\anton\Documents\Claude\corel-screenshots\MAPEAMENTO` (browse with `index.html`) |
| Every menu item with its order and the selection states that enable it | `MAPEAMENTO\01-inventario\inventory.csv`, `MAPEAMENTO\90-relatorios\matriz_contextos.csv`, `cobertura_final.csv` |
| What each function does | `MAPEAMENTO\CORELDRAW_FUNCTIONALITIES_FROM_HELPER` |
| Architecture reference | `C:\Users\anton\Documents\deepseek-harness\default-workspace\Vector\` |
| Progress, item by item | `docs/COVERAGE.md` (from `cargo run -p xtask -- coverage`; statuses in `docs/coverage-status.tsv`) |
| AnyVector renders next to the reference | `target/visual/compare.html` (from `cargo run -p xtask -- compare`; pairs in `docs/reference-map.tsv`) |

## Decisions

- **Icons**: AnyVector draws its own icons, but each keeps the meaning of the reference icon (a pen stays a pen).
  Corel's artwork is never copied.
- **Left out**: the Purchase menu, the hidden Debug menu, Corel Cloud (Open from Cloud, Save to Cloud, Sign out of
  Cloud), Corel accounts (Refresh your account, Account Settings, Restore Purchases), Corel Support, the Learn
  docker and Help items (Learn, Video Tutorials, CorelDRAW Community) and Share Your Experience.
- **WordPress publishing and Send To ▸ Mail**: to be built; skipped only if they turn out to be too much.
- **Edit Bitmap**: opens the bitmap in the user's default image editor and brings the result back (instead of
  Corel PHOTO-PAINT); skipped only if no generic way works.
- **Acquire Image**: implemented for real scanners (Windows scanner support).
- **Branding**: `anyvector-icon.svg` is the program icon (exe, window, shortcuts, file types, installer icon);
  `anyvector-logo.svg` is the logo (splash screen, About box, installer side picture).

## Batches

Each batch ends with a build, the tests, renders compared with the reference, an updated `docs/COVERAGE.md`
and a pull request.

| # | Batch | Contents |
|---|---|---|
| 0 | Foundation and branding (done) | Version 0.1.0; icon, logo and installer pictures from the SVGs; the installer; Help ▸ About with System Info and Legal Notices; the coverage report and the comparison page. |
| 1 | Program frame (done) | All 12 menus and submenus in order, enabled per selection state (17 states) and the reduced menu bar without a document; right-click menus in 23 places; Standard toolbar, status bar and the 8 hidden toolbars; toolbox and flyouts; property bar of each of the 60 tools and of the Pick tool for each selection; Welcome Screen. |
| 2 | File, Edit and View | The File menu dialogs (Document Properties, Print with all tabs, PDF settings, Export, Acquire Image, Send To, Export For), the Edit and View dialogs and modes. |
| 3 | Layout, Object and Table | The Layout dialogs; Object (PowerClip, Symbols, Rollover, Align, Order and the rest); Table dialogs. |
| 4 | Text | The Text dialogs; Text, Font Sampler, Glyphs and Find and Replace dockers; writing tools. |
| 5 | Tools | Options checked page by page against the reference; Color Management, Customize, Scripts, Object Data Manager. |
| 6 | Dockers | All 45 dockers with their inner states (the 7 Properties tabs for each kind of selection, Objects, Pages, Transform, Align and Distribute, Shape, Step and Repeat, Live Guides and the rest). |
| 7 | Effects and Bitmaps | The 120 bitmap effects of Properties ▸ Bitmap Effects; Effects menu (Lens, Contour, Blend, Extrude, Bevel, Envelope, Pointillizer, PhotoCocktail, Artistic Media); Bitmaps menu (tracing, Image Adjustment Lab, resample, color modes, Edit Bitmap). |
| 8 | Window, Help and release | Workspaces and the Window menu; Help; remaining gaps; the 0.1.0 installer and documentation. |
