#!/bin/sh
# Side by side: DocuWorks Viewer Light (left) and our editor (right), page 1.
# usage: compare.sh FILE.xdw OUTDIR
set -e
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
f="$1"; out="$2"; mkdir -p "$out"
b=$(basename "$f"); b=${b%.*}
python3 "$root/tools/dwview/view.py" "$f" "$out/$b-dw.png" --fit >/dev/null
tmp=$(mktemp --suffix=.xdw); cp "$f" "$tmp"
node "$here/shot.js" "$root/web/dist/ezxdw-editor.html" "$tmp" "$out/$b-web.png" 1; rm -f "$tmp"
