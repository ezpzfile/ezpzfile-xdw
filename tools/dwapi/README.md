# dwapi: DocuWorks itself as a referee

`dwapi.c` is a small Windows program that calls the DocuWorks API
(`xdwapi.dll`, installed with DocuWorks Desk) under Wine. It lets the real
DocuWorks engine open our saved files, list pages and annotations with their
settings, and draw pages:

    i686-w64-mingw32-gcc -O1 -o dwapi.exe dwapi.c
    wine dwapi.exe info   'Z:\path\file.xdw'          # pages, annotations, settings (Shift_JIS)
    wine dwapi.exe render 'Z:\path\file.xdw' 1 'Z:\path\p1.bmp' 100
    wine dwapi.exe make   'Z:\path\copy.xdw'          # DocuWorks adds a date stamp, sticky note, text
    python3 check.py out_*.xdw                        # ours vs DocuWorks: pages, annotations, drawing

`make` is how the attribute names DocuWorks 10 writes for date stamps and
sticky notes were found (see `docs/spec/XDW-FORMAT.md`).

Setup used: DocuWorks 10.1.1 trial (no sign-in needed for 60 days) in a 64-bit
Wine 9 prefix with Japanese locale; Visual C++ runtimes from the installer, and
`msvcp140`, `vcruntime140` set to native. DocuWorks is not part of this
project; you download it from FUJIFILM and accept its licence yourself.
Note: `xdwapil.exe` stays running after a call, so capture output through a
file, not a pipe (as `check.py` does).
