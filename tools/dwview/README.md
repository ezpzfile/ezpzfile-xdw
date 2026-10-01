# dwview — DocuWorks Viewer Light as a referee

Opens a `.xdw` in the real **DocuWorks Viewer Light** (free, from FUJIFILM)
running under Wine on a virtual display, and reports what happened:

    python3 view.py FILE.xdw OUT.png [--fit] [--pages N]
    → OPEN | DIALOG | CRASH | TIMEOUT, and a screenshot

Setup (Linux):

    apt install wine64 xvfb xdotool imagemagick fonts-ipafont
    Xvfb :97 -screen 0 1280x900x24 &
    WINEPREFIX=~/wine/dw wine msiexec /i dwvlt10.msi   # the installer from FUJIFILM's site
    DISPLAY=:97 WINEPREFIX=~/wine/dw python3 view.py file.xdw shot.png

Only one viewer per Wine prefix at a time (the script stops the prefix's
programs before and after each file). For parallel runs copy the prefix and use
another display.

The viewer is not part of this project; you download it yourself and accept its
licence.
