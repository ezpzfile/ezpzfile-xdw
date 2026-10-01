# EZPZ File XDW

**Open, annotate and save DocuWorks (`.xdw`) documents anywhere — Mac, Linux, phone, browser.**
An open-source engine, editor and format specification for FUJIFILM (Fuji Xerox)
DocuWorks files, built from scratch.

> DocuWorks の `.xdw` ファイルを、Mac・Linux・スマホ・ブラウザで開いて、注釈を書き込み、
> そのまま `.xdw` で保存できるオープンソースです。ファイル形式の解析結果（仕様書）も公開しています。
> ファイルは端末の外に送信されません。

Status: **v0.2 — viewer + editor that saves back to `.xdw` / `.xbd` (developer preview).**
Not affiliated with FUJIFILM Business Innovation.

## What works

Tested on 43 public DocuWorks files (generation 7 and 10, `.xdw` and `.xbd`):

- all 43 open, every page draws with no unknown drawing records
- pages: EMF and WMF drawings, the DocuWorks driver's own compact path and
  picture records, JPEG picture strips, scanned/rotated pages, annotations
- **saving back to `.xdw`**, the way DocuWorks itself saves (one segment appended,
  nothing earlier rewritten) — saved files open in **DocuWorks Viewer Light**
- annotations: add **text, sticky note (付箋), date stamp (日付印), highlighter,
  rectangle, ellipse, line, picture**; move, resize, change, delete (existing
  annotations from DocuWorks too: move / delete)
- pages: turn left / right, delete, reorder (drag in the page list);
  **insert** a blank page, pictures (JPEG / PNG / …), pages of another `.xdw` /
  `.xbd`, or PDF pages (as pictures; needs the network once for pdf.js)
- **binders (`.xbd`)**: documents shown in the page list; add `.xdw` files,
  rename, reorder, remove documents
- export: **PDF** (looks like the screen, text searchable/copyable), plain text; print
- undo / redo

The format findings, including the check value DocuWorks verifies and the
driver's private drawing records, are in [docs/spec/XDW-FORMAT.md](docs/spec/XDW-FORMAT.md).

Not yet: signatures / passwords (signed documents open; editing voids the
signature, and the editor says so), text selection in pages, see-through
picture annotations. See the [plan](docs/PLAN.ko.md).

## Editor

`web/dist/ezpzxdw-editor.html` is the whole editor in one file: double-click it,
drop a `.xdw` on the window, annotate, and save (Ctrl+S → `.xdw`). The screen
follows DocuWorks Viewer (page list on the left, annotation tools on top,
properties on the right) in the EZPZ File design. Keys: T text, S sticky note,
D date stamp, H highlighter, R rectangle, E ellipse, L line, Esc select,
Delete, arrows (Shift = 1 cm), Ctrl+Z / Ctrl+Y, Ctrl+S, Ctrl+Shift+S (save as
PDF / text), Ctrl+P. Drop a PDF or a picture on an open document to insert it
as pages.

## Checked against the real DocuWorks

`tools/dwview` runs FUJIFILM's free DocuWorks Viewer Light under Wine and reports
whether a file opens, with a screenshot. `tools/dwapi` calls the API of
DocuWorks 10 itself (installed from the trial) so that DocuWorks reads our saved
files — pages, annotations and their settings — and draws their pages. Every
saved test file matches and draws (38 public samples + 5 DocuWorks 10 samples,
all editing features); DocuWorks Desk lists them with their pages. See
[experiments/results.md](experiments/results.md).

## Layout

```
engine/crates/ezpzxdw-core   reader, renderer (EMF/WMF/DW → display list), editor, writer, PDF
engine/crates/ezpzxdw-cli    `ezpzxdw` command: info, tree, pages, render, text, edit, check …
engine/crates/ezpzxdw-wasm   browser bindings
web/                       the editor (build: web/build.sh → web/dist/ezpzxdw-editor.html)
docs/spec/XDW-FORMAT.md    the format
tools/dwview               DocuWorks Viewer Light as a referee (Wine)
tools/webshot              the editor driven headless (Playwright)
corpus/manifest.tsv        where the public sample files come from (files not included)
```

## Build

```
cd engine && cargo test                       # + EZPZXDW_CORPUS=/path/to/samples for the corpus tests
cargo run -p ezpzxdw-cli -- pages file.xdw
web/build.sh                                  # needs wasm32 target and wasm-bindgen-cli 0.2.129
```

## Licence

MIT OR Apache-2.0. See [NOTICE](NOTICE) for trademarks and third-party assets.
