"""Render drawings with CorelDRAW itself, as the reference for AnyVector's import.

Opens each drawing in the installed CorelDRAW 2026 through its automation (COM), exports page 1 as a
PNG and closes it without saving. Use the PNGs to compare with AnyVector's own render of the same
file (`ANYVECTOR_CDR_PNG=1` with the `corel_batch` probe in vector-io).

    python -I xtask/corel_render.py [--dpi 96] [--out <folder>] <file or folder> ...

The PNG of `name.cdr` is `name.corel.png`, next to the drawing unless `--out` is given. Work on copies:
never point it at a folder you don't want PNGs written into.

What makes it work (CorelDRAW refuses to open files otherwise):
- CorelDRAW must be idle: no trial reminder or other dialog up. Start it, close the reminder, and
  let it settle before running this.
- A document must be open: with none, CorelDRAW sits on its Welcome screen and OpenDocument fails
  with "Failed to open document". The script creates a blank one when there is none.
- pywin32 doesn't pass "Nothing" for optional object arguments, so Export gets both option structs.

Requires pywin32 (`python -m pip install pywin32`). Only one script should drive CorelDRAW at a time.
"""
import argparse
import sys
from pathlib import Path

CDR_PNG = 802  # cdrPNG export filter
EXPORT_PAGE = 1  # cdrCurrentPage
RGB = 4  # cdrRGBColorImage
ANTIALIAS = 2  # cdrSupersampling


def corel():
    from win32com.client import gencache
    app = gencache.EnsureDispatch("CorelDRAW.Application.27")
    if app.Documents.Count == 0:
        app.CreateDocument()
    return app


def render(app, path: Path, out: Path, dpi: int) -> None:
    doc = app.OpenDocument(str(path))
    try:
        opts = app.CreateStructExportOptions()
        opts.ResolutionX = dpi
        opts.ResolutionY = dpi
        opts.AntiAliasingType = ANTIALIAS
        opts.ImageType = RGB
        filt = doc.Export(str(out), CDR_PNG, EXPORT_PAGE, opts, app.CreateStructPaletteOptions())
        if filt is not None and hasattr(filt, "Finish"):
            filt.Finish()
    finally:
        doc.Dirty = False
        doc.Close()


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("paths", nargs="+", type=Path)
    ap.add_argument("--dpi", type=int, default=96)
    ap.add_argument("--out", type=Path)
    a = ap.parse_args()
    files = []
    for p in a.paths:
        files += sorted(q for q in p.iterdir() if q.suffix.lower() in (".cdr", ".cmx", ".cdt")) if p.is_dir() else [p]
    app = corel()
    failed = 0
    for f in files:
        out = (a.out or f.parent) / (f.stem + ".corel.png")
        out.parent.mkdir(parents=True, exist_ok=True)
        try:
            render(app, f.resolve(), out.resolve(), a.dpi)
            print(f"ok   {f.name} -> {out}")
        except Exception as e:  # one bad file must not stop the batch
            failed += 1
            print(f"FAIL {f.name}: {e}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
