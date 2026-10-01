# EZPZ File XDW

**Open, annotate and save DocuWorks (`.xdw`) documents anywhere — Mac, Linux, phone, browser.**
An open-source engine, editor and format specification for FUJIFILM (Fuji Xerox)
DocuWorks files, built from scratch.

> DocuWorks の `.xdw` ファイルを、Mac・Linux・スマホ・ブラウザで開いて、注釈を書き込み、
> そのまま `.xdw` で保存できるオープンソースです。ファイル形式の解析結果（仕様書）も公開しています。
> ファイルは端末の外に送信されません。

Status: **v0.1 — viewer + annotation editor that saves back to `.xdw` (developer preview).**
Not affiliated with FUJIFILM Business Innovation.

## What works

Tested on 43 public DocuWorks files (generation 7 and 10, `.xdw` and `.xbd`):

- all 43 open, every page draws with no unknown drawing records
- pages: EMF and WMF drawings, the DocuWorks driver's own compact path and
  picture records, JPEG picture strips, scanned/rotated pages, annotations
- **saving back to `.xdw`**, the way DocuWorks itself saves (one segment appended,
  nothing earlier rewritten) — saved files open in **DocuWorks Viewer Light**
- annotations: add **text, highlighter, rectangle, ellipse, line**; move, resize,
  change, delete (existing annotations from DocuWorks too: move / delete)
- pages: turn left / right, delete, reorder (drag in the page list)
- export: **PDF** (looks like the screen, text searchable/copyable), plain text; print
- undo / redo

The format findings, including the check value DocuWorks verifies and the
driver's private drawing records, are in [docs/spec/XDW-FORMAT.md](docs/spec/XDW-FORMAT.md).

Not yet: stamps (日付印) and sticky notes (付箋), inserting pages from other
files, binder editing, signatures/passwords. See the [plan](docs/PLAN.ko.md).

## Editor

`web/dist/ezxdw-editor.html` is the whole editor in one file: double-click it,
drop a `.xdw` on the window, annotate, and save (Ctrl+S → `.xdw`). The screen
follows DocuWorks Viewer (page list on the left, annotation tools on top,
properties on the right) in the EZPZ File design. Keys: T text, H highlighter,
R rectangle, E ellipse, L line, Esc select, Delete, arrows (Shift = 1 cm),
Ctrl+Z / Ctrl+Y, Ctrl+S, Ctrl+Shift+S (save as PDF / text), Ctrl+P.

## Checked against the real viewer

`tools/dwview` runs FUJIFILM's free DocuWorks Viewer Light under Wine and reports
whether a file opens, with a screenshot. It was used to find the format rules
(each marked [viewer] in the spec) and to check saved files:
see [experiments/results.md](experiments/results.md). Full DocuWorks (Desk) has not
been tested yet.

## Layout

```
engine/crates/ezxdw-core   reader, renderer (EMF/WMF/DW → display list), editor, writer, PDF
engine/crates/ezxdw-cli    `ezxdw` command: info, tree, pages, render, text, edit, check …
engine/crates/ezxdw-wasm   browser bindings
web/                       the editor (build: web/build.sh → web/dist/ezxdw-editor.html)
docs/spec/XDW-FORMAT.md    the format
tools/dwview               DocuWorks Viewer Light as a referee (Wine)
tools/webshot              the editor driven headless (Playwright)
corpus/manifest.tsv        where the public sample files come from (files not included)
```

## Build

```
cd engine && cargo test                       # + EZXDW_CORPUS=/path/to/samples for the corpus tests
cargo run -p ezxdw-cli -- pages file.xdw
web/build.sh                                  # needs wasm32 target and wasm-bindgen-cli 0.2.129
```

## Licence

MIT OR Apache-2.0. See [NOTICE](NOTICE) for trademarks and third-party assets.
