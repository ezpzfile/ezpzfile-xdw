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
- Saving **writes the whole file anew with only what the document uses now** (since 2026-10-07):
  deleted pages and annotations do not stay in the file. Only a signed document is saved the way
  DocuWorks saves it, one bundle (segment) appended at the end, which keeps the signed state. Either
  way, parts we do not understand are copied byte for byte.
- Sizes and counts written in a file are not trusted: they are checked against the data that is
  really there, and reading stops at the limits (v0.3).
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
| Picture annotation (画像を貼る) | Done. DocuWorks draws the white areas of its picture annotation as opaque, so they hide the text underneath (see-through pictures: v0.3) |
| Insert page: blank | Done |
| Insert page: photo (JPG, PNG and others) | Done. Stored the way DocuWorks stores scanned pages (JPEG + DWb/DWc) |
| Insert page: pages from another .xdw/.xbd | Done. Data the page uses (drawings, photos, thumbnails) is copied with it |
| Insert page: PDF | Done. PDF pages go in as pictures (pdf.js is downloaded once, the first time) |
| Editing binders (.xbd) | Done. The page list groups pages by document; add, rename, reorder and remove documents |
| Checked in full DocuWorks 10 | Done. With the latest version, the DocuWorks API opens and draws all 43 saved files, and Desk lists them |
| DocuWorks 10 scan and photo pages (drawing kind 5) | Done (new finding) |
| Signed documents | On opening, a notice says editing will invalidate the signature; the signature itself cannot be moved or deleted |

## v0.3 (2026-10-02 to 2026-11-04): selecting and searching text, see-through pictures, saving anew, untrusted files

| Item | Status |
|---|---|
| Selecting and copying text in pages | Done. An invisible text layer lies over the drawn text. Ctrl+A selects all text on the page |
| Searching the document (Ctrl+F) | Done. Ignores full-width / half-width and case differences, and selects what it finds |
| See-through pictures | Done. The white parts show the text underneath. DocuWorks has no see-through picture annotation, so the picture is kept the way a metafile pasted from the clipboard is kept (an OLE Enhanced Metafile) whose drawing multiplies the picture with the page. It looks the same in DocuWorks 10, which can also move and resize it (spec 5.5) |
| Opaque pictures | DocuWorks' picture annotation, as before. The properties panel switches a picture between the two. A picture with transparent parts (a PNG, say) goes in see-through |
| Resizing pictures | Done (drag a corner) |
| Recognizing protected documents | Done. A document protected by a password or a certificate is reported as protected rather than damaged, with a note to remove the protection in DocuWorks first. This editor never handles passwords |
| Show signature information | Done. Selecting a signature shows its type (electronic seal or certificate), the signing module (`%smin`) and version (`%sigver`). Whether it still holds needs the DocuWorks API or verifying the certificate (not yet) |
| Saving the whole file anew (10/07) | Done. Only the entries in use are written, numbered anew, in a one-segment file; deleted pages and annotations do not stay. A signed document is appended to instead. 39 public samples rewritten and edited open in Viewer Light |
| New documents (10/07) | Done. One blank A4 page, written the way DocuWorks 10 writes a new document. Dropping PDFs or pictures with nothing open makes a new document of them |
| Text of pages taken from a PDF (10/28) | Done. In the engine (core), `Document::insert_image_page_text` draws the text first in the page drawing and the page picture over it: the page looks the same, and the text is there for search, selection, copying and the PDF text layer (in DocuWorks too). Taking the text and its places out of the PDF is the web editor's part (pdf.js), passed in through wasm `insertImagePageText` |
| Limits for untrusted files (11/04) | Done. Expanded sizes from the file are no longer reserved up front: they are checked against `maxExpand` and `maxRatio` and memory grows with what is really decoded. Limits too for pixels per picture, drawing records and points per page, pages, and working memory (`Limits`, `Document::open_with`). Counts inside a record are cut to what the record holds. Past a limit: `TooLarge` ("too large or damaged"). The browser engine uses at most 1 GiB, and the editor checks a file in a Web Worker before opening it (`web/worker/ezpzxdw-safe.js`) |
| Leaving out covered text (11/04) | Done. Text completely covered by an opaque fill or picture drawn after it is left out of the text, the PDF text layer and the editor's text selection (by default). Highlighters, partly transparent pictures and pictures in the same drawing (scans, pages taken from a PDF) are not covers. `include_hidden` / `setIncludeHidden(true)` / `--include-hidden` keep it; `hidden_chars` says how much was left out |

## Next

1. **Verifying signatures**: check the certificate and show whether a signature still holds. Adding new signatures comes later.
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
