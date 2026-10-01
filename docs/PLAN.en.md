# EZPZ File XDW: plan and progress

[日本語](PLAN.ja.md) · **English** · [한국어](PLAN.ko.md)

An open-source project to open DocuWorks (.xdw) documents anywhere (Mac, Linux, phone, browser),
annotate them, and save them back as .xdw.
Same approach as for JTD: **a Rust engine + a one-file web editor + a public format spec, checked
against the real viewer**.

## Principles

- We build it from scratch. Other projects (xdw-salvage and others) are read only to check our
  answers.
- The real DocuWorks Viewer Light is the referee. A [viewer] mark in the spec means we made a file
  and opened it there.
- Saving works the way DocuWorks does it: the original stays untouched and **one bundle (segment)
  is appended at the end**. Parts we do not understand are left alone.
- A saved file is read back and compared. If anything differs, nothing is saved.
- No effect on ezpzfile.com (separate repository).

## v0.1 (2026-10-01)

| Item | Status |
|---|---|
| File structure (segments, trailer, entries) | Done, for both generation 7 and 10 |
| Check values (checksums) | Done. XOR over 4-byte words; the viewer checks the trailer value |
| LZH (-lh5-) decompression and compression | Done, our own implementation |
| Reading and writing properties (the object tree) | Done. All 43 corpus files are written back byte for byte |
| Drawing pages: EMF, WMF, DocuWorks' own records (DW…) | Done. No unknown records on any corpus page |
| Photos (JPEG strips), bitmaps, rotated pages | Done |
| Showing annotations | Done (draws the stored drawing) |
| Adding annotations: text, highlighter, rectangle, ellipse, line | Done, they open in the viewer |
| Moving, resizing, editing and deleting annotations | Done |
| Rotating, deleting and reordering pages | Done |
| Saving as .xdw, PDF (with a searchable text layer), text; printing | Done |
| Web editor (EZPZ design, Japanese screen) | Done, `web/dist/ezpzxdw-editor.html` |

## v0.2 (2026-10-01): 付箋, 日付印, inserting pages, binders

| Item | Status |
|---|---|
| 付箋 (sticky note) | Done. A real DocuWorks 付箋 (0x801a) with a text box inside, built the same way as DocuWorks 10 |
| 日付印 (date stamp) | Done. Top text, date and bottom text in a red circle, using the same settings DocuWorks 10 writes (`STAMPATT_…`) |
| Picture annotation (画像を貼る) | Done. Note: DocuWorks draws white areas as opaque, so they hide the text underneath |
| Insert page: blank | Done |
| Insert page: photo (JPG, PNG and others) | Done. Stored the way DocuWorks stores scanned pages (JPEG + DWb/DWc) |
| Insert page: pages from another .xdw/.xbd | Done. Data the page uses (drawings, photos, thumbnails) is copied with it |
| Insert page: PDF | Done. PDF pages go in as pictures (pdf.js is downloaded once, the first time) |
| Editing binders (.xbd) | Done. The page list groups pages by document; add, rename, reorder and remove documents |
| Checked in full DocuWorks 10 | Done. With the trial installed, the DocuWorks API opens and draws all 43 saved files, and Desk lists them |
| DocuWorks 10 scan and photo pages (drawing kind 5) | Done (new finding) |
| Signed documents | On opening, a notice says editing will invalidate the signature; the signature itself cannot be moved or deleted |

## Next

1. **Selecting and searching text in the document**, using the positions in the text records.
2. Extracting **attached original files (1306)**.
3. Creating new binders (for now, only existing binders can be edited).
4. The remaining annotation kinds such as markers and polygons (samples can be made with the
   DocuWorks API to match against).

## How it is checked

- `cargo test` with `EZPZXDW_CORPUS=…`: reads the whole corpus, draws every page, edits, saves and
  reads back.
- `tools/dwview`: opens saved files in the real Viewer Light.
- `tools/dwapi`: DocuWorks 10 itself (through its API) opens and draws the saved files, and the page
  and annotation counts are compared with our engine.
- `tools/webshot`: drives the editor like a person, all the way to saving, and puts the result next
  to the viewer's screen.
