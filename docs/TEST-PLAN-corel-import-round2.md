# Manual test plan: CorelDRAW import, second round

System integration test (SIT) for the work merged in pull requests #35 to #44, before it ships as a release. The
automated tests and the import check passed. These cases cover what was never tried by hand in the running app: the
messages, the Properties docker, editing patterns, saving and reopening.

| | |
|---|---|
| Build under test | `main` at or after the merge of this plan |
| Test files | `C:\Users\anton\Documents\AnyVector-test\files` |
| CorelDRAW references | `C:\Users\anton\Documents\AnyVector-test\reference` (PNGs rendered by CorelDRAW) |
| Tester | |
| Date | |

## 1. How to run this plan

**Result codes.** Write one of these in each case's *Result* line:
- **P** (pass): every expected result happened.
- **F** (fail): at least one didn't. Note which step and what you saw. A screenshot helps.
- **B** (blocked): the case couldn't run, for example because an earlier case failed.
- **N/A**: doesn't apply.

**What counts as the same picture.** When a case says "compare with CorelDRAW", AnyVector should match it in kind:
- the same pattern, texture or drawing
- similar colors and density
- the same position and size

It doesn't need to match pixel for pixel. Where a case expects a known difference, the case says so.

**One file at a time.** Unless a step says otherwise, close the drawing before the next case and answer **No** to
saving. Never save over a file in the test folder; *Save As* goes to `C:\Users\anton\Documents\AnyVector-test\saved`.

## 2. Setup (do once)

| # | Step | Expected result |
|---|---|---|
| S1 | In a terminal at the repository folder run `cargo build -p anyvector --release`. | The build ends with `Finished`, no errors. |
| S2 | Create the folder `C:\Users\anton\Documents\AnyVector-test\saved`. | The folder exists. |
| S3 | Start `target\release\anyvector.exe`. | AnyVector opens. If it asks for the language, pick **English**. |
| S4 | Check **Tools > Options > Global > General > User interface language**. | It reads **English**. Close Options. |
| S5 | Open the docker **Properties** (**Alt+Enter**) and keep it open for the whole plan. | The Properties docker shows on the right. |
| S6 | (For the comparison cases) Start CorelDRAW 2026 and close its trial reminder. | CorelDRAW is ready. Open each file with it read-only and close it without saving. |

The test files (copies, safe to open):

| File | What it is |
|---|---|
| `5EFFECTS.CDR`, `COLORS.CDR` | CorelDRAW 5 drawings using the AvantGarde font (not installed) |
| `JOB.CDT`, `NEWS_BOX.CDT` | CorelDRAW 5 templates with empty paragraph frames |
| `COLORBAR.CDR` | CorelDRAW 3 drawing with text |
| `KING.CDR`, `LIZARD.CDR` | CorelDRAW 1/2 drawings (Arial Narrow; Bangkok, not installed) |
| `ARROW.CDR` | Corel clipart saved as CMX but named `.cdr` |
| `PRES.CDR` | A damaged CorelDRAW 3 file (0.2.0 refused it too) |
| `square_texturefill_nooutline.cdr`, `texture-cdr12.cdr` | CorelDRAW 12 texture fills |
| `square_postscriptfill_nooutline.cdr`, `postscript-cdr12.cdr` | CorelDRAW 12 PostScript fills |
| `square_patternfill_nooutline.cdr` | CorelDRAW 12 pattern fill |
| `vector-pattern-x5.cdr`, `vector-pattern-2026.cdr`, `vector-pattern-2026.cmx` | Full-color (vector) pattern fills |
| `fills-x5.cdr` | Ten rectangles with different fills, CorelDRAW X5 |

## 3. Import messages

### TC-01 Opening a drawing with a missing font shows a message
- **Data:** `5EFFECTS.CDR`
- **Steps:**
  1. **File > Open** (**Ctrl+O**), go to the test folder, pick `5EFFECTS.CDR`, press **Open**.
- **Expected result:**
  - The drawing opens in a new tab.
  - Then a message box titled **Open** appears, reading:
    > 5EFFECTS.CDR came in with these differences from the original:
    > • The font "AvantGarde Bk BT" isn't installed, so its text is shown in "Century Gothic"
    > • The font "AvantGarde Md BT" isn't installed, so its text is shown in "Century Gothic"
  - **OK** closes the message. The drawing stays open and usable.
- **Result:** ____ **Notes:** ____

### TC-02 A drawing that comes in exactly shows no message
- **Data:** `texture-cdr12.cdr`
- **Steps:**
  1. **File > Open**, pick `texture-cdr12.cdr`.
