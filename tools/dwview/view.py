#!/usr/bin/env python3
"""Open a .xdw in the real DocuWorks Viewer Light (Wine) and report what happened.

Usage: view.py FILE.xdw OUT.png [--pages N] [--fit]

With --pages N, pages 2..N are also captured as OUT-2.png, OUT-3.png, …
(the next-page button is clicked). With --fit the whole page is shown.

Prints one line: STATUS<TAB>detail, where STATUS is
  OPEN    the document window appeared (detail: window title | status bar pages)
  DIALOG  some other window appeared (message box) - its title is in detail
  CRASH   the viewer process ended without showing the document
  TIMEOUT nothing happened in time
Needs: Xvfb on $DISPLAY, a 64-bit wine prefix with Viewer Light installed (README.md).
"""
import os, subprocess, sys, time, shutil

PREFIX = os.environ.get("WINEPREFIX", "/home/claude/wine/dw")
EXE = r"C:\Program Files (x86)\FUJIFILM\DocuWorks\bin\DWVLT.exe"
APP = "DocuWorks Viewer Light"
ENV = dict(os.environ, WINEPREFIX=PREFIX, LANG="ja_JP.UTF-8", LC_ALL="ja_JP.UTF-8",
           WINEDEBUG="-all", DISPLAY=os.environ.get("DISPLAY", ":97"))


def windows():
    r = subprocess.run(["xdotool", "search", "--onlyvisible", "--name", "."], capture_output=True, text=True, env=ENV)
    out = []
    for wid in r.stdout.split():
        n = subprocess.run(["xdotool", "getwindowname", wid], capture_output=True, text=True, env=ENV).stdout.strip()
        if n and n != "Default IME":
            out.append((wid, n))
    return out


# toolbar buttons (window at 0,0, 1280x900)
NEXT_BUTTON = (301, 34)
FIT_BUTTON = (378, 34)  # zoom out (twice)


def click(wid, xy):
    subprocess.run(["xdotool", "mousemove", "--window", wid, str(xy[0]), str(xy[1]), "click", "1"], env=ENV,
                   stderr=subprocess.DEVNULL)


def kill():
    subprocess.run(["wineserver", "-k"], env=ENV, stdin=subprocess.DEVNULL,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=30)
    time.sleep(1.0)


def main():
    src, png = sys.argv[1], sys.argv[2]
    kill()
    tmpdir = os.path.join(PREFIX, "drive_c", "t", "run")
    os.makedirs(tmpdir, exist_ok=True)
    for f in os.listdir(tmpdir):
        os.remove(os.path.join(tmpdir, f))
    name = os.path.basename(src)
    shutil.copy(src, os.path.join(tmpdir, name))
    stem = os.path.splitext(name)[0]
    p = subprocess.Popen(["wine", EXE, "C:\\t\\run\\" + name], env=ENV, stdin=subprocess.DEVNULL,
                         stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
    status, detail, wid = "TIMEOUT", "", None
    t0 = time.time()
    other_since = None
    wait = float(os.environ.get("DWVIEW_WAIT", "10"))
    while time.time() - t0 < 40 + wait:
        time.sleep(1.0)
        ws = windows()
        doc = [w for w in ws if stem in w[1]]
        other = [w for w in ws if stem not in w[1]]
        if doc:
            time.sleep(4.0)
            ws = windows()
            doc = [w for w in ws if stem in w[1]]
            extra = [w for w in ws if stem not in w[1]]
            if extra:
                status, detail, wid = "DIALOG", " | ".join(w[1] for w in extra), extra[0][0]
            elif not doc:
                continue
            else:
                status, detail, wid = "OPEN", doc[0][1], doc[0][0]
            break
        if other:
            # the main window shows "- DocuWorks Viewer Light" while loading
            other_since = other_since or time.time()
            if time.time() - other_since > wait:
                status, detail, wid = "DIALOG", " | ".join(w[1] for w in other), other[0][0]
                break
        if p.poll() is not None and not ws:
            status, detail = "CRASH", f"exit={p.returncode}"
            break
    if status == "OPEN":
        subprocess.run(["xdotool", "windowsize", wid, "1280", "900"], env=ENV)
        subprocess.run(["xdotool", "windowmove", wid, "0", "0"], env=ENV)
        time.sleep(3.0)
        if "--fit" in sys.argv:
            for _ in range(int(os.environ.get("DWVIEW_ZOOMOUT", "1"))):
                click(wid, FIT_BUTTON)
                time.sleep(1.5)
    subprocess.run(["import", "-window", "root", png], env=ENV)
    if status == "OPEN" and "--pages" in sys.argv:
        n = int(sys.argv[sys.argv.index("--pages") + 1])
        base, ext = os.path.splitext(png)
        for k in range(2, n + 1):
            click(wid, NEXT_BUTTON)
            time.sleep(2.5)
            subprocess.run(["import", "-window", "root", f"{base}-{k}{ext}"], env=ENV)
    kill()
    print(f"{status}\t{detail}")


if __name__ == "__main__":
    main()
