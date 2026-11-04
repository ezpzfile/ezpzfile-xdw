// Runs ezpzxdw in a Web Worker so a crafted or huge .xdw cannot freeze the page.
// Used by ezpzxdw-safe.js; load it as a module worker next to ../pkg/.
//
// in:  { id, bytes: Uint8Array, limits: {maxPages, …}, want: "text" | "pages" | "check", includeHidden }
// out: { id, ok: true, result, hiddenChars } or { id, ok: false, error }
import init, { XdwDoc } from "../pkg/ezpzxdw_wasm.js";

const ready = init();

self.onmessage = async (e) => {
  const { id, bytes, limits = {}, want = "text", includeHidden = false } = e.data || {};
  try {
    await ready;
    const doc = XdwDoc.openWith(bytes, JSON.stringify(limits));
    doc.setIncludeHidden(!!includeHidden);
    const n = doc.pageCount();
    let result, hiddenChars = 0;
    if (want === "pages") {
      result = JSON.parse(doc.pages());
    } else {
      // "text": the text of every page; "check": draw every page, return nothing
      const texts = [];
      for (let i = 0; i < n; i++) {
        const t = doc.text(i);
        hiddenChars += doc.hiddenChars(i);
        doc.forget(i);
        if (want === "text") texts.push(t);
      }
      result = want === "text" ? texts : { pages: n };
    }
    doc.free();
    self.postMessage({ id, ok: true, result, hiddenChars });
  } catch (err) {
    self.postMessage({ id, ok: false, error: String((err && err.message) || err) });
  }
};