- **Expected result:** The drawing opens and no message box appears.
- **Result:** ____ **Notes:** ____

### TC-03 Importing shows the message titled Import
- **Data:** `LIZARD.CDR`
- **Steps:**
  1. **File > New** (**Ctrl+N**).
  2. **File > Import** (**Ctrl+I**), pick `LIZARD.CDR`.
  3. Click the page to place it if AnyVector asks you to.
- **Expected result:**
  - The lizard drawing is placed on the page.
  - A message box titled **Import** appears, reading:
    > LIZARD.CDR came in with these differences from the original:
    > • The font "Bangkok" isn't installed, so its text is shown in "Arial"
- **Result:** ____ **Notes:** ____

### TC-04 Templates list their font once, however many texts use it
- **Data:** `NEWS_BOX.CDT`
- **Steps:**
  1. **File > Open**. Choose the file type **CDT - CorelDRAW Template** if the template doesn't show, then pick `NEWS_BOX.CDT`.
- **Expected result:**
  - It opens with 2 pages.
  - The message has exactly one line: "The font "ChelthmITC Bk BT" isn't installed, so its text is shown in "Arial"".
- **Result:** ____ **Notes:** ____

### TC-05 The message in Brazilian Portuguese
- **Data:** `COLORS.CDR`
- **Steps:**
  1. **Tools > Options > Global > General**, set **User interface language** to **Português Brasileiro**, press **OK**. Restart AnyVector if it asks you to.
  2. **Arquivo > Abrir**, pick `COLORS.CDR`.
- **Expected result:** The message reads:
  > COLORS.CDR foi aberto com estas diferenças em relação ao original:
  > • A fonte "AvantGarde Bk BT" não está instalada, então o texto dela aparece em "Century Gothic"
  > • A fonte "AvantGarde Md BT" não está instalada, então o texto dela aparece em "Century Gothic"
- **Cleanup:** Set the language back to **English** (the same place) and restart if asked.
- **Result:** ____ **Notes:** ____

### TC-06 A damaged file gives an error, not a crash
- **Data:** `PRES.CDR`
- **Steps:**
  1. **File > Open**, pick `PRES.CDR`.
- **Expected result:**
  - An error says the drawing is empty or damaged.
  - AnyVector keeps running and the other open tabs are untouched.
- **Result:** ____ **Notes:** ____

## 4. Font substitution

### TC-07 Arial Narrow is really used
- **Data:** `KING.CDR`
- **Steps:**
  1. Open `KING.CDR`.
  2. With the **Pick** tool click the text "ROADMASTER 1949".
  3. Read the font name in the property bar and the object width (**W**) in the property bar.
- **Expected result:**
  - No message box.
  - The font is **Arial Narrow**, and the letters are visibly narrow.
  - **W** is about **29.0 mm**, give or take 0.5 mm.
- **Result:** ____ **Notes:** ____

### TC-08 A substituted font looks like the original
- **Data:** `5EFFECTS.CDR`
- **Steps:**
  1. Open `5EFFECTS.CDR` (close the message).
  2. Zoom into a text and look at the letters.
  3. Select the text and read the font name in the property bar.
- **Expected result:**
  - The letters are Century Gothic: a round geometric "o" and a single-storey "a".
  - Write down which name the property bar shows: the original (AvantGarde) or Century Gothic.
- **Result:** ____ **Notes (font name shown):** ____

## 5. Text

### TC-09 Empty paragraph frames are kept
- **Data:** `JOB.CDT`
- **Steps:**
  1. Open `JOB.CDT`.
  2. Open **Window > Dockers > Objects** and expand the page and its layers.
  3. Count the **Paragraph Text** objects.
  4. Select the empty frame from the Objects docker.
  5. Press **F8** for the **Text** tool, click inside that frame, and type `test`.
- **Expected result:**
  - There are **3** paragraph texts; before this round there were 2.
  - In step 4 the selection handles show a frame where nothing is printed.
  - In step 5, "test" appears inside that frame, wrapped to its width.
- **Result:** ____ **Notes:** ____

### TC-10 More empty frames, across pages
- **Data:** `NEWS_BOX.CDT`
- **Steps:**
  1. Open `NEWS_BOX.CDT`.
  2. Count the **Paragraph Text** objects on both pages in the Objects docker.
- **Expected result:**
  - There are **27** paragraph texts across the 2 pages; before this round there were 18.
  - The page layout looks like a newsletter box template, with no frames piled up in a corner.
