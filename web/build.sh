#!/usr/bin/env bash
# Build the browser editor.
#   web/pkg/                         ES module + .wasm (for embedding in other sites)
#   web/dist/ezpzxdw-editor.html     the editor in Japanese, one self-contained file (double-click, offline)
#   web/dist/ezpzxdw-editor.en.html  the same in English
#   web/dist/ezpzxdw-editor.embed.html
#                                    the editor for a site that serves pkg/ itself: fill in
#                                    /*__EZPZXDW_WASM_URL__*/ with the .wasm URL and set <html lang>
# Needs: rustup target add wasm32-unknown-unknown ; cargo install wasm-bindgen-cli --version 0.2.129
set -euo pipefail
cd "$(dirname "$0")/../engine"
cargo build --release --target wasm32-unknown-unknown -p ezpzxdw-wasm
wasm-bindgen --target web --no-typescript --out-dir ../web/pkg target/wasm32-unknown-unknown/release/ezpzxdw_wasm.wasm
if command -v wasm-opt >/dev/null; then wasm-opt -Oz ../web/pkg/ezpzxdw_wasm_bg.wasm -o ../web/pkg/ezpzxdw_wasm_bg.wasm; fi
cd ../web
mkdir -p dist
python3 - <<'PY'
import base64, re
glue = open("pkg/ezpzxdw_wasm.js", encoding="utf-8").read()
glue = re.sub(r"^export \{[^}]*\};?\s*$", "", glue, flags=re.M)
glue = re.sub(r"^export (class|function|async function|const|let) ", r"\1 ", glue, flags=re.M)
wasm = base64.b64encode(open("pkg/ezpzxdw_wasm_bg.wasm", "rb").read()).decode()
# the editor script: words (i18n.js), the optional link to a framing page (host.js), the app
app = "\n".join(open(f, encoding="utf-8").read() for f in ("editor/i18n.js", "editor/host.js", "editor/app.js"))
ed = open("editor.html", encoding="utf-8").read()
ed = ed.replace("/*__EZPZXDW_GLUE__*/", glue).replace("/*__EZPZXDW_APP__*/", app)
for lang, name in (("ja", "ezpzxdw-editor.html"), ("en", "ezpzxdw-editor.en.html")):
    page = ed.replace("/*__EZPZXDW_WASM__*/", wasm).replace('<html lang="ja">', f'<html lang="{lang}">')
    open(f"dist/{name}", "w", encoding="utf-8").write(page)
    print(f"dist/{name}", len(page) // 1024, "KB")
open("dist/ezpzxdw-editor.embed.html", "w", encoding="utf-8").write(ed)
print("dist/ezpzxdw-editor.embed.html", len(ed) // 1024, "KB")
PY
