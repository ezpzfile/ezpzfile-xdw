// A page that shows this editor in a frame (ezpzfile.com does, at /xdw-editor).
//
// Everything here is off when the editor runs on its own: it only talks to a
// parent window on the same origin, with postMessage. Messages are named
// `ezpz-xdw-<what>`, the same set the HWP editor on that site uses:
//
//   editor → page  ready              the editor is up
//                  state {dirty, hasDocument, fileName}
//                                     so the page can ask before leaving with unsaved changes
//                  event {name, params}
//                                     what people use (file type, saves, features), never
//                                     file names or contents; the page sends it to its analytics
//                  share {rect} | {done}
//                                     the share button: where to open the page's share menu,
//                                     or that a phone's own share sheet was used
//   page → editor  hello              "are you there?": answered with ready once the editor is up
//                                     (the frame can finish loading before the page's own script runs)
//                  open {file}        open this File (handed over from another tool)
//                  reads {title, items:[{title, href}]}
//                                     articles to list under the empty screen
//                  video {id, label}  a how-to video (a YouTube id) to show under the drop box:
//                                     a thumbnail first, the player only once it is pressed
//
// The narrow-screen menu button asks the page to open its side menu with a
// plain `ezpz:nav-open` event on the parent window.

const HOST = (() => {
  const PREFIX = "ezpz-xdw";
  let parent = null;
  try {
    if (window.parent !== window && window.parent.location.origin === location.origin) parent = window.parent;
  } catch {
    // a frame on another site: stay on our own
  }
  const post = (type, data = {}) => {
    if (parent) parent.postMessage({ type: `${PREFIX}-${type}`, ...data }, location.origin);
  };
  const counted = new Set();
  const listeners = {};
  let last = "";
  let up = false;
  if (parent) {
    window.addEventListener("message", (e) => {
      if (e.origin !== location.origin || e.source !== parent) return;
      const type = e.data && e.data.type;
      if (typeof type !== "string" || !type.startsWith(PREFIX + "-")) return;
      const what = type.slice(PREFIX.length + 1);
      if (what === "hello") {
        if (up) post("ready");
        return;
      }
      for (const fn of listeners[what] || []) fn(e.data);
    });
  }

  /** An error as one short word for the measurements (the text may hold a file name). */
  function errorKind(err) {
    const text = String((err && (err.message || err)) || "");
    if (/password|encrypt|protect|暗号|パスワード/i.test(text)) return "password";
    if (/memory|allocation|out of bounds|unreachable/i.test(text)) return "memory";
    if (/unsupported|not supported|not a |format|signature|magic|対応/i.test(text)) return "unsupported";
    if (/parse|corrupt|invalid|damaged|broken|truncat|壊れ/i.test(text)) return "damaged";
    if (/NotReadable|permission|NotAllowed|Security/i.test(text)) return "read";
    return err && err.name && err.name !== "Error" ? String(err.name).slice(0, 30) : "other";
  }

  return {
    embedded: !!parent,
    errorKind,
    ready() { up = true; post("ready"); },
    /** Sent only when something changed. */
    state(s) {
      const key = JSON.stringify(s);
      if (key === last) return;
      last = key;
      post("state", s);
    },
    event(name, params = {}) { post("event", { name, params }); },
    /** Once per document and feature: bold a hundred times counts once. */
    once(name, params = {}) {
      const key = name + ":" + Object.values(params).join(",");
      if (counted.has(key)) return;
      counted.add(key);
      post("event", { name, params });
    },
    resetOnce() { counted.clear(); },
    on(what, fn) { (listeners[`${what}`] ||= []).push(fn); },
    navOpen() {
      try { parent && parent.dispatchEvent(new Event("ezpz:nav-open")); } catch {}
    },
    /** The site's home page in the page's language ("/ja" or "/"). */
    homeHref() {
      if (!parent) return null;
      const m = /^\/([a-z]{2}(?:-[a-z]{2})?)\//.exec(parent.location.pathname + "/");
      return m ? "/" + m[1] : "/";
    },
    /** Phones share with their own sheet; elsewhere the page opens its share menu at the button. */
    async share(btn) {
      if (!parent) return;
      const at = parent.location;
      if (typeof navigator.share === "function" && matchMedia("(pointer: coarse)").matches) {
        try {
          await navigator.share({ title: parent.document.title, url: at.origin + at.pathname });
          post("share", { done: "native" });
        } catch {
          // closed or not allowed
        }
        return;
      }
      const r = btn.getBoundingClientRect();
      post("share", { rect: { x: r.left, y: r.top, w: r.width, h: r.height } });
    },
    /** Errors nobody caught, counted by kind (once each). */
    watchErrors() {
      if (!parent) return;
      const seen = new Set();
      const once = (code) => { if (!seen.has(code)) { seen.add(code); post("event", { name: "js_error", params: { error_code: code } }); } };
      window.addEventListener("error", (e) => { if (!/ResizeObserver/i.test(String(e.message || ""))) once(errorKind(e.error || e.message)); });
      window.addEventListener("unhandledrejection", (e) => once(errorKind(e.reason)));
    },
  };
})();
