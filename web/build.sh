#!/usr/bin/env bash
# Build the browser editor.
#   web/pkg/                    ES module + .wasm (for embedding in other sites)
#   web/dist/ezpzxdw-editor.html  the editor, one self-contained file (double-click, offline)
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
app = open("editor/app.js", encoding="utf-8").read()
ed = open("editor.html", encoding="utf-8").read()
ed = ed.replace("/*__EZPZXDW_GLUE__*/", glue).replace("/*__EZPZXDW_WASM__*/", wasm).replace("/*__EZPZXDW_APP__*/", app)
open("dist/ezpzxdw-editor.html", "w", encoding="utf-8").write(ed)
print("dist/ezpzxdw-editor.html", len(ed) // 1024, "KB")
PY
