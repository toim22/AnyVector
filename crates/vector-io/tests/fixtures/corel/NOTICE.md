# Third-party fixtures

`texture-cdr12.cdr` is `square_texturefill_nooutline.cdr` from the libcdr project's regression corpus
(https://git.libreoffice.org/libcdr-test), copied unchanged and renamed. It is licensed under Creative Commons
Attribution-ShareAlike 4.0 International (CC-BY-SA 4.0, http://creativecommons.org/licenses/by-sa/4.0/) and is used
here only as test data for the CorelDRAW importer.

`postscript-cdr12.cdr` is `square_postscriptfill_nooutline.cdr` from the same corpus, copied unchanged and renamed,
under the same licence and for the same use.

The other fixtures in this folder were drawn for AnyVector in CorelDRAW 2026 (see `tests/corel.rs`).

`vector-pattern-x5.cdr`, `vector-pattern-2026.cdr` and `vector-pattern-2026.cmx` hold a full-color pattern fill whose
pattern was drawn for AnyVector from simple shapes (rectangles, ellipses, a curve and a line) in CorelDRAW 2026 and
saved by CorelDRAW. They contain nothing from CorelDRAW's own pattern library or clipart.
