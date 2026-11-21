# dwapi: DocuWorks itself as a referee

`dwapi.c` is a small Windows program that calls the DocuWorks API
(`xdwapi.dll`, installed with DocuWorks Desk) under Wine. It lets the real
DocuWorks engine open our saved files, list pages and annotations with their
settings, and draw pages:

    i686-w64-mingw32-gcc -O1 -o dwapi.exe dwapi.c
    wine dwapi.exe info   'Z:\path\file.xdw'          # pages, annotations, settings (Shift_JIS)
    wine dwapi.exe render 'Z:\path\file.xdw' 1 'Z:\path\p1.bmp' 100
    wine dwapi.exe make   'Z:\path\copy.xdw'          # DocuWorks adds a date stamp, sticky note, text
    wine dwapi.exe addimg 'Z:\path\copy.xdw' 'Z:\path\pic.bmp' X Y   # DocuWorks adds a picture annotation (BMP only)
    wine dwapi.exe move   'Z:\path\copy.xdw' PAGE N X Y [W H]          # DocuWorks moves (and sizes) annotation N, then saves
    wine dwapi.exe addann 'Z:\path\copy.xdw' 32795 X Y 0 0 1000 500 …    # any annotation type; lines, markers and polygons take points
    wine dwapi.exe setattr 'Z:\path\copy.xdw' PAGE N i:%ArrowheadType=2  # DocuWorks changes a setting of annotation N
    wine dwapi.exe binder 'Z:\path\new.xbd' 'Z:\path\a.xdw' …           # a new binder with these documents
    wine dwapi.exe orig   'Z:\path\copy.xdw' 'Z:\path\file'             # attach original data (原本)
    wine dwapi.exe originfo 'Z:\path\copy.xdw'                           # the original data a document carries
    wine dwapi.exe getorig  'Z:\path\copy.xdw' N 'Z:\path\out'          # write original data N out
    wine dwapi.exe text   'Z:\path\file.xdw' PAGE 'Z:\path\out.txt'       # the text DocuWorks finds on a page
    python3 check.py out_*.xdw                        # ours vs DocuWorks: pages, annotations, drawing

`make` is how the attribute names DocuWorks 10 writes for date stamps and
sticky notes were found (see `docs/spec/XDW-FORMAT.md`).

Setup used: the latest DocuWorks (10.1.1) in a 64-bit
Wine 9 prefix with Japanese locale; Visual C++ runtimes from the installer, and
`msvcp140`, `vcruntime140` set to native. DocuWorks is not part of this
project; you download it from FUJIFILM and accept its licence yourself.
Under a sandbox whose environment holds very long variables, wineserver can
abort with "free(): invalid pointer"; run Wine with a clean environment
(`env -i HOME=… PATH=… WINEPREFIX=… LANG=ja_JP.UTF-8 DISPLAY=… wine …`).
Install DocuWorks with `msiexec /i dw10_act-sub.msi TRANSFORMS=1041.mst
DWSOURCEDIR=<the setup folder> /qn` (the package refuses to run without
DWSOURCEDIR), after the two Visual C++ runtimes it ships.
Note: `xdwapil.exe` stays running after a call, so capture output through a
file, not a pipe (as `check.py` does).