- **Result:** ____ **Notes:** ____

### TC-11 CorelDRAW 3 text uses the drawing's font
- **Data:** `COLORBAR.CDR`
- **Steps:**
  1. Look at the file's preview in the **File > Open** dialog.
  2. Open `COLORBAR.CDR`.
- **Expected result:**
  - No message box.
  - All 10 texts show, in place, in one consistent font, and line up with the preview's layout.
  - CorelDRAW 2026 can't open CorelDRAW 3 files, so the preview is the reference.
- **Result:** ____ **Notes:** ____

## 6. Texture fills

### TC-12 A CorelDRAW 12 texture comes in as a texture
- **Data:** `square_texturefill_nooutline.cdr`; reference `reference\square_texturefill_nooutline.png`
- **Steps:**
  1. Open the drawing.
  2. Open the reference PNG next to it (in any image viewer) and compare.
- **Expected result:**
  - The square has a texture, not flat gray, with the reference's character and colors.
  - The texture's scale is close to the reference: the same number of "features" across the square, give or take.
- **Result:** ____ **Notes:** ____

### TC-13 The Properties docker shows the texture's settings
- **Data:** the same drawing, still open
- **Steps:**
  1. Click the square with the **Pick** tool.
  2. In **Properties**, open the **Fill** section.
- **Expected result:** The docker shows:
  - the label **Texture fill: <style name>**
  - the fields **Texture #:**, **Setting 2:** and so on, **Color 1:** and so on (color buttons), and **Brightness:**
  - a **Convert to bitmap pattern** button
  - no "isn't in CorelDRAW's texture library" line
- **Result:** ____ **Notes (style name):** ____

### TC-14 Editing the brightness redraws the texture, and Undo restores it
- **Steps:**
  1. With the square selected, set **Brightness:** to `40` and press **Enter**.
  2. **Edit > Undo** (**Ctrl+Z**).
- **Expected result:**
  - After step 1 the texture redraws lighter. Only this square changes, within about a second.
  - After step 2 it is back to the original look and Brightness reads its old value.
- **Result:** ____ **Notes:** ____

### TC-15 Editing a color and the texture number
- **Steps:**
  1. Click **Color 1:** and pick a bright red.
  2. Change **Texture #:** to a different number and press **Enter**.
  3. Press **Ctrl+Z** twice.
- **Expected result:**
  - Step 1: the texture redraws with red in it.
  - Step 2: the texture redraws with a different random pattern.
  - Step 3: it is back to the original.
- **Result:** ____ **Notes:** ____

### TC-16 Convert to bitmap pattern
- **Steps:**
  1. Press **Convert to bitmap pattern**.
  2. Look at the square and the Properties docker.
  3. **Ctrl+Z**.
- **Expected result:**
  - Step 2: the square looks the same, but the docker shows a bitmap pattern fill. The texture fields and the **Convert to bitmap pattern** button are gone.
  - Step 3: the texture fields are back.
- **Result:** ____ **Notes:** ____

### TC-17 The texture's settings survive saving
- **Steps:**
  1. Change **Brightness:** to `25`.
  2. **File > Save As**. Choose the type **VCT - AnyVector Drawing**, the folder `saved`, and the name `texture-test.vct`.
  3. Close the drawing, then open `saved\texture-test.vct`.
  4. Select the square.
  5. Repeat steps 2 to 4 with the type **CDR - CorelDRAW** and the name `texture-test.cdr`.
- **Expected result:** After each reopen:
  - the square looks as it did before saving
  - the docker still shows **Texture fill: <style>** with Brightness **25**
  - the fields can still be edited
- **Result:** ____ **Notes:** ____

### TC-18 A second texture file
- **Data:** `texture-cdr12.cdr`
- **Steps:**
  1. Open the file.
  2. Open the same file in CorelDRAW 2026 and compare.
  3. Select the object and check the docker.
- **Expected result:**
  - A texture similar to CorelDRAW's.
  - The docker shows the texture fields as in TC-13.
- **Result:** ____ **Notes:** ____

## 7. PostScript fills

### TC-19 A PostScript fill is drawn
- **Data:** `square_postscriptfill_nooutline.cdr`; reference `reference\square_postscriptfill_nooutline.png`
- **Steps:**
  1. Open the drawing and compare with the reference PNG.
- **Expected result:**
  - The square shows the same PostScript pattern as the reference, with similar line weight and spacing, not a flat tone.
  - If the pattern is random by nature (scattered shapes), it may differ in placement and color from CorelDRAW's.
