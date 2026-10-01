"""Open every file in DocuWorks' own engine (xdwapi.dll via dwapi.exe under Wine):
page count and annotations per page must match what our reader sees, and
the first and last page must draw. Usage: python3 check.py FILES..."""
import os, re, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ENV = dict(os.environ, LANG="ja_JP.UTF-8", LC_ALL="ja_JP.UTF-8", WINEDEBUG="-all")
ENV.setdefault("WINEPREFIX", os.path.expanduser("~/wine/desk"))
ENV.setdefault("DISPLAY", ":95")
EXE = os.environ.get("DWAPI_EXE", os.path.join(HERE, "dwapi.exe"))
OURS = os.environ.get("EZPZXDW", os.path.join(HERE, "..", "..", "engine", "target", "release", "ezpzxdw"))


def win(p):
    return "Z:" + os.path.abspath(p).replace("/", "\\")


def ours(path):
    out = subprocess.run([OURS, "pages", path], capture_output=True, text=True).stdout
    pages = []
    for line in out.splitlines():
        if line.startswith("page "):
            pages.append(0)
        elif line.startswith("  ") and pages and not line.startswith("  page 0x") and not line.startswith("  document") and not line.startswith("  signature"):
            pages[-1] += 1
    return pages


def run(args):
    # a file, not a pipe: DocuWorks leaves a helper running that would hold a pipe open
    with open("/tmp/dwapi.out", "wb") as fo:
        subprocess.run(["wine", EXE] + args, stdout=fo, stderr=subprocess.DEVNULL, env=ENV, timeout=300)
    return open("/tmp/dwapi.out", "rb").read().decode("cp932", "replace")


def dw(path):
    out = run(["info", win(path)])
    if "OPEN-ERROR" in out or "document pages" not in out:
        return None, out[-300:]
    pages = [int(m) for m in re.findall(r"^page \d+ size \S+ type \d+ annotations (\d+)", out, re.M)]
    return pages, out


def render(path, page, bmp):
    out = run(["render", win(path), str(page), win(bmp), "60"])
    return "render 0" in out and os.path.exists(bmp) and os.path.getsize(bmp) > 1000


ok = 0
for f in sys.argv[1:]:
    mine = ours(f)
    theirs, text = dw(f)
    if theirs is None:
        print(f"FAIL-OPEN\t{f}\t{text!r}", flush=True)
        continue
    same = mine == theirs
    r1 = render(f, 1, f + ".p1.bmp")
    rl = render(f, len(theirs), f + ".pl.bmp") if len(theirs) > 1 else r1
    good = same and r1 and rl
    ok += good
    print(f"{'OK' if good else 'DIFF'}\t{os.path.basename(f)}\tpages {len(mine)}/{len(theirs)}\tannots {sum(mine)}/{sum(theirs)}\trender {r1} {rl}" + ("" if same else f"\n\tours {mine}\n\tdw   {theirs}"), flush=True)
print(f"{ok}/{len(sys.argv) - 1} match and draw in DocuWorks")
