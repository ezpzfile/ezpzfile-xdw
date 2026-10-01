# webshot — our editor, headless

    node shot.js web/dist/ezxdw-editor.html FILE.xdw OUT.png [page]
    node edit-test.js web/dist/ezxdw-editor.html FILE.xdw OUTDIR    # scripted editing session + saves
    ./compare.sh FILE.xdw OUTDIR                                     # viewer vs editor, page 1
    node feature-test.js web/dist/ezxdw-editor.html FILE.xdw OUTDIR PICTURE.jpg OTHER.xdw [DOC.pdf]
                                    # sticky note, date stamp, inserted pages (blank, picture, .xdw, PDF)
    node feature-test.js web/dist/ezxdw-editor.html FILE.xbd OUTDIR - OTHER.xdw
                                    # binder: add, rename, reorder documents

Needs Node with Playwright (Chromium).