- **Result:** ____ **Notes:** ____

### TC-20 Archimedes spiral
- **Data:** `postscript-cdr12.cdr`
- **Steps:**
  1. Open the file in AnyVector and in CorelDRAW 2026, and compare.
- **Expected result:** A spiral sized to the object, centered as in CorelDRAW.
- **Result:** ____ **Notes:** ____

### TC-21 The Properties docker shows PostScript fills read-only
- **Steps:**
  1. Select the square from TC-19 and look at **Properties > Fill**.
- **Expected result:** The docker shows:
  - **PostScript fill <number>**
  - **Settings:** followed by a comma-separated list, or "none"
  - the note "Its settings are shown only: they can't be changed here yet."
  - no editable fields
  - a **Convert to bitmap pattern** button
- **Result:** ____ **Notes:** ____

### TC-22 PostScript fill survives saving
- **Steps:**
  1. Save as `saved\postscript-test.vct`, close, and reopen it.
  2. Select the square.
- **Expected result:** It looks as before, and the docker still shows **PostScript fill <same number>** with the same settings.
- **Result:** ____ **Notes:** ____

## 8. Vector (full-color) patterns

### TC-23 X5 vector pattern comes in with its drawing
- **Data:** `vector-pattern-x5.cdr`
- **Steps:**
  1. Open the file in AnyVector and in CorelDRAW 2026, and compare.
- **Expected result:**
  - A colored, repeating pattern: not gray, not blank.
  - Its tile size and offset are as in CorelDRAW.
- **Result:** ____ **Notes:** ____

### TC-24 CorelDRAW 2026 vector pattern, .cdr and .cmx
- **Data:** `vector-pattern-2026.cdr`, then `vector-pattern-2026.cmx`
- **Steps:**
  1. Open `vector-pattern-2026.cdr` and compare with CorelDRAW 2026.
  2. **File > Open**, set the type list to **CMX - Corel Presentation Exchange (\*.cmx)**, pick `vector-pattern-2026.cmx`.
- **Expected result:**
  - Both show the same colored pattern as CorelDRAW.
  - The CMX type is in the list.
- **Result:** ____ **Notes:** ____

### TC-25 The Properties docker names the pattern's objects
- **Data:** `vector-pattern-x5.cdr`
- **Steps:**
  1. Select the rectangle and look at **Properties > Fill**.
- **Expected result:** The docker shows:
  - **Vector pattern "<name>": <n> object(s)**, with n of 1 or more
  - an enabled **Edit fill** button
  - **Convert to bitmap pattern**
- **Result:** ____ **Notes (name, n):** ____

### TC-26 Editing the pattern tile
- **Steps:**
  1. With the rectangle selected, press **Edit fill** (the pencil icon next to the pattern).
  2. A new tab opens with the tile's objects on a page the size of one tile. Select one object and click a red swatch in the color palette.
  3. Close that tab (the **×** on the tab).
  4. Look at the original drawing's tab.
  5. **Edit > Undo**.
- **Expected result:**
  - Step 2: the tab shows the same objects as one repeat of the pattern.
  - Step 3: the tab closes without asking to save.
  - Step 4: the rectangle's pattern now has that object in red, in every repeat.
  - Step 5: the menu item reads **Undo Edit Pattern**, and the pattern goes back to its original colors.
- **Result:** ____ **Notes:** ____

### TC-27 A shared tile updates every object using it
- **Steps:**
  1. Select the rectangle and **Edit > Duplicate** (**Ctrl+D**). Move the copy aside.
  2. Select the copy, press **Edit fill**, change one object's color, and close the pattern tab.
- **Expected result:** Both rectangles show the change.
- **Result:** ____ **Notes:** ____

### TC-28 Vector pattern survives saving
- **Steps:**
  1. Save as `saved\vpattern-test.vct`. Close, reopen, select the rectangle.
  2. Repeat with type **CDR** as `saved\vpattern-test.cdr`.
- **Expected result:**
  - The pattern looks as before saving.
  - The docker still shows **Vector pattern "<name>": <n> object(s)**.
  - **Edit fill** still opens the objects.
- **Result:** ____ **Notes:** ____

### TC-29 Object > Create > Pattern Fill keeps its objects
- **Steps:**
  1. **File > New**. Draw a small rectangle and a small ellipse with different fill colors.
  2. **Object > Create > Pattern Fill...**. In **Create Pattern** choose **Type: Vector**, **Resolution: Medium**, and press **OK**.
  3. Drag a marquee around both shapes.
  4. Draw a large rectangle. In **Properties > Fill**, pick **Vector pattern fill** and choose the new pattern from the list.
  5. Look at the docker. Press **Edit fill**.
