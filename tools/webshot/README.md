# webshot — our editor, headless

    node shot.js web/dist/ezxdw-editor.html FILE.xdw OUT.png [page]
    node edit-test.js web/dist/ezxdw-editor.html FILE.xdw OUTDIR    # scripted editing session + saves
    ./compare.sh FILE.xdw OUTDIR                                     # viewer vs editor, page 1

Needs Node with Playwright (Chromium).
