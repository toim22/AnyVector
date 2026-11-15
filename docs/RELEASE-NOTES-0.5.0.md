# AnyVector 0.5.0

AnyVector 0.5.0 opens PDF, EPS and PostScript files, manages color with ICC profiles, cuts on sign cutters, pastes
cleanly into Office, and works with more AI providers, including ones that run on your own computer. It also fixes
the bugs reported after 0.4.1.

## Open more kinds of files

- **PDF files.** File ▸ Open and File ▸ Import read PDF files. The Import PDF dialog asks whether text comes in as
  text or as curves, and which pages to bring in. Paths, clipping (as PowerClips), pictures, gradients, transparency
  and spot colors come through. Text stays editable when its font is installed; otherwise it comes in as curves in
  the font the PDF carries. Illustrator files that are PDF inside open the same way.
- **EPS, PS and older Illustrator files.** AnyVector reads PostScript itself, so you don't need Ghostscript. For an EPS
  file it can't read, it uses the preview picture inside, and the import messages list what was left out.
- **JPEG XL.** .jxl pictures can be imported and exported. Export is lossless only.
- **Fonts inside CorelDRAW files.** Fonts saved inside a CorelDRAW X7 or later drawing are used while the drawing is
  open, without installing them, as long as the font allows it.

## Color

- **Color management with ICC profiles.** Colors on screen, in the color dockers, in soft proofing and in PDF files go
  through the profiles you choose. CMYK colors keep their exact values when you import, edit and publish to PDF, and
  PDF/X files carry the document's profile.
- **Tools ▸ Color Management.** A real dialog with CorelDRAW's presets, including Simulate Color Management Off, for
  colors that look the same as with color management turned off.

## Sign making and cutters

- **HPGL and DXF export.** Export as .plt for cutters and plotters, with a pen for each outline color, or as .dxf for
  CAD programs.
- **Print and cut.** Object ▸ Create Cut Contour draws a hairline around the selection in the CutContour spot color,
  which prints as its own separation in PDF files.
- **Cutter plugins.** A script can add an export for a particular cutter.

## Office

- **Paste into Office as vectors.** Copy puts the drawing on the clipboard as SVG, EMF and a picture, so Office apps
  such as Word and PowerPoint paste clean shapes instead of a blurry bitmap.
- **EMF export.** File ▸ Export saves EMF files, and Export for Office ▸ Editing works.

## AI Generate

- **More AI providers.** Besides Anthropic, AI Generate works with OpenAI-compatible services and with Ollama and
  LM Studio, which run on your own computer and need no API key. ComfyUI can make the pictures for Create image.
  Choose the provider in Tools ▸ Options ▸ Global ▸ Privacy.

## Finding things

- **Search in Options.** Type in the search box of Tools ▸ Options to find a setting on any page; clicking a result
  opens its page and highlights it. The Object Styles docker and the Glyphs docker also have a search box.

## Fixed

- **Freehand lines join up.** Freehand, Polyline, Bézier, Pen and 2-Point Line continue the selected curve when you
  start near one of its ends, and close it when you end on the other end, as in CorelDRAW.
- **The first document fills the window.** The document AnyVector starts with is fitted and centered once the window
  has its real size, and a new document remembers the paper size you used last.
- **Fonts shown as squares.** Installed fonts that were reported as missing, so their text showed as squares, are
  found by every name Windows gives them, in every language.
- **CorelDRAW 2022 files.** The import no longer reports objects of type 31 as left out; they are CorelDRAW's own
  notes for its import filters and have nothing to draw.
- **Ungrouping big drawings.** Ungrouping thousands of objects no longer makes the window stop painting for seconds.