- **Expected result:**
  - Step 3: the status bar says "Pattern <n> was added to the pattern fills".
  - Step 4: the large rectangle is tiled with the two shapes.
  - Step 5: the docker reads **Vector pattern "Pattern <n>": 2 object(s)**, and Edit fill opens a tab with the two shapes.
- **Result:** ____ **Notes:** ____

### TC-30 CorelDRAW 12 two-color/bitmap pattern still works
- **Data:** `square_patternfill_nooutline.cdr`; reference `reference\square_patternfill_nooutline.png`
- **Steps:**
  1. Open the file and compare with the reference.
- **Expected result:** The same pattern as the reference. This case checks for regressions.
- **Result:** ____ **Notes:** ____

## 9. CMX clipart

### TC-31 Clipart saved as CMX under a .cdr name opens
- **Data:** `ARROW.CDR`
- **Steps:**
  1. **File > Open**, pick `ARROW.CDR`.
  2. Open the same file in CorelDRAW 2026 and compare.
  3. In the Objects docker, look at what the page holds.
- **Expected result:**
  - It opens with no error. Before this round it failed with "the drawing has no pages".
  - The arrows look like CorelDRAW's: shapes, fills and outlines.
  - The page holds 1 group of 18 curves.
- **Result:** ____ **Notes:** ____

### TC-32 CMX in the Import list
- **Steps:**
  1. **File > New**, then **File > Import**. Open the file type list.
  2. Pick **CMX - Corel Presentation Exchange (\*.cmx)** and import `vector-pattern-2026.cmx`.
- **Expected result:** The type is listed, and the rectangle with its pattern is placed on the page.
- **Result:** ____ **Notes:** ____

## 10. Regression

### TC-33 Ten fills from CorelDRAW X5
- **Data:** `fills-x5.cdr`
- **Steps:**
  1. Open the file in AnyVector and in CorelDRAW 2026, and compare each of the 10 rectangles.
- **Expected result:**
  - Each rectangle has the same kind of fill as in CorelDRAW: solid, gradient, pattern, texture, PostScript and so on.
  - None is gray or blank where CorelDRAW shows color.
- **Result:** ____ **Notes:** ____

### TC-34 Basic drawing, save and export
- **Steps:**
  1. **File > New**. Draw a rectangle, fill it blue, and type an artistic text.
  2. Save as `saved\smoke.cdr`, close, and reopen it.
  3. **File > Export** as PNG to `saved\smoke.png`.
- **Expected result:**
  - It reopens identical.
  - The PNG shows the rectangle and text.
  - No error messages at any point.
- **Result:** ____ **Notes:** ____

### TC-35 Many drawings open at once
- **Steps:**
  1. Open `5EFFECTS.CDR`, `KING.CDR`, `ARROW.CDR` and `vector-pattern-x5.cdr` in one **File > Open** (Ctrl+click to pick several).
- **Expected result:**
  - Four tabs open.
  - Only 5EFFECTS shows a message, and the message names the right file.
  - Switching tabs works.
- **Result:** ____ **Notes:** ____

## 11. Not covered by this plan

These were built but can't be tried here, for lack of a file. They are listed in `docs/STATUS.md` under known
issues.

- **Unit tests only:**
  - Texture fills from CorelDRAW 7 to 11 files.
  - Vector patterns from CorelDRAW 6 to X4 files. CorelDRAW 2026 can't save older than X5.
  - The messages for a skipped object type, an unknown texture style and a font number the drawing doesn't list.
- **Known not to work yet:** vector patterns in CorelDRAW 5 files stay gray.
- **No sample here:** the "ON THE RIGHT TRACK!" drawing (Bedrock font).

## 12. Summary

| Area | Cases | Pass | Fail | Blocked |
|---|---|---|---|---|
| Import messages | TC-01 to TC-06 | | | |
| Font substitution | TC-07, TC-08 | | | |
| Text | TC-09 to TC-11 | | | |
| Texture fills | TC-12 to TC-18 | | | |
| PostScript fills | TC-19 to TC-22 | | | |
| Vector patterns | TC-23 to TC-30 | | | |
| CMX clipart | TC-31, TC-32 | | | |
| Regression | TC-33 to TC-35 | | | |

Ready to release when no case fails. A failure in sections 3 to 9 holds the release; a failure in section 10 always
does.
