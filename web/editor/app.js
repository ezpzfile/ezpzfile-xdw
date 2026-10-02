// EZPZ File XDW editor UI.
// The engine (Rust → WebAssembly, `XdwDoc`) reads the document, draws pages
// as display lists, makes annotation drawings and saves. This file shows the
// pages and turns mouse/keyboard input into engine calls.
//
// Expects `__wbg_init`, `XdwDoc` and `WASM_B64` in scope (see build.sh).

const ICONS = {"folder-open": "<path d=\"m6 14 1.5-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.54 6a2 2 0 0 1-1.95 1.5H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H18a2 2 0 0 1 2 2v2\" />", "save": "<path d=\"M15.2 3a2 2 0 0 1 1.4.6l3.8 3.8a2 2 0 0 1 .6 1.4V19a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z\" /> <path d=\"M17 21v-7a1 1 0 0 0-1-1H8a1 1 0 0 0-1 1v7\" /> <path d=\"M7 3v4a1 1 0 0 0 1 1h7\" />", "printer": "<path d=\"M6 18H4a2 2 0 0 1-2-2v-5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2v5a2 2 0 0 1-2 2h-2\" /> <path d=\"M6 9V3a1 1 0 0 1 1-1h10a1 1 0 0 1 1 1v6\" /> <rect x=\"6\" y=\"14\" width=\"12\" height=\"8\" rx=\"1\" />", "undo-2": "<path d=\"M9 14 4 9l5-5\" /> <path d=\"M4 9h10.5a5.5 5.5 0 0 1 5.5 5.5a5.5 5.5 0 0 1-5.5 5.5H11\" />", "redo-2": "<path d=\"m15 14 5-5-5-5\" /> <path d=\"M20 9H9.5A5.5 5.5 0 0 0 4 14.5A5.5 5.5 0 0 0 9.5 20H13\" />", "search": "<circle cx=\"11\" cy=\"11\" r=\"8\" /> <path d=\"m21 21-4.3-4.3\" />", "minus": "<path d=\"M5 12h14\" />", "plus": "<path d=\"M5 12h14\" /> <path d=\"M12 5v14\" />", "chevron-down": "<path d=\"m6 9 6 6 6-6\" />", "chevron-right": "<path d=\"m9 18 6-6-6-6\" />", "panel-left": "<rect width=\"18\" height=\"18\" x=\"3\" y=\"3\" rx=\"2\" /> <path d=\"M9 3v18\" />", "panel-right": "<rect width=\"18\" height=\"18\" x=\"3\" y=\"3\" rx=\"2\" /> <path d=\"M15 3v18\" />", "type": "<polyline points=\"4 7 4 4 20 4 20 7\" /> <line x1=\"9\" x2=\"15\" y1=\"20\" y2=\"20\" /> <line x1=\"12\" x2=\"12\" y1=\"4\" y2=\"20\" />", "trash-2": "<path d=\"M3 6h18\" /> <path d=\"M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6\" /> <path d=\"M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2\" /> <line x1=\"10\" x2=\"10\" y1=\"11\" y2=\"17\" /> <line x1=\"14\" x2=\"14\" y1=\"11\" y2=\"17\" />", "x": "<path d=\"M18 6 6 18\" /> <path d=\"m6 6 12 12\" />", "check": "<path d=\"M20 6 9 17l-5-5\" />", "file-text": "<path d=\"M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z\" /> <path d=\"M14 2v4a2 2 0 0 0 2 2h4\" /> <path d=\"M10 9H8\" /> <path d=\"M16 13H8\" /> <path d=\"M16 17H8\" />", "info": "<circle cx=\"12\" cy=\"12\" r=\"10\" /> <path d=\"M12 16v-4\" /> <path d=\"M12 8h.01\" />", "keyboard": "<path d=\"M10 8h.01\" /> <path d=\"M12 12h.01\" /> <path d=\"M14 8h.01\" /> <path d=\"M16 12h.01\" /> <path d=\"M18 8h.01\" /> <path d=\"M6 8h.01\" /> <path d=\"M7 16h10\" /> <path d=\"M8 12h.01\" /> <rect width=\"20\" height=\"16\" x=\"2\" y=\"4\" rx=\"2\" />", "mouse-pointer-2": "<path d=\"M4.037 4.688a.495.495 0 0 1 .651-.651l16 6.5a.5.5 0 0 1-.063.947l-6.124 1.58a2 2 0 0 0-1.438 1.435l-1.579 6.126a.5.5 0 0 1-.947.063z\" />", "highlighter": "<path d=\"m9 11-6 6v3h9l3-3\" /> <path d=\"m22 12-4.6 4.6a2 2 0 0 1-2.8 0l-5.2-5.2a2 2 0 0 1 0-2.8L14 4\" />", "square": "<rect width=\"18\" height=\"18\" x=\"3\" y=\"3\" rx=\"2\" />", "circle": "<circle cx=\"12\" cy=\"12\" r=\"10\" />", "line": "<path d=\"M5 19 19 5\" />", "rotate-ccw": "<path d=\"M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8\" /> <path d=\"M3 3v5h5\" />", "rotate-cw": "<path d=\"M21 12a9 9 0 1 1-9-9c2.52 0 4.93 1 6.74 2.74L21 8\" /> <path d=\"M21 3v5h-5\" />", "arrow-up": "<path d=\"m5 12 7-7 7 7\" /> <path d=\"M12 19V5\" />", "arrow-down": "<path d=\"M12 5v14\" /> <path d=\"m19 12-7 7-7-7\" />", "maximize": "<path d=\"M8 3H5a2 2 0 0 0-2 2v3\" /> <path d=\"M21 8V5a2 2 0 0 0-2-2h-3\" /> <path d=\"M3 16v3a2 2 0 0 0 2 2h3\" /> <path d=\"M16 21h3a2 2 0 0 0 2-2v-3\" />", "file-down": "<path d=\"M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z\" /> <path d=\"M14 2v4a2 2 0 0 0 2 2h4\" /> <path d=\"M12 18v-6\" /> <path d=\"m9 15 3 3 3-3\" />", "sticky-note": "<path d=\"M15.5 3H5a2 2 0 0 0-2 2v14c0 1.1.9 2 2 2h14a2 2 0 0 0 2-2V8.5L15.5 3Z\" /> <path d=\"M15 3v6h6\" />", "stamp": "<path d=\"M5 22h14\" /> <path d=\"M19.27 13.73A2.5 2.5 0 0 0 17.5 13h-11A2.5 2.5 0 0 0 4 15.5V17a1 1 0 0 0 1 1h14a1 1 0 0 0 1-1v-1.5c0-.66-.26-1.3-.73-1.77Z\" /> <path d=\"M14 13V8.5C14 7 15 7 15 5a3 3 0 0 0-3-3c-1.69 0-3 1-3 3 0 2 1 2 1 3.5V13\" />", "file-plus": "<path d=\"M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z\" /> <path d=\"M14 2v4a2 2 0 0 0 2 2h4\" /> <path d=\"M9 15h6\" /> <path d=\"M12 18v-6\" />", "image-plus": "<path d=\"M16 5h6\" /> <path d=\"M19 2v6\" /> <path d=\"M21 11.5V19a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h7.5\" /> <path d=\"m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21\" /> <circle cx=\"9\" cy=\"9\" r=\"2\" />", "files": "<path d=\"M20 7h-3a2 2 0 0 1-2-2V2\" /> <path d=\"M9 18a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h7l4 4v10a2 2 0 0 1-2 2Z\" /> <path d=\"M3 7.6v12.8A1.6 1.6 0 0 0 4.6 22h9.8\" />", "pencil": "<path d=\"M21.174 6.812a1 1 0 0 0-3.986-3.987L3.842 16.174a2 2 0 0 0-.5.83l-1.321 4.352a.5.5 0 0 0 .623.622l4.353-1.32a2 2 0 0 0 .83-.497z\" /> <path d=\"m15 5 4 4\" />", "chevron-up": "<path d=\"m18 15-6-6-6 6\" />"};

const ic = (n, cls = "i") => `<svg class="${cls}" viewBox="0 0 24 24" aria-hidden="true">${ICONS[n] || ""}</svg>`;

const U = 96 / 2540; // CSS px per 1/100 mm at 100 %
const SANS = '"MS Gothic","MS PGothic","Hiragino Kaku Gothic ProN","Hiragino Sans","Yu Gothic","Noto Sans JP","Noto Sans CJK JP",sans-serif';
const SERIF = '"MS Mincho","MS PMincho","Hiragino Mincho ProN","Yu Mincho","Noto Serif JP","Noto Serif CJK JP",serif';
const IS_MAC = /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);
const MOD = IS_MAC ? "⌘" : "Ctrl+";

const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => Array.from(r.querySelectorAll(s));
const el = (tag, attrs = {}, ...kids) => {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (k === "class") e.className = v;
    else if (k === "text") e.textContent = v;
    else if (k === "html") e.innerHTML = v;
    else if (k.startsWith("on")) e.addEventListener(k.slice(2), v);
    else if (v !== false && v != null) e.setAttribute(k, v === true ? "" : v);
  }
  for (const k of kids) if (k != null) e.append(k);
  return e;
};
const store = {
  get(k, d) { try { const v = localStorage.getItem("ezpzxdw." + k); return v == null ? d : JSON.parse(v); } catch { return d; } },
  set(k, v) { try { localStorage.setItem("ezpzxdw." + k, JSON.stringify(v)); } catch {} },
};
const hex = (n) => "#" + (n >>> 0 & 0xffffff).toString(16).padStart(6, "0");
const num = (h) => parseInt(h.slice(1), 16);

// ------------------------------------------------------------------ state
const S = {
  ed: null,
  name: "",
  zoom: store.get("zoom", 1),
  info: [],          // pages(): [{w,h,objects:[…]}]
  pages: [],         // {div, canvas, drawn, busy}
  sel: null,         // {p, o}
  tool: "select",
  color: store.get("color", "#d40000"),
  hl: store.get("hl", "#ffe14d"),
  width: store.get("width", 2),
  size: store.get("size", 12),
  cur: 0,
  observer: null,
  thumbs: [],
  binder: null,      // {name, docs:[{name, first_page, pages}]} for a .xbd
  ext: ".xdw",
  sticky: store.get("sticky2", "#ffff64"),
  stamp: store.get("stamp", { top: "", bottom: "", fmt: "yy", size: 1800, color: "#e60012" }),
};
const sc = () => S.zoom * U;

const COLORS = ["#131a2e", "#d40000", "#0155ff", "#00a36c", "#f08c00", "#8b5cf6"];
const HIGHLIGHTS = ["#ffe14d", "#9cf0b0", "#9fd3ff", "#ffb3d9", "#ffc58a"];
// DocuWorks' own sticky-note colours
const STICKIES = ["#ffff64", "#fffac3", "#ffc2ff", "#9dbfff", "#9dffc2"];
const STAMP_COLORS = ["#e60012", "#0155ff", "#131a2e"];
function stampDate(fmt) {
  const d = new Date();
  const p = (v) => String(v).padStart(2, "0");
  return fmt === "yyyy" ? `${d.getFullYear()}.${p(d.getMonth() + 1)}.${p(d.getDate())}` : `'${p(d.getFullYear() % 100)}.${p(d.getMonth() + 1)}.${p(d.getDate())}`;
}

// ------------------------------------------------------------------ boot
function b64bytes(s) { const bin = atob(s); const u = new Uint8Array(bin.length); for (let i = 0; i < bin.length; i++) u[i] = bin.charCodeAt(i); return u; }

async function boot() {
  await (WASM_B64.startsWith("/*") ? __wbg_init() : __wbg_init({ module_or_path: b64bytes(WASM_B64) }));
  buildMenus();
  buildToolbar();
  bindChrome();
  renderProps();
  updateChrome();
}

// ------------------------------------------------------------------ drawing
function path2d(a) {
  const p = new Path2D();
  for (let i = 0; i < a.length;) {
    const c = a[i++];
    if (c === 0) p.moveTo(a[i++], a[i++]);
    else if (c === 1) p.lineTo(a[i++], a[i++]);
    else if (c === 2) { p.bezierCurveTo(a[i], a[i + 1], a[i + 2], a[i + 3], a[i + 4], a[i + 5]); i += 6; }
    else p.closePath();
  }
  return p;
}

function fontFor(face, weight, italic, size) {
  const stack = /明朝|Mincho|Serif|Times|Century/i.test(face) ? SERIF : SANS;
  const f = face ? `"${face.replace(/"/g, "")}",` : "";
  return `${italic ? "italic " : ""}${weight >= 600 ? 700 : 400} ${size}px ${f}${stack}`;
}

async function loadImages(i, d) {
  // take every picture's bytes first: other renders of the same page may
  // drop the engine's copy while we wait for the decoder
  const raw = d.images.map((_, k) => S.ed.image(i, k));
  const out = [];
  for (let k = 0; k < d.images.length; k++) {
    const im = d.images[k], bytes = raw[k];
    try {
      if (!bytes.length) out.push(null);
      else if (im.k === "jpeg") out.push(await createImageBitmap(new Blob([bytes], { type: "image/jpeg" })));
      else out.push(await createImageBitmap(new ImageData(new Uint8ClampedArray(bytes.buffer, bytes.byteOffset, bytes.length), im.w, im.h)));
    } catch (e) { out.push(null); console.warn("picture", i, k, e); }
  }
  return out;
}

const MEASURE = new Map();
function measure(ctx, c) {
  const key = ctx.font + "\u0000" + c;
  let w = MEASURE.get(key);
  if (w === undefined) {
    w = ctx.measureText(c).width;
    if (MEASURE.size > 20000) MEASURE.clear();
    MEASURE.set(key, w);
  }
  return w;
}

// in vertical writing these turn with the line instead of standing upright
const TURN = new Set(Array.from("「」『』（）【】〔〕［］｛｝〈〉《》ー～〜…‥－＝→←―─"));

const isFull = (c) => c.codePointAt(0) >= 0x2e80 && !(c >= "｡" && c <= "ﾟ");

/** Draw display list `d` (with its pictures) on `ctx`, `k` px per unit. */
function paint(ctx, d, imgs, k) {
  ctx.setTransform(k, 0, 0, k, 0, 0);
  ctx.fillStyle = "#fff";
  ctx.fillRect(0, 0, d.w, d.h);
  const clips = d.clips.map((c) => c.map(path2d));
  const minW = 0.8 / k;
  let cur = 0;
  ctx.save();
  for (const it of d.items) {
    const t = it[0];
    const clip = t === "f" ? it[4] : t === "s" ? it[7] : t === "t" ? it[15] : it[3];
    if (clip !== cur) {
      ctx.restore(); ctx.save();
      if (clip) for (const p of clips[clip] || []) ctx.clip(p);
      cur = clip;
    }
    if (t === "f") {
      ctx.fillStyle = hex(it[2]);
      if (it[5]) { ctx.save(); ctx.globalCompositeOperation = "multiply"; }
      ctx.fill(path2d(it[1]), it[3] ? "evenodd" : "nonzero");
      if (it[5]) ctx.restore();
    } else if (t === "s") {
      ctx.strokeStyle = hex(it[2]);
      ctx.lineWidth = Math.max(it[3], minW);
      ctx.setLineDash(it[4]);
      ctx.lineCap = ["round", "square", "butt"][it[5]] || "round";
      ctx.lineJoin = ["round", "bevel", "miter"][it[6]] || "round";
      ctx.stroke(path2d(it[1]));
    } else if (t === "t") {
      const [, x, y, angle, size, sx, face, weight, italic, under, strike, vert, color, text, xs] = it;
      if (size * k < 0.6) continue;
      ctx.save();
      ctx.translate(x, y);
      if (angle) ctx.rotate(-angle * Math.PI / 180);
      if (sx && sx !== 1) ctx.scale(sx, 1);
      ctx.font = fontFor(face, weight, italic, size);
      ctx.fillStyle = hex(color);
      ctx.textBaseline = "alphabetic";
      const chars = Array.from(text);
      for (let n = 0; n < chars.length; n++) {
        const c = chars[n];
        const cx = xs.length ? xs[n] : n * size;
        const adv = xs.length > n + 1 ? xs[n + 1] - cx : 0;
        if (!vert && adv > 0) {
          // proportional fonts give some characters less room than the
          // fallback font's glyph needs: squeeze the glyph into its advance
          const w0 = measure(ctx, c);
          if (w0 > adv * 1.15) {
            ctx.save();
            ctx.translate(cx, 0);
            ctx.scale(adv / w0, 1);
            ctx.fillText(c, 0, 0);
            ctx.restore();
            continue;
          }
        }
        if (vert && isFull(c) && !TURN.has(c)) {
          // vertical writing: CJK glyphs stand upright
          ctx.save();
          ctx.translate(cx + size * 0.5, -size * 0.38);
          ctx.rotate(-Math.PI / 2);
          ctx.textAlign = "center";
          ctx.textBaseline = "middle";
          ctx.fillText(c, 0, 0);
          ctx.restore();
        } else ctx.fillText(c, cx, 0);
      }
      if (under || strike) {
        const w = xs.length ? (xs[xs.length - 1] + size) : chars.length * size;
        ctx.fillRect(0, under ? size * 0.12 : -size * 0.3, w, Math.max(size * 0.05, minW));
      }
      ctx.restore();
    } else if (t === "i") {
      const img = imgs[it[1]];
      if (!img) continue;
      const m = it[2];
      ctx.save();
      ctx.transform(m[0], m[1], m[2], m[3], m[4], m[5]);
      if (it[4] < 1) ctx.globalAlpha = it[4];
      if (it[5]) ctx.globalCompositeOperation = "multiply";
      ctx.drawImage(img, 0, 0, 1, 1);
      ctx.restore();
    }
  }
  ctx.restore();
}

/** Render page i into `canvas` at k CSS px per unit. */
async function renderTo(i, canvas, k) {
  const d = JSON.parse(S.ed.render(i));
  const imgs = await loadImages(i, d);
  const dpr = window.devicePixelRatio || 1;
  canvas.width = Math.max(1, Math.round(d.w * k * dpr));
  canvas.height = Math.max(1, Math.round(d.h * k * dpr));
  canvas.style.width = Math.round(d.w * k) + "px";
  canvas.style.height = Math.round(d.h * k) + "px";
  paint(canvas.getContext("2d"), d, imgs, k * dpr);
  imgs.forEach((b) => b && b.close && b.close());
  return d;
}

// ------------------------------------------------------------------ pages
function buildPages() {
  const host = $("#pages");
  host.replaceChildren();
  if (S.observer) S.observer.disconnect();
  S.observer = new IntersectionObserver((ents) => {
    for (const e of ents) if (e.isIntersecting) drawPage(+e.target.dataset.i);
  }, { root: $("#scroller"), rootMargin: "600px 0px" });
  S.pages = S.info.map((p, i) => {
    const div = el("div", { class: "page", "data-i": i });
    const canvas = el("canvas");
    div.append(canvas, el("span", { class: "no", text: String(i + 1) }));
    host.append(div);
    sizePage(div, p);
    bindPage(div, i);
    S.observer.observe(div);
    return { div, canvas, drawn: false, busy: false };
  });
  buildThumbs();
}

function sizePage(div, p) {
  div.style.width = Math.round(p.w * sc()) + "px";
  div.style.height = Math.round(p.h * sc()) + "px";
}

/** Lay the page's text over the drawing as invisible, selectable text,
 * placed where the drawing puts each line (as PDF viewers do). */
function buildTextLayer(pg, d) {
  const old = $(".textlayer", pg.div);
  if (old) old.remove();
  const k = sc();
  const layer = el("div", { class: "textlayer" });
  const ctx = (buildTextLayer.ctx ||= document.createElement("canvas").getContext("2d"));
  let lastY = null;
  for (const it of d.items) {
    if (it[0] !== "t") continue;
    const [, x, y, angle, size, sx, face, weight, italic, , , , , text, xs] = it;
    if (!text.trim() || size * k < 2) continue;
    const chars = Array.from(text);
    ctx.font = fontFor(face, weight, italic, size);
    // the width the drawing gives the line: up to the last character's place
    // plus that character
    const target = xs.length >= chars.length ? xs[chars.length - 1] + measure(ctx, chars[chars.length - 1]) : chars.length * size;
    const natural = measure(ctx, text) || 1;
    // a new line in the copied text when this item starts below the last one
    if (lastY != null && Math.abs(y - lastY) > size * 0.5) layer.append(document.createElement("br"));
    lastY = y;
    const span = el("span", { text });
    span.style.font = fontFor(face, weight, italic, size * k);
    span.style.transform = `translate(${x * k}px, ${y * k}px) rotate(${-(angle || 0)}deg) scale(${(sx || 1) * target / natural}, 1) translateY(-0.86em)`;
    layer.append(span);
  }
  pg.canvas.after(layer);
}

async function drawPage(i, force = false) {
  const pg = S.pages[i];
  if (!pg || pg.busy || (pg.drawn && !force)) return;
  pg.busy = true;
  try {
    const d = await renderTo(i, pg.canvas, sc());
    buildTextLayer(pg, d);
    S.ed.forget(i);
    pg.drawn = true;
  } catch (e) {
    console.error(e);
    toast("ページ " + (i + 1) + " を表示できませんでした");
  }
  pg.busy = false;
  if (S.sel && S.sel.p === i) showSel();
}

function buildThumbs() {
  const list = $("#pagelist");
  list.replaceChildren();
  S.binder = JSON.parse(S.ed.binder());
  const starts = new Map((S.binder ? S.binder.docs : []).map((d, k) => [d.first_page, k]));
  S.thumbs = S.info.map((p, i) => {
    if (starts.has(i)) list.append(docHeader(starts.get(i)));
    const c = el("canvas");
    const li = el("li", { "data-i": i, draggable: "true" },
      el("span", { class: "thumb" }, c),
      el("span", { class: "tools" },
        el("button", { title: "左へ90°回転", html: ic("rotate-ccw"), onclick: (e) => { e.stopPropagation(); rotatePage(i, 3); } }),
        el("button", { title: "右へ90°回転", html: ic("rotate-cw"), onclick: (e) => { e.stopPropagation(); rotatePage(i, 1); } }),
        el("button", { title: "ページを削除", html: ic("trash-2"), onclick: (e) => { e.stopPropagation(); deletePage(i); } })),
      el("span", { class: "n", text: String(i + 1) }));
    li.addEventListener("click", () => gotoPage(i));
    li.addEventListener("dragstart", (e) => { e.dataTransfer.setData("text/x-page", String(i)); e.dataTransfer.effectAllowed = "move"; });
    li.addEventListener("dragover", (e) => { if (e.dataTransfer.types.includes("text/x-page")) { e.preventDefault(); li.classList.add("dragover"); } });
    li.addEventListener("dragleave", () => li.classList.remove("dragover"));
    li.addEventListener("drop", (e) => {
      li.classList.remove("dragover");
      const from = +e.dataTransfer.getData("text/x-page");
      if (Number.isFinite(from) && e.dataTransfer.types.includes("text/x-page")) { e.preventDefault(); e.stopPropagation(); movePage(from, i); }
    });
    list.append(li);
    return { c, done: false };
  });
  $("#pagecount").textContent = S.binder ? `${S.binder.docs.length} 文書 · ${S.info.length} ページ` : S.info.length + " ページ";
  $("#binder-add").hidden = !S.binder;
  $("#menu-binder").hidden = !S.binder;
  drawThumbs();
}

function docHeader(k) {
  const d = S.binder.docs[k];
  const n = S.binder.docs.length;
  const li = el("li", { class: "doc", "data-doc": k, title: d.name },
    el("span", { class: "dn", html: ic("file-text") + "<span></span>" }),
    el("span", { class: "tools" },
      el("button", { title: "文書名を変更", html: ic("pencil"), onclick: (e) => { e.stopPropagation(); renameDoc(k); } }),
      el("button", { title: "文書を上へ", html: ic("chevron-up"), disabled: k === 0, onclick: (e) => { e.stopPropagation(); moveDoc(k, k - 1); } }),
      el("button", { title: "文書を下へ", html: ic("chevron-down"), disabled: k === n - 1, onclick: (e) => { e.stopPropagation(); moveDoc(k, k + 1); } }),
      el("button", { title: "文書をバインダーから外す", html: ic("trash-2"), disabled: n <= 1, onclick: (e) => { e.stopPropagation(); deleteDoc(k); } })));
  $("span span", li).textContent = d.name || "（名前なし）";
  li.addEventListener("click", () => gotoPage(d.first_page));
  li.addEventListener("dblclick", () => renameDoc(k));
  return li;
}

async function drawThumbs() {
  for (let i = 0; i < S.thumbs.length; i++) {
    const t = S.thumbs[i];
    if (t.done) continue;
    t.done = true;
    const w = 160;
    try { await renderTo(i, t.c, w / S.info[i].w); S.ed.forget(i); } catch {}
    await new Promise((r) => setTimeout(r, 0));
  }
  markCurrent();
}

function markCurrent() {
  $$("#pagelist li[data-i]").forEach((li) => li.classList.toggle("cur", +li.dataset.i === S.cur));
  const bd = docOf(S.cur);
  $$("#pagelist li.doc").forEach((li) => li.classList.toggle("cur", bd != null && +li.dataset.doc === bd));
}

/** The binder document page `i` belongs to (null for a plain document). */
function docOf(i) {
  if (!S.binder) return null;
  let k = 0;
  S.binder.docs.forEach((d, j) => { if (i >= d.first_page) k = j; });
  return k;
}

function gotoPage(i) {
  const pg = S.pages[i];
  if (!pg) return;
  $("#scroller").scrollTo({ top: pg.div.offsetTop - 20, behavior: "smooth" });
  S.cur = i;
  markCurrent();
  updateStatus();
}

function trackCurrent() {
  const s = $("#scroller");
  const mid = s.scrollTop + s.clientHeight / 3;
  let cur = 0;
  S.pages.forEach((p, k) => { if (p.div.offsetTop <= mid) cur = k; });
  if (cur !== S.cur) { S.cur = cur; markCurrent(); updateStatus(); }
}

/** After an edit: re-read pages, redraw what changed. */
function refresh(pagesChanged) {
  const before = S.info.length;
  S.info = JSON.parse(S.ed.pages());
  if (pagesChanged === "all" || S.info.length !== before) {
    buildPages();
  } else {
    for (const i of pagesChanged || []) {
      const pg = S.pages[i];
      if (!pg) continue;
      sizePage(pg.div, S.info[i]);
      pg.drawn = false;
      drawPage(i, true);
      if (S.thumbs[i]) { S.thumbs[i].done = false; }
    }
    drawThumbs();
  }
  if (S.sel && !(S.info[S.sel.p] && S.info[S.sel.p].objects[S.sel.o])) S.sel = null;
  showSel();
  renderProps();
  updateChrome();
}

// ------------------------------------------------------------------ selection & tools
function objAt(i, u, v) {
  const objs = S.info[i].objects;
  const pad = 4 / sc();
  for (let k = objs.length - 1; k >= 0; k--) {
    const o = objs[k];
    if (o.kind === "page") continue;
    if (u >= o.x - pad && u <= o.x + o.w + pad && v >= o.y - pad && v <= o.y + o.h + pad) return k;
  }
  return -1;
}

function showSel() {
  $$(".selbox").forEach((b) => b.remove());
  if (!S.sel) return;
  const o = S.info[S.sel.p]?.objects[S.sel.o];
  const pg = S.pages[S.sel.p];
  if (!o || !pg) return;
  const b = el("div", { class: "selbox" + (o.editable ? "" : " ro") });
  for (const h of ["nw", "ne", "sw", "se"]) b.append(el("span", { class: "h " + h, "data-h": h }));
  placeBox(b, o.x, o.y, o.w, o.h);
  pg.div.append(b);
}

function placeBox(b, x, y, w, h) {
  b.style.left = x * sc() + "px";
  b.style.top = y * sc() + "px";
  b.style.width = Math.max(2, w * sc()) + "px";
  b.style.height = Math.max(2, h * sc()) + "px";
}

function select(p, o) {
  S.sel = o == null || o < 0 ? null : { p, o };
  showSel();
  renderProps();
}

function setTool(t) {
  S.tool = t;
  $$("[data-tool]").forEach((b) => b.classList.toggle("on", b.dataset.tool === t));
  S.pages.forEach((p) => {
    p.div.classList.toggle("tool-draw", t !== "select" && t !== "text" && t !== "sticky");
    p.div.classList.toggle("tool-text", t === "text" || t === "sticky");
  });
  if (t !== "select") select(null);
}

function shapeFor(tool, w, h, from) {
  const c = num(S.color);
  switch (tool) {
    case "highlight": return { type: "rect", fill: num(S.hl), highlight: true };
    case "rect": return { type: "rect", stroke: c, width: S.width };
    case "ellipse": return { type: "ellipse", stroke: c, width: S.width };
    case "line": return { type: "line", points: from, color: c, width: S.width };
  }
  return null;
}

function bindPage(div, i) {
  let drag = null;
  const pos = (e) => {
    const r = div.getBoundingClientRect();
    return [(e.clientX - r.left) / sc(), (e.clientY - r.top) / sc()];
  };
  div.addEventListener("pointerdown", (e) => {
    if (e.button !== 0 || !S.ed) return;
    if (e.target.closest(".textedit")) return;
    const [u, v] = pos(e);
    S.cur = i; markCurrent(); updateStatus();
    if (S.tool === "text" || S.tool === "sticky") {
      e.preventDefault();
      openTextEditor(i, u, v, null, S.tool === "sticky" ? stickyShape() : null);
      return;
    }
    if (S.tool === "stamp") {
      e.preventDefault();
      placeStamp(i, u, v);
      return;
    }
    if (S.tool !== "select") {
      e.preventDefault();
      div.setPointerCapture(e.pointerId);
      const r = el("div", { class: "rubber" });
      div.append(r);
      drag = { kind: "new", u0: u, v0: v, u, v, r };
      return;
    }
    const handle = e.target.closest(".h");
    const k = handle && S.sel && S.sel.p === i ? S.sel.o : objAt(i, u, v);
    if (k < 0) { select(null); return; }
    e.preventDefault();
    if (!S.sel || S.sel.p !== i || S.sel.o !== k) select(i, k);
    const o = S.info[i].objects[k];
    div.setPointerCapture(e.pointerId);
    drag = { kind: handle ? "resize" : "move", h: handle && handle.dataset.h, u0: u, v0: v, o: { ...o }, k, moved: false };
  });
  div.addEventListener("mousedown", (e) => {
    // on an annotation (or with a drawing tool) a drag moves or draws: no text selection
    if (e.button !== 0 || !S.ed || e.target.closest(".textedit")) return;
    const [u, v] = pos(e);
    if (S.tool !== "select" || objAt(i, u, v) >= 0 || e.target.closest(".selbox")) e.preventDefault();
  });
  div.addEventListener("pointermove", (e) => {
    if (!drag) return;
    const [u, v] = pos(e);
    drag.u = u; drag.v = v;
    if (drag.kind === "new") {
      const x = Math.min(u, drag.u0), y = Math.min(v, drag.v0);
      placeBox(drag.r, x, y, Math.abs(u - drag.u0), Math.abs(v - drag.v0));
      return;
    }
    const du = u - drag.u0, dv = v - drag.v0;
    if (Math.abs(du) * sc() + Math.abs(dv) * sc() > 2) drag.moved = true;
    const b = $(".selbox", div);
    if (!b) return;
    const g = geom(drag, du, dv, e.shiftKey);
    placeBox(b, g.x, g.y, g.w, g.h);
  });
  const end = (e) => {
    if (!drag) return;
    const d = drag; drag = null;
    if (d.kind === "new") {
      d.r.remove();
      const [u, v] = pos(e);
      const click = Math.abs(u - d.u0) * sc() < 4 && Math.abs(v - d.v0) * sc() < 4;
      let x, y, w, h, shape;
      if (S.tool === "line") {
        const x0 = d.u0, y0 = d.v0, x1 = click ? x0 + 3000 : u, y1 = click ? y0 : v;
        const pad = 100;
        x = Math.min(x0, x1) - pad; y = Math.min(y0, y1) - pad;
        w = Math.abs(x1 - x0) + 2 * pad; h = Math.abs(y1 - y0) + 2 * pad;
        shape = shapeFor("line", w, h, [[x0 - x, y0 - y], [x1 - x, y1 - y]]);
      } else {
        if (click) { x = u; y = v; w = 4000; h = S.tool === "highlight" ? 600 : 2500; }
        else { x = Math.min(u, d.u0); y = Math.min(v, d.v0); w = Math.abs(u - d.u0); h = Math.abs(v - d.v0); }
        shape = shapeFor(S.tool, w, h, null);
      }
      act(() => {
        const k = S.ed.addAnnotation(i, x, y, w, h, JSON.stringify(shape));
        return { pages: [i], sel: { p: i, o: k } };
      });
      return;
    }
    if (!d.moved) return;
    const g = geom(d, d.u - d.u0, d.v - d.v0, e.shiftKey);
    if (d.kind === "move") {
      act(() => { S.ed.moveObject(i, d.k, g.x, g.y); return { pages: [i], sel: { p: i, o: d.k } }; });
    } else {
      const o = S.info[i].objects[d.k];
      if (o.kind === "picture" && o.seeThrough != null) {
        act(() => { S.ed.setPicture(i, d.k, g.x, g.y, g.w, g.h, o.seeThrough); return { pages: [i], sel: { p: i, o: d.k } }; });
        return;
      }
      if (!o.shape) { showSel(); return; }
      const shape = scaleShape(o.shape, o, g);
      act(() => { S.ed.changeAnnotation(i, d.k, g.x, g.y, g.w, g.h, JSON.stringify(shape)); return { pages: [i], sel: { p: i, o: d.k } }; });
    }
  };
  div.addEventListener("pointerup", end);
  div.addEventListener("pointercancel", () => { if (drag && drag.r) drag.r.remove(); drag = null; showSel(); });
  div.addEventListener("dblclick", (e) => {
    const [u, v] = pos(e);
    const k = objAt(i, u, v);
    if (k < 0) return;
    const o = S.info[i].objects[k];
    if (o.shape && (o.shape.type === "text" || o.shape.type === "sticky")) openTextEditor(i, o.x, o.y, k);
    else if (o.shape && o.shape.type === "stamp") select(i, k);
  });
}

function geom(d, du, dv, keep) {
  const o = d.o;
  if (d.kind === "move") return { x: o.x + du, y: o.y + dv, w: o.w, h: o.h };
  let x0 = o.x, y0 = o.y, x1 = o.x + o.w, y1 = o.y + o.h;
  if (d.h.includes("w")) x0 += du; else x1 += du;
  if (d.h.includes("n")) y0 += dv; else y1 += dv;
  if (keep && o.w && o.h) {
    const r = o.w / o.h;
    const w = Math.abs(x1 - x0), h = Math.abs(y1 - y0);
    if (w / h > r) { const nh = w / r; if (d.h.includes("n")) y0 = y1 - nh; else y1 = y0 + nh; }
    else { const nw = h * r; if (d.h.includes("w")) x0 = x1 - nw; else x1 = x0 + nw; }
  }
  return { x: Math.min(x0, x1), y: Math.min(y0, y1), w: Math.max(100, Math.abs(x1 - x0)), h: Math.max(100, Math.abs(y1 - y0)) };
}

function scaleShape(shape, o, g) {
  const s = JSON.parse(JSON.stringify(shape));
  if (s.type === "line" && o.w && o.h) s.points = s.points.map(([x, y]) => [x * g.w / o.w, y * g.h / o.h]);
  return s;
}

// ------------------------------------------------------------------ text annotations
/** A new sticky note (stored as DocuWorks stores one: the note and a text on it). */
function stickyShape() {
  return { type: "sticky", text: "", size: S.size, color: 0x131a2e, background: num(S.sticky) };
}

function openTextEditor(p, x, y, k, base) {
  closeTextEditor(true);
  const o = k == null ? null : S.info[p].objects[k];
  const shape = o ? o.shape : base || { type: "text", text: "", size: S.size, color: num(S.color), bold: false };
  const ta = el("textarea", { class: "textedit", spellcheck: "false" });
  ta.value = shape.text;
  const px = shape.size * 96 / 72 * S.zoom;
  ta.style.fontSize = px + "px";
  ta.style.color = hex(shape.color);
  ta.style.fontWeight = shape.bold ? "700" : "400";
  if (shape.background != null) ta.style.background = hex(shape.background);
  ta.style.left = x * sc() + "px";
  ta.style.top = y * sc() + "px";
  const fit = () => {
    const lines = ta.value.split("\n");
    const cols = Math.max(4, ...lines.map((l) => Array.from(l).reduce((a, c) => a + (isFull(c) ? 1 : 0.55), 0)));
    ta.style.width = (cols + 1) * px + 16 + "px";
    ta.style.height = lines.length * px * 1.25 + 14 + "px";
  };
  ta.addEventListener("input", fit);
  ta.addEventListener("keydown", (e) => {
    if (e.key === "Escape") { e.preventDefault(); closeTextEditor(false); }
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) { e.preventDefault(); closeTextEditor(true); }
    e.stopPropagation();
  });
  ta.addEventListener("blur", () => setTimeout(() => closeTextEditor(true), 0));
  ta._ctx = { p, x, y, k, shape };
  S.pages[p].div.append(ta);
  fit();
  setTimeout(() => ta.focus(), 0);
}

function closeTextEditor(commit) {
  const ta = $(".textedit");
  if (!ta || ta._closing) return;
  ta._closing = true;
  const { p, x, y, k, shape } = ta._ctx;
  const text = ta.value.replace(/\s+$/, "");
  ta.remove();
  if (!commit) return;
  if (k == null) {
    if (!text) return;
    const s = { ...shape, text };
    act(() => ({ pages: [p], sel: { p, o: S.ed.addAnnotation(p, x, y, 0, 0, JSON.stringify(s)) } }));
  } else {
    if (text === shape.text) return;
    if (!text) { act(() => { S.ed.deleteObject(p, k); return { pages: [p], sel: null }; }); return; }
    const s = { ...shape, text };
    act(() => { S.ed.changeAnnotation(p, k, x, y, 0, 0, JSON.stringify(s)); return { pages: [p], sel: { p, o: k } }; });
  }
}

// ------------------------------------------------------------------ edits
function act(fn) {
  try {
    const r = fn() || {};
    if (r.sel !== undefined) S.sel = r.sel;
    refresh(r.pages || "all");
  } catch (e) {
    toast(String(e.message || e));
  }
}

function deleteSelected() {
  if (!S.sel) return;
  const { p, o } = S.sel;
  act(() => { S.ed.deleteObject(p, o); return { pages: [p], sel: null }; });
}

function nudge(dx, dy) {
  if (!S.sel) return;
  const { p, o } = S.sel;
  const ob = S.info[p].objects[o];
  act(() => { S.ed.moveObject(p, o, ob.x + dx, ob.y + dy); return { pages: [p], sel: { p, o } }; });
}

function rotatePage(i, q) { act(() => { S.ed.rotatePage(i, q); return { pages: "all", sel: null }; }); }
async function deletePage(i) {
  if (S.info.length <= 1) { toast("最後の 1 ページは削除できません"); return; }
  const r = await confirmBox("ページの削除", `${i + 1} ページを削除しますか？（元に戻すことができます）`, false);
  if (r !== "ok") return;
  act(() => { S.ed.deletePage(i); return { pages: "all", sel: null }; });
}
function movePage(from, to) { if (from !== to) act(() => { S.ed.movePage(from, to); S.cur = to; return { pages: "all", sel: null }; }); }

function undo() { if (S.ed && S.ed.undo()) { S.sel = null; refresh("all"); } }
function redo() { if (S.ed && S.ed.redo()) { S.sel = null; refresh("all"); } }

// ------------------------------------------------------------------ properties panel
function renderProps() {
  const host = $("#props");
  host.replaceChildren();
  if (!S.ed) { host.append(el("p", { class: "empty", text: "文書を開くと、ここに選んだ注釈の設定が出ます。" })); return; }
  if (!S.sel) {
    const p = S.info[S.cur] || S.info[0];
    const annots = S.info.reduce((a, pg) => a + pg.objects.filter((o) => o.kind !== "page").length, 0);
    host.append(el("div", { class: "infobody" },
      el("div", { class: "kv" },
        el("span", { text: "ページ数" }), el("b", { text: String(S.info.length) }),
        el("span", { text: "用紙" }), el("b", { text: p ? paperName(p.w, p.h) : "-" }),
        el("span", { text: "注釈" }), el("b", { text: annots + " 個" })),
      el("p", { class: "note", html: "注釈を選ぶ: クリック<br>移動: ドラッグ / 矢印キー<br>大きさ: 角をドラッグ（Shift で縦横比を保つ）<br>テキストの修正: ダブルクリック<br>削除: Delete キー" })));
    return;
  }
  const { p, o } = S.sel;
  const ob = S.info[p].objects[o];
  const names = { text: "テキスト", rectangle: "四角形", ellipse: "楕円", line: "直線", marker: "マーカー", picture: "画像", "date stamp": "日付印", "sticky note": "付箋", "received stamp": "受信印", shape: "図形", polygon: "多角形", link: "リンク", signature: "署名", "header/footer": "ページフォーム", object: "オブジェクト", annotation: "注釈" };
  const isSticky = ob.shape && ob.shape.type === "sticky";
  const kind = ob.shape && ob.shape.type === "rect" && ob.shape.highlight ? "蛍光ペン" : isSticky ? "付箋" : names[ob.kind] || ob.kind;
  const box = el("div", { class: "infobody" });
  box.append(el("div", { class: "kv" },
    el("span", { text: "種類" }), el("b", { text: kind }),
    el("span", { text: "位置" }), el("b", { text: `${(ob.x / 100).toFixed(1)} , ${(ob.y / 100).toFixed(1)} mm` }),
    el("span", { text: "大きさ" }), el("b", { text: `${(ob.w / 100).toFixed(1)} × ${(ob.h / 100).toFixed(1)} mm` })));
  const s = ob.shape;
  const change = (patch) => {
    const ns = { ...s, ...patch };
    const auto = ns.type === "text" || ns.type === "sticky";
    act(() => { S.ed.changeAnnotation(p, o, ob.x, ob.y, auto ? 0 : ob.w, auto ? 0 : ob.h, JSON.stringify(ns)); return { pages: [p], sel: { p, o } }; });
  };
  const colorRow = (label, value, list, onpick, allowNone) => {
    const row = el("div", { class: "row" }, el("label", { text: label }));
    if (allowNone) row.append(el("button", { class: "pbtn" + (value == null ? " on" : ""), text: "なし", onclick: () => onpick(null) }));
    for (const c of list) row.append(el("button", { class: "swatch", style: `background:${c};${value != null && hex(value) === c ? "outline:2px solid var(--brand-500);outline-offset:2px" : ""}`, title: c, onclick: () => onpick(num(c)) }));
    const inp = el("input", { type: "color", value: value == null ? "#000000" : hex(value), style: "width:28px;height:24px;border:0;padding:0;background:none" });
    inp.addEventListener("change", () => onpick(num(inp.value)));
    row.append(inp);
    return row;
  };
  if (s && (s.type === "text" || s.type === "sticky")) {
    const ta = el("textarea", { class: "field" });
    ta.value = s.text;
    box.append(el("label", { class: "flabel" }, "文字", ta));
    ta.addEventListener("change", () => { if (ta.value.trim()) change({ text: ta.value }); });
    const size = el("input", { class: "num", type: "number", min: "4", max: "200", step: "1", value: String(s.size) });
    size.addEventListener("change", () => change({ size: Math.max(4, Math.min(200, +size.value || 12)) }));
    const bold = isSticky ? null : el("button", { class: "pbtn" + (s.bold ? " on" : ""), text: "太字", onclick: () => change({ bold: !s.bold }) });
    box.append(el("div", { class: "row" }, el("label", { text: "サイズ" }), size, el("span", { class: "lab", text: "pt" }), bold));
    box.append(colorRow("色", s.color, COLORS, (c) => change({ color: c ?? 0 })));
    if (isSticky) box.append(colorRow("付箋", s.background, STICKIES, (c) => { if (c != null) { S.sticky = hex(c); store.set("sticky2", S.sticky); change({ background: c }); } }));
    else box.append(colorRow("背景", s.background, ["#ffffff", ...HIGHLIGHTS], (c) => change({ background: c }), true));
  } else if (s && s.type === "stamp") {
    const fieldRow = (label, key) => {
      const inp = el("input", { class: "field", value: s[key] });
      inp.addEventListener("change", () => change({ [key]: inp.value }));
      return el("label", { class: "flabel" }, label, inp);
    };
    box.append(fieldRow("上の文字", "top"), fieldRow("日付", "date"), fieldRow("下の文字", "bottom"));
    box.append(el("div", { class: "row" }, el("button", { class: "pbtn", text: "今日の日付にする", onclick: () => change({ date: stampDate(S.stamp.fmt) }) })));
    box.append(colorRow("色", s.color, STAMP_COLORS, (c) => change({ color: c ?? num(STAMP_COLORS[0]) })));
  } else if (s && s.type === "rect" && s.highlight) {
    box.append(colorRow("色", s.fill, HIGHLIGHTS, (c) => change({ fill: c ?? num(S.hl) })));
  } else if (s && (s.type === "rect" || s.type === "ellipse")) {
    box.append(colorRow("線", s.stroke, COLORS, (c) => change({ stroke: c }), true));
    box.append(colorRow("塗り", s.fill, ["#ffffff", ...HIGHLIGHTS], (c) => change({ fill: c }), true));
    box.append(widthRow(s.width, (w) => change({ width: w })));
  } else if (s && s.type === "line") {
    box.append(colorRow("色", s.color, COLORS, (c) => change({ color: c ?? 0 })));
    box.append(widthRow(s.width, (w) => change({ width: w })));
  } else if (ob.kind === "picture" && ob.seeThrough != null) {
    const set = (on) => { if (on !== ob.seeThrough) act(() => { S.ed.setPicture(p, o, ob.x, ob.y, ob.w, ob.h, on); return { pages: [p], sel: { p, o } }; }); };
    box.append(el("div", { class: "row" }, el("label", { text: "透過" }),
      el("button", { class: "pbtn" + (ob.seeThrough ? " on" : ""), text: "白を透かす", onclick: () => set(true) }),
      el("button", { class: "pbtn" + (ob.seeThrough ? "" : " on"), text: "透かさない", onclick: () => set(false) })));
    box.append(el("p", { class: "note", text: ob.seeThrough ? "白い部分から下の文字が見えます。DocuWorks でも同じに見えます（貼り付けた図として保存）。" : "白い部分も含めて不透明です（DocuWorks の画像注釈）。" }));
  } else if (ob.kind !== "page") {
    box.append(el("p", { class: "note", text: ob.kind === "signature" ? "署名です。ここでは動かしたり消したりできません。" : "この注釈は移動と削除ができます（中身の変更は DocuWorks で）。" }));
  }
  if (ob.kind !== "page" && ob.kind !== "signature") box.append(el("div", { class: "row" }, el("button", { class: "pbtn", html: ic("trash-2") + "削除", onclick: deleteSelected })));
  host.append(box);
}

function widthRow(v, on) {
  const row = el("div", { class: "row" }, el("label", { text: "太さ" }));
  for (const w of [1, 2, 3, 5, 8]) row.append(el("button", { class: "pbtn" + (Math.round(v) === w ? " on" : ""), text: w + "pt", onclick: () => on(w) }));
  return row;
}

function paperName(w, h) {
  const a = Math.round(Math.min(w, h) / 100), b = Math.round(Math.max(w, h) / 100);
  const names = { "210x297": "A4", "297x420": "A3", "148x210": "A5", "257x364": "B4", "182x257": "B5", "216x279": "レター" };
  const n = names[`${a}x${b}`] || `${(w / 100).toFixed(0)} × ${(h / 100).toFixed(0)} mm`;
  return n + (w > h ? " 横" : names[`${a}x${b}`] ? " 縦" : "");
}

// ------------------------------------------------------------------ open / save
async function openFile(file) {
  if (!(await askDiscard())) return;
  try {
    const buf = new Uint8Array(await file.arrayBuffer());
    const ed = new XdwDoc(buf);
    if (S.ed) S.ed.free();
    S.ed = ed;
    S.name = file.name.replace(/\.(xdw|xbd)$/i, "");
    S.ext = ed.binder() !== "null" ? ".xbd" : ".xdw";
    if (ed.isSigned()) setTimeout(() => toast("この文書には署名があります。編集して保存すると、署名は無効になります。", 7000), 400);
    S.sel = null;
    S.cur = 0;
    S.info = JSON.parse(ed.pages());
    $("#welcome").hidden = true;
    $("#pages").hidden = false;
    fitIfNarrow();
    buildPages();
    setTool("select");
    renderProps();
    updateChrome();
    $("#scroller").scrollTop = 0;
  } catch (e) {
    console.error(e);
    toast("開けませんでした: " + (e.message || e));
  }
}

function download(bytes, name, type) {
  const url = URL.createObjectURL(new Blob([bytes], { type }));
  const a = el("a", { href: url, download: name });
  document.body.append(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 4000);
}

function saveXdw(name) {
  if (!S.ed) return;
  try {
    const b = S.ed.save();
    const binder = S.ext === ".xbd";
    download(b, (name || S.name) + S.ext, binder ? "application/vnd.fujixerox.docuworks.binder" : "application/vnd.fujixerox.docuworks");
    toast(binder ? "保存しました（DocuWorks バインダー）" : "保存しました（DocuWorks 文書）");
    updateChrome();
  } catch (e) {
    toast("保存できませんでした: " + (e.message || e));
  }
}

async function savePdf(name) {
  toast("PDF を作っています…", 60000);
  try {
    const parts = [], lens = [], dims = [];
    for (let i = 0; i < S.info.length; i++) {
      const c = document.createElement("canvas");
      const k = 150 / 2540; // 150 dpi
      const d = JSON.parse(S.ed.render(i));
      const imgs = await loadImages(i, d);
      c.width = Math.round(d.w * k); c.height = Math.round(d.h * k);
      paint(c.getContext("2d"), d, imgs, k);
      S.ed.forget(i);
      const blob = await new Promise((r) => c.toBlob(r, "image/jpeg", 0.88));
      const b = new Uint8Array(await blob.arrayBuffer());
      parts.push(b); lens.push(b.length); dims.push(c.width, c.height);
    }
    const all = new Uint8Array(lens.reduce((a, b) => a + b, 0));
    let at = 0;
    for (const p of parts) { all.set(p, at); at += p.length; }
    const pdf = S.ed.toPdf(all, new Uint32Array(lens), new Uint32Array(dims), name || S.name);
    download(pdf, (name || S.name) + ".pdf", "application/pdf");
    toast("PDF を保存しました");
  } catch (e) {
    toast("PDF を作れませんでした: " + (e.message || e));
  }
}

// ------------------------------------------------------------------ find
/** Text nodes of a page's text layer, with where each starts in the joined text. */
function layerText(layer) {
  const nodes = [];
  let text = "";
  for (const span of layer.querySelectorAll("span")) {
    const t = span.firstChild;
    if (!t) continue;
    nodes.push({ t, at: text.length });
    text += t.data;
  }
  return { nodes, text };
}

const fold = (s) => s.normalize("NFKC").toLowerCase();

/** Every match of the query on page i: [start, end] in the layer text. */
async function pageMatches(i, q) {
  const pg = S.pages[i];
  if (!pg) return [];
  // pages not drawn yet: ask the engine first, draw only when the text is there
  if (!pg.drawn && !fold(S.ed.text(i).replace(/\s+/g, "")).includes(fold(q).replace(/\s+/g, ""))) return [];
  if (!pg.drawn) await drawPage(i);
  const layer = $(".textlayer", pg.div);
  if (!layer) return [];
  const { text } = layerText(layer);
  const hay = fold(text), needle = fold(q);
  const out = [];
  for (let k = hay.indexOf(needle); k >= 0 && needle; k = hay.indexOf(needle, k + 1)) out.push([k, k + needle.length]);
  return out;
}

function selectMatch(i, [a, b]) {
  const layer = $(".textlayer", S.pages[i].div);
  const { nodes } = layerText(layer);
  const at = (pos) => {
    let n = nodes[0];
    for (const x of nodes) if (x.at <= pos) n = x;
    return [n.t, Math.min(pos - n.at, n.t.data.length)];
  };
  const r = document.createRange();
  r.setStart(...at(a));
  r.setEnd(...at(b));
  const sel = getSelection(); sel.removeAllRanges(); sel.addRange(r);
  const box = r.getBoundingClientRect(), sc = $("#scroller"), sb = sc.getBoundingClientRect();
  if (box.top < sb.top + 40 || box.bottom > sb.bottom - 40) sc.scrollBy({ top: box.top - sb.top - sb.height / 3 });
  S.cur = i; markCurrent(); updateStatus();
}

/** Go to the next (dir 1) or previous (-1) match, page by page from the current one. */
async function findNext(dir) {
  const q = $("#find-q").value;
  if (!S.ed || !q.trim()) { $("#find-n").textContent = ""; return; }
  const f = S.find && S.find.q === q ? S.find : (S.find = { q, p: S.cur, k: -1 });
  const n = S.info.length;
  for (let step = 0; step <= n; step++) {
    const i = ((f.p + dir * step) % n + n) % n;
    const m = await pageMatches(i, q);
    if (!m.length) continue;
    let k;
    if (step === 0 && f.k >= 0) { k = f.k + dir; if (k < 0 || k >= m.length) continue; }
    else k = dir > 0 ? 0 : m.length - 1;
    f.p = i; f.k = k;
    selectMatch(i, m[k]);
    $("#find-n").textContent = `${k + 1} / ${m.length}（${i + 1}ページ）`;
    return;
  }
  $("#find-n").textContent = "見つかりません";
}

function openFind() {
  if (!S.ed) return;
  $("#findbar").hidden = false;
  const sel = getSelection().toString().trim();
  if (sel && sel.length < 60) $("#find-q").value = sel;
  $("#find-q").select();
  $("#find-q").focus();
}

function closeFind() {
  $("#findbar").hidden = true;
  S.find = null;
  $("#find-n").textContent = "";
}

function saveText(name) {
  const t = S.info.map((_, i) => S.ed.text(i)).join("\n\f\n");
  download(new TextEncoder().encode(t), (name || S.name) + ".txt", "text/plain");
}

async function saveAs() {
  if (!S.ed) return;
  const d = $("#dlg-save");
  $("#save-name").value = S.name;
  $("#fmt-xdw-label").textContent = S.ext === ".xbd" ? "DocuWorks バインダー (.xbd)" : "DocuWorks 文書 (.xdw)";
  d.showModal();
  const r = await new Promise((res) => d.addEventListener("close", () => res(d.returnValue), { once: true }));
  if (r !== "ok") return;
  const name = $("#save-name").value.trim() || S.name;
  const fmt = $("input[name=fmt]:checked", d).value;
  if (fmt === "xdw") saveXdw(name);
  else if (fmt === "pdf") savePdf(name);
  else saveText(name);
}

async function askDiscard() {
  if (!S.ed || !S.ed.dirty()) return true;
  const r = await confirmBox("保存していない変更", "変更が保存されていません。保存しますか？", true);
  if (r === "ok") { saveXdw(); return true; }
  return r === "discard";
}

function confirmBox(title, text, withDiscard) {
  const d = $("#dlg-confirm");
  $("#cf-title").textContent = title;
  $("#cf-text").textContent = text;
  $("#cf-discard").hidden = !withDiscard;
  $("#cf-ok").textContent = withDiscard ? "保存" : "OK";
  d.showModal();
  return new Promise((res) => d.addEventListener("close", () => res(d.returnValue), { once: true }));
}

// ------------------------------------------------------------------ date stamp
async function placeStamp(p, u, v) {
  const r = await stampDialog();
  if (!r) return;
  const size = S.stamp.size || 1800;
  const pg = S.info[p];
  const x = Math.max(0, Math.min(pg.w - size, u - size / 2)), y = Math.max(0, Math.min(pg.h - size, v - size / 2));
  act(() => ({ pages: [p], sel: { p, o: S.ed.addAnnotation(p, x, y, size, size, JSON.stringify({ type: "stamp", ...r })) } }));
}

function stampDialog() {
  const d = $("#dlg-stamp");
  const st = S.stamp;
  $("#stamp-top").value = st.top || "";
  $("#stamp-bottom").value = st.bottom || "";
  $("#stamp-date").value = stampDate(st.fmt);
  $$("input[name=stamp-fmt]", d).forEach((r) => { r.checked = r.value === (st.fmt || "yy"); });
  $$("input[name=stamp-size]", d).forEach((r) => { r.checked = +r.value === (st.size || 1800); });
  let color = st.color || STAMP_COLORS[0];
  const sw = $("#stamp-colors");
  const paint = () => sw.replaceChildren(...STAMP_COLORS.map((c) => el("button", { type: "button", class: "swatch", style: `background:${c};${c === color ? "outline:2px solid var(--brand-500);outline-offset:2px" : ""}`, onclick: () => { color = c; paint(); } })));
  paint();
  d.showModal();
  return new Promise((res) => d.addEventListener("close", () => {
    if (d.returnValue !== "ok") return res(null);
    const fmt = ($("input[name=stamp-fmt]:checked", d) || {}).value || "yy";
    const size = +(($("input[name=stamp-size]:checked", d) || {}).value || 1800);
    S.stamp = { top: $("#stamp-top").value.trim(), bottom: $("#stamp-bottom").value.trim(), fmt, size, color };
    store.set("stamp", S.stamp);
    res({ top: S.stamp.top, date: $("#stamp-date").value.trim() || stampDate(fmt), bottom: S.stamp.bottom, color: num(color) });
  }, { once: true }));
}

// ------------------------------------------------------------------ pictures and pages from files
function pickFile(accept, multiple) {
  return new Promise((res) => {
    const inp = el("input", { type: "file", accept, hidden: true });
    if (multiple) inp.multiple = true;
    inp.addEventListener("change", () => { res(Array.from(inp.files || [])); inp.remove(); });
    document.body.append(inp);
    inp.click();
  });
}

/** Draw a picture file on a canvas (at most `max` pixels on its long side, on white).
 * `c.clear` tells whether the picture had transparent parts. */
async function fileCanvas(file, max) {
  const bmp = await createImageBitmap(file);
  const k = Math.min(1, max / Math.max(bmp.width, bmp.height));
  const c = document.createElement("canvas");
  c.width = Math.max(1, Math.round(bmp.width * k)); c.height = Math.max(1, Math.round(bmp.height * k));
  const g = c.getContext("2d");
  g.drawImage(bmp, 0, 0, c.width, c.height);
  const a = g.getImageData(0, 0, c.width, c.height).data;
  c.clear = false;
  for (let i = 3; i < a.length; i += 4) if (a[i] < 250) { c.clear = true; break; }
  g.globalCompositeOperation = "destination-over";
  g.fillStyle = "#fff"; g.fillRect(0, 0, c.width, c.height);
  g.globalCompositeOperation = "source-over";
  bmp.close && bmp.close();
  return c;
}

const jpegOf = async (c, q = 0.9) => new Uint8Array(await (await new Promise((r) => c.toBlob(r, "image/jpeg", q))).arrayBuffer());

/** The small picture DocuWorks keeps for a page: about 104 px wide. */
function thumbOf(c, pw, ph) {
  const tw = 104, th = Math.max(1, Math.round(104 * ph / pw));
  const t = document.createElement("canvas");
  t.width = tw; t.height = th;
  const g = t.getContext("2d");
  g.fillStyle = "#fff"; g.fillRect(0, 0, tw, th);
  const k = Math.min(tw / c.width, th / c.height);
  g.drawImage(c, (tw - c.width * k) / 2, (th - c.height * k) / 2, c.width * k, c.height * k);
  return { rgba: new Uint8Array(g.getImageData(0, 0, tw, th).data.buffer), w: tw, h: th };
}

async function pickPicture() {
  const [f] = await pickFile("image/*");
  if (!f) return;
  try {
    const c = await fileCanvas(f, 1600);
    const p = S.cur, pg = S.info[p];
    let w = Math.min(pg.w * 0.5, c.width * 2540 / 150), h = w * c.height / c.width;
    if (h > pg.h * 0.6) { h = pg.h * 0.6; w = h * c.width / c.height; }
    const rgba = new Uint8Array(c.getContext("2d").getImageData(0, 0, c.width, c.height).data.buffer);
    // a picture with transparent parts stays see-through (white shows the page)
    const see = c.clear;
    act(() => ({ pages: [p], sel: { p, o: S.ed.addPicture(p, (pg.w - w) / 2, (pg.h - h) / 2, w, h, rgba, c.width, c.height, see) } }));
    toast(see ? "画像を貼りました。白い部分は下の文字が透けて見えます。" : "画像を貼りました。右の「透過」で白い部分を透かせます。", 4000);
  } catch (e) {
    toast("画像を読めませんでした: " + (e.message || e));
  }
}

function insertBlank(at) {
  const ref = S.info[Math.min(at, S.info.length) - 1] || S.info[0];
  act(() => { S.ed.insertBlankPage(at, ref.w, ref.h); S.cur = at; return { pages: "all", sel: null }; });
  setTimeout(() => gotoPage(at), 50);
}

async function pickInsert(at) {
  const files = await pickFile(".xdw,.xbd,.pdf,image/*", true);
  if (!files.length) return;
  await insertFiles(files, at);
}

/** Insert pages from files (DocuWorks, PDF, pictures) at position `at`. */
async function insertFiles(files, at) {
  const start = at;
  let total = 0;
  const before = S.info.length;
  for (const f of files) {
    try {
      const n = await insertOne(f, at);
      at += n; total += n;
    } catch (e) {
      console.error(e);
      toast(`${f.name}: ${e.message || e}`, 5000);
    }
  }
  if (!total) return;
  S.sel = null;
  refresh("all");
  S.cur = Math.min(start, S.info.length - 1);
  setTimeout(() => gotoPage(S.cur), 50);
  toast(`${S.info.length - before} ページを挿入しました`);
}

async function insertOne(f, at) {
  const name = f.name.toLowerCase();
  if (/\.(xdw|xbd)$/.test(name)) {
    const bytes = new Uint8Array(await f.arrayBuffer());
    const tmp = new XdwDoc(bytes);
    const n = tmp.pageCount();
    tmp.free();
    let which = "";
    if (n > 1) {
      const r = await rangeDialog(f.name, n);
      if (r === null) return 0;
      which = r;
    }
    return S.ed.insertPagesFrom(at, bytes, which);
  }
  if (/\.pdf$/.test(name) || f.type === "application/pdf") return insertPdf(f, at);
  if (/^image\//.test(f.type) || /\.(jpe?g|png|gif|webp|bmp)$/.test(name)) {
    const c = await fileCanvas(f, 4000);
    const [pw, ph] = c.width > c.height ? [29700, 21000] : [21000, 29700];
    const t = thumbOf(c, pw, ph);
    S.ed.insertImagePage(at, pw, ph, await jpegOf(c), c.width, c.height, t.rgba, t.w, t.h);
    return 1;
  }
  throw new Error("この種類のファイルは挿入できません");
}

/** Ask which pages to take: "" = all, or a JSON list of page numbers from 0. */
function rangeDialog(fname, n) {
  const d = $("#dlg-range");
  $("#range-file").textContent = `${fname}（${n} ページ）`;
  $("#range-text").value = `1-${n}`;
  $("input[name=range][value=all]", d).checked = true;
  d.showModal();
  return new Promise((res) => d.addEventListener("close", () => {
    if (d.returnValue !== "ok") return res(null);
    if ($("input[name=range]:checked", d).value === "all") return res("");
    const pages = [];
    for (const part of $("#range-text").value.split(/[,、\s]+/)) {
      const m = part.match(/^(\d+)(?:[-~〜](\d+))?$/);
      if (!m) continue;
      const a = +m[1], b = m[2] ? +m[2] : a;
      for (let k = Math.min(a, b); k <= Math.max(a, b); k++) if (k >= 1 && k <= n) pages.push(k - 1);
    }
    res(pages.length ? JSON.stringify(pages) : null);
  }, { once: true }));
}

/** Paper sizes within 0.6 mm of a standard one become exactly that size. */
function snapPaper(w, h) {
  const std = [[21000, 29700], [29700, 42000], [14800, 21000], [25700, 36400], [18200, 25700], [21590, 27940], [21590, 35560]];
  for (const [a, b] of std) for (const [x, y] of [[a, b], [b, a]]) if (Math.abs(w - x) < 60 && Math.abs(h - y) < 60) return [x, y];
  return [Math.round(w), Math.round(h)];
}

let PDFJS = null;
async function pdfjs() {
  if (PDFJS) return PDFJS;
  const base = "https://cdnjs.cloudflare.com/ajax/libs/pdf.js/3.11.174/";
  await new Promise((res, rej) => {
    const sc = document.createElement("script");
    sc.src = base + "pdf.min.js";
    sc.onload = res;
    sc.onerror = () => rej(new Error("PDF を読む部品を取得できませんでした（インターネット接続が必要です）"));
    document.head.append(sc);
  });
  const lib = window.pdfjsLib;
  try {
    // a worker from another site is not allowed: run it from a local copy
    const code = await (await fetch(base + "pdf.worker.min.js")).text();
    lib.GlobalWorkerOptions.workerSrc = URL.createObjectURL(new Blob([code], { type: "text/javascript" }));
  } catch {
    lib.GlobalWorkerOptions.workerSrc = base + "pdf.worker.min.js";
  }
  return (PDFJS = lib);
}

/** PDF pages become picture pages (200 dpi). */
async function insertPdf(f, at) {
  toast("PDF を読み込んでいます…", 60000);
  const lib = await pdfjs();
  const pdf = await lib.getDocument({ data: new Uint8Array(await f.arrayBuffer()) }).promise;
  const n = pdf.numPages;
  let which = [0];
  if (n > 1) {
    toast("", 1);
    const r = await rangeDialog(f.name, n);
    if (r === null) return 0;
    which = r ? JSON.parse(r) : [...Array(n).keys()];
  }
  let done = 0;
  for (const k of which) {
    toast(`PDF を挿入しています… ${done + 1} / ${which.length}`, 60000);
    const page = await pdf.getPage(k + 1);
    const v1 = page.getViewport({ scale: 1 });
    const [pw, ph] = snapPaper(v1.width * 2540 / 72, v1.height * 2540 / 72);
    const vp = page.getViewport({ scale: 200 / 72 });
    const c = document.createElement("canvas");
    c.width = Math.round(vp.width); c.height = Math.round(vp.height);
    const g = c.getContext("2d");
    g.fillStyle = "#fff"; g.fillRect(0, 0, c.width, c.height);
    await page.render({ canvasContext: g, viewport: vp }).promise;
    const t = thumbOf(c, pw, ph);
    S.ed.insertImagePage(at + done, pw, ph, await jpegOf(c, 0.88), c.width, c.height, t.rgba, t.w, t.h);
    done++;
  }
  pdf.destroy && pdf.destroy();
  return done;
}

// ------------------------------------------------------------------ binder documents
function promptBox(title, label, value) {
  const d = $("#dlg-prompt");
  $("#pr-title").textContent = title;
  $("#pr-label").firstChild.textContent = label;
  $("#pr-input").value = value || "";
  d.showModal();
  setTimeout(() => { $("#pr-input").focus(); $("#pr-input").select(); }, 0);
  return new Promise((res) => d.addEventListener("close", () => res(d.returnValue === "ok" ? $("#pr-input").value.trim() : null), { once: true }));
}

async function renameDoc(k) {
  if (!S.binder || k == null) return;
  const d = S.binder.docs[k];
  const name = await promptBox("文書名の変更", "文書名", d.name);
  if (!name || name === d.name) return;
  act(() => { S.ed.renameBinderDoc(k, name); return { pages: "all" }; });
}

function moveDoc(k, to) {
  if (!S.binder || k == null || to < 0 || to >= S.binder.docs.length) return;
  act(() => { S.ed.moveBinderDoc(k, to); return { pages: "all", sel: null }; });
  setTimeout(() => gotoPage(S.binder.docs[to].first_page), 50);
}

async function deleteDoc(k) {
  if (!S.binder || k == null) return;
  const d = S.binder.docs[k];
  const r = await confirmBox("文書を外す", `「${d.name}」（${d.pages} ページ）をバインダーから外しますか？（元に戻すことができます）`, false);
  if (r !== "ok") return;
  act(() => { S.ed.deleteBinderDoc(k); return { pages: "all", sel: null }; });
}

async function pickBinderAdd() {
  if (!S.binder) return;
  const files = await pickFile(".xdw,.xbd", true);
  let at = docOf(S.cur) + 1, added = 0;
  for (const f of files) {
    try {
      const n = S.ed.addBinderDocs(at, new Uint8Array(await f.arrayBuffer()), f.name.replace(/\.(xdw|xbd)$/i, ""));
      at += n; added += n;
    } catch (e) {
      toast(`${f.name}: ${e.message || e}`, 5000);
    }
  }
  if (added) { refresh("all"); toast(`${added} 文書を追加しました`); }
}

// ------------------------------------------------------------------ chrome
function buildMenus() {
  const menus = [
    ["ファイル", "F", [
      ["開く…", MOD + "O", () => $("#fileinput").click()],
      ["上書き保存 (.xdw)", MOD + "S", () => saveXdw()],
      ["名前を付けて保存…", MOD + "Shift+S", saveAs],
      null,
      ["PDF として保存", "", () => savePdf()],
      ["テキストとして保存", "", () => saveText()],
      null,
      ["印刷…", MOD + "P", printDoc],
    ]],
    ["編集", "E", [
      ["元に戻す", MOD + "Z", undo, () => S.ed && S.ed.canUndo()],
      ["やり直し", MOD + "Y", redo, () => S.ed && S.ed.canRedo()],
      null,
      ["削除", "Delete", deleteSelected, () => !!S.sel],
      null,
      ["文書内を検索…", MOD + "F", () => openFind()],
    ]],
    ["表示", "V", [
      ["拡大", MOD + "+", () => rezoom(S.zoom * 1.2)],
      ["縮小", MOD + "-", () => rezoom(S.zoom / 1.2)],
      ["幅に合わせる", "", fitWidth],
      ["100%", "", () => rezoom(1)],
      null,
      ["ページ一覧", "", () => $("#main").classList.toggle("no-jump")],
      ["プロパティ", "", () => $("#main").classList.toggle("no-palette")],
    ]],
    ["ページ", "P", [
      ["白紙ページを挿入", "", () => S.ed && insertBlank(S.cur + 1)],
      ["ファイルからページを挿入…", "", () => S.ed && pickInsert(S.cur + 1)],
      null,
      ["左へ 90° 回転", "", () => S.ed && rotatePage(S.cur, 3)],
      ["右へ 90° 回転", "", () => S.ed && rotatePage(S.cur, 1)],
      ["180° 回転", "", () => S.ed && rotatePage(S.cur, 2)],
      null,
      ["前へ移動", "", () => S.ed && S.cur > 0 && movePage(S.cur, S.cur - 1)],
      ["後ろへ移動", "", () => S.ed && S.cur < S.info.length - 1 && movePage(S.cur, S.cur + 1)],
      null,
      ["ページを削除", "", () => S.ed && deletePage(S.cur)],
    ]],
    ["アノテーション", "A", [
      ["選択", "Esc", () => setTool("select")],
      ["テキスト", "T", () => setTool("text")],
      ["蛍光ペン", "H", () => setTool("highlight")],
      ["四角形", "R", () => setTool("rect")],
      ["楕円", "E", () => setTool("ellipse")],
      ["直線", "L", () => setTool("line")],
      null,
      ["付箋", "S", () => setTool("sticky")],
      ["日付印", "D", () => setTool("stamp")],
      ["画像を貼る…", "", () => S.ed && pickPicture()],
    ]],
    ["バインダー", "B", [
      ["文書を追加…", "", () => pickBinderAdd(), () => !!S.binder],
      ["文書名を変更…", "", () => renameDoc(docOf(S.cur)), () => !!S.binder],
      ["文書を上へ", "", () => moveDoc(docOf(S.cur), docOf(S.cur) - 1), () => !!S.binder && docOf(S.cur) > 0],
      ["文書を下へ", "", () => moveDoc(docOf(S.cur), docOf(S.cur) + 1), () => !!S.binder && docOf(S.cur) < S.binder.docs.length - 1],
      null,
      ["文書をバインダーから外す", "", () => deleteDoc(docOf(S.cur)), () => !!S.binder && S.binder.docs.length > 1],
    ]],
  ];
  const bar = $("#menubar");
  for (const [name, key, items] of menus) {
    const m = el("div", { class: "menu" });
    if (name === "バインダー") { m.id = "menu-binder"; m.hidden = true; }
    const b = el("button", { html: `${name}<span class="k">(${key})</span>` });
    const drop = el("div", { class: "drop" });
    const open = () => {
      $$(".menu.open").forEach((x) => x !== m && x.classList.remove("open"));
      drop.replaceChildren();
      for (const it of items) {
        if (!it) { drop.append(el("hr")); continue; }
        const [lbl, sc_, fn, en] = it;
        const btn = el("button", { html: `<span class="lbl">${lbl}</span><span class="sc">${sc_}</span>`, onclick: () => { m.classList.remove("open"); fn(); } });
        if (en && !en()) btn.disabled = true;
        if (!S.ed && !/開く/.test(lbl)) btn.disabled = true;
        drop.append(btn);
      }
      const r = b.getBoundingClientRect();
      drop.style.left = r.left + "px";
      drop.style.top = r.bottom + 4 + "px";
      m.classList.toggle("open");
    };
    b.addEventListener("click", open);
    m.append(b, drop);
    bar.append(m);
  }
  document.addEventListener("click", (e) => { if (!e.target.closest(".menu")) $$(".menu.open").forEach((x) => x.classList.remove("open")); });
  $("#actions").append(
    el("button", { class: "tbtn", html: ic("folder-open") + "<span class=hide-narrow>開く</span>", onclick: () => $("#fileinput").click() }),
    el("span", { class: "split" },
      el("button", { class: "main", html: ic("save") + "<span>保存</span>", title: "DocuWorks 文書として保存 (" + MOD + "S)", onclick: () => saveXdw() }),
      el("button", { class: "more", html: ic("chevron-down"), title: "形式を選んで保存", onclick: saveAs })));
}

function buildToolbar() {
  const bar = $("#toolbar");
  const tool = (t, icon, label, key) => el("button", { class: "toolbtn", "data-tool": t, title: `${label} (${key})`, html: ic(icon) + `<span class=hide-narrow>${label}</span>`, onclick: () => setTool(t) });
  const sw = el("span", { class: "swatches", title: "色" });
  for (const c of COLORS) sw.append(el("button", { style: `background:${c}`, "data-c": c, onclick: () => { S.color = c; store.set("color", c); updateSwatches(); } }));
  const hs = el("span", { class: "swatches", title: "蛍光ペンの色" });
  for (const c of HIGHLIGHTS) hs.append(el("button", { style: `background:${c}`, "data-h": c, onclick: () => { S.hl = c; store.set("hl", c); updateSwatches(); setTool("highlight"); } }));
  const size = el("select", { title: "テキストの大きさ" });
  for (const v of [8, 9, 10, 10.5, 11, 12, 14, 16, 18, 20, 24, 28, 36, 48]) size.append(el("option", { value: v, text: v, selected: v === S.size }));
  size.addEventListener("change", () => { S.size = +size.value; store.set("size", S.size); });
  const width = el("select", { title: "線の太さ" });
  for (const v of [1, 2, 3, 5, 8]) width.append(el("option", { value: v, text: v + " pt", selected: v === S.width }));
  width.addEventListener("change", () => { S.width = +width.value; store.set("width", S.width); });
  bar.append(
    tool("select", "mouse-pointer-2", "選択", "Esc"),
    tool("text", "type", "テキスト", "T"),
    tool("sticky", "sticky-note", "付箋", "S"),
    tool("stamp", "stamp", "日付印", "D"),
    tool("highlight", "highlighter", "蛍光ペン", "H"),
    tool("rect", "square", "四角形", "R"),
    tool("ellipse", "circle", "楕円", "E"),
    tool("line", "line", "直線", "L"),
    el("span", { class: "vsep" }), sw, el("span", { class: "vsep" }), hs, el("span", { class: "vsep" }),
    el("span", { class: "fill" }, size, el("span", { class: "unit", text: "pt" })),
    el("span", { class: "fill" }, width),
    el("span", { class: "vsep" }),
    el("button", { class: "ib", id: "tb-undo", title: "元に戻す (" + MOD + "Z)", html: ic("undo-2"), onclick: undo }),
    el("button", { class: "ib", id: "tb-redo", title: "やり直し (" + MOD + "Y)", html: ic("redo-2"), onclick: redo }),
    el("span", { class: "vsep" }),
    el("button", { class: "ib", title: "ページを左へ 90° 回転", html: ic("rotate-ccw"), onclick: () => S.ed && rotatePage(S.cur, 3) }),
    el("button", { class: "ib", title: "ページを右へ 90° 回転", html: ic("rotate-cw"), onclick: () => S.ed && rotatePage(S.cur, 1) }),
    el("button", { class: "ib", title: "ページを削除", html: ic("trash-2"), onclick: () => S.ed && deletePage(S.cur) }));
  setTool("select");
  updateSwatches();
}

function updateSwatches() {
  $$("#toolbar [data-c]").forEach((b) => b.classList.toggle("on", b.dataset.c === S.color));
  $$("#toolbar [data-h]").forEach((b) => b.classList.toggle("on", b.dataset.h === S.hl));
}

function updateChrome() {
  const dirty = !!(S.ed && S.ed.dirty());
  $("#topbar").classList.toggle("modified", dirty);
  $("#docname").textContent = S.ed ? S.name + S.ext : "文書を開いてください";
  document.title = (S.ed ? (dirty ? "● " : "") + S.name + " | " : "") + "XDW エディタ | EZPZ File";
  const u = $("#tb-undo"), r = $("#tb-redo");
  if (u) u.disabled = !(S.ed && S.ed.canUndo());
  if (r) r.disabled = !(S.ed && S.ed.canRedo());
  updateStatus();
}

function updateStatus() {
  $("#st-page").textContent = S.ed ? String(S.cur + 1) : "-";
  $("#st-pages").textContent = S.ed ? String(S.info.length) : "-";
  const p = S.info[S.cur];
  $("#st-size").textContent = p ? paperName(p.w, p.h) : "";
  $("#st-zv").textContent = Math.round(S.zoom * 100) + "%";
}

function rezoom(z) {
  S.zoom = Math.max(0.25, Math.min(4, z));
  store.set("zoom", S.zoom);
  if (S.ed) {
    const s = $("#scroller");
    const rel = s.scrollTop / Math.max(1, s.scrollHeight);
    S.pages.forEach((pg, i) => { sizePage(pg.div, S.info[i]); pg.drawn = false; });
    s.scrollTop = rel * s.scrollHeight;
    S.pages.forEach((pg, i) => {
      const r = pg.div.getBoundingClientRect(), v = s.getBoundingClientRect();
      if (r.bottom > v.top - 600 && r.top < v.bottom + 600) drawPage(i, true);
    });
    showSel();
  }
  updateStatus();
}

function fitWidth() {
  const s = $("#scroller");
  const p = S.info[S.cur] || { w: 21000 };
  rezoom((s.clientWidth - 80) / (p.w * U));
}

function fitIfNarrow() {
  const s = $("#scroller");
  const p = S.info[0];
  if (p && s.clientWidth && s.clientWidth < p.w * U * S.zoom + 80) fitWidth();
}

async function printDoc() {
  if (!S.ed) return;
  const area = $("#printarea");
  area.replaceChildren();
  toast("印刷の準備をしています…", 60000);
  for (let i = 0; i < S.info.length; i++) {
    const c = document.createElement("canvas");
    await renderTo(i, c, 200 / 2540 / (window.devicePixelRatio || 1));
    S.ed.forget(i);
    const img = el("img", { src: c.toDataURL("image/jpeg", 0.92) });
    area.append(img);
  }
  toast("", 1);
  window.print();
}

function toast(msg, ms = 2600) {
  const t = $("#toast");
  t.textContent = msg;
  t.classList.toggle("show", !!msg);
  clearTimeout(toast._t);
  toast._t = setTimeout(() => t.classList.remove("show"), ms);
}

function bindChrome() {
  $("#fileinput").addEventListener("change", (e) => { const f = e.target.files[0]; if (f) openFile(f); e.target.value = ""; });
  $("#find-prev").innerHTML = ic("chevron-up");
  $("#find-next").innerHTML = ic("chevron-down");
  $("#find-close").innerHTML = ic("x");
  $("#find-prev").addEventListener("click", () => findNext(-1));
  $("#find-next").addEventListener("click", () => findNext(1));
  $("#find-close").addEventListener("click", closeFind);
  $("#find-q").addEventListener("keydown", (e) => {
    if (e.key === "Enter") { e.preventDefault(); findNext(e.shiftKey ? -1 : 1); }
    else if (e.key === "Escape") { e.preventDefault(); closeFind(); }
  });
  $("#find-q").addEventListener("input", () => { S.find = null; $("#find-n").textContent = ""; });
  $("#welcome-open").addEventListener("click", () => $("#fileinput").click());
  $("#binder-add").innerHTML = ic("plus");
  $("#binder-add").addEventListener("click", pickBinderAdd);
  $("#st-zin").innerHTML = ic("plus"); $("#st-zout").innerHTML = ic("minus"); $("#st-fit").innerHTML = ic("maximize");
  $("#st-zin").addEventListener("click", () => rezoom(S.zoom * 1.2));
  $("#st-zout").addEventListener("click", () => rezoom(S.zoom / 1.2));
  $("#st-fit").addEventListener("click", fitWidth);
  $("#scroller").addEventListener("scroll", () => trackCurrent(), { passive: true });
  const dz = $("#dropzone");
  let depth = 0;
  window.addEventListener("dragenter", (e) => { if (e.dataTransfer.types.includes("Files")) { depth++; dz.classList.add("show"); } });
  window.addEventListener("dragleave", () => { depth = Math.max(0, depth - 1); if (!depth) dz.classList.remove("show"); });
  window.addEventListener("dragover", (e) => { if (e.dataTransfer.types.includes("Files")) e.preventDefault(); });
  window.addEventListener("drop", (e) => {
    depth = 0; dz.classList.remove("show");
    const fs = Array.from((e.dataTransfer.files) || []);
    const f = fs[0];
    if (!f) return;
    e.preventDefault();
    // pictures and PDFs dropped on an open document become new pages
    if (S.ed && !/\.(xdw|xbd)$/i.test(f.name)) insertFiles(fs, S.cur + 1);
    else openFile(f);
  });
  window.addEventListener("beforeunload", (e) => { if (S.ed && S.ed.dirty()) { e.preventDefault(); e.returnValue = ""; } });
  document.addEventListener("keydown", (e) => {
    if (e.target.closest("input,textarea,select")) return;
    const mod = e.ctrlKey || e.metaKey;
    const k = e.key.toLowerCase();
    if (mod && k === "o") { e.preventDefault(); $("#fileinput").click(); }
    else if (mod && k === "s") { e.preventDefault(); e.shiftKey ? saveAs() : saveXdw(); }
    else if (mod && k === "p") { e.preventDefault(); printDoc(); }
    else if (mod && k === "f" && S.ed) { e.preventDefault(); openFind(); }
    else if (mod && k === "a" && S.ed) {
      // the text of the current page
      const layer = S.pages[S.cur] && $(".textlayer", S.pages[S.cur].div);
      if (layer && layer.textContent) {
        e.preventDefault();
        const r = document.createRange();
        r.selectNodeContents(layer);
        const sel = getSelection(); sel.removeAllRanges(); sel.addRange(r);
      }
    }
    else if (mod && k === "z") { e.preventDefault(); e.shiftKey ? redo() : undo(); }
    else if (mod && k === "y") { e.preventDefault(); redo(); }
    else if (mod && (k === "+" || k === "=")) { e.preventDefault(); rezoom(S.zoom * 1.2); }
    else if (mod && k === "-") { e.preventDefault(); rezoom(S.zoom / 1.2); }
    else if (!mod && (e.key === "Delete" || e.key === "Backspace") && S.sel) { e.preventDefault(); deleteSelected(); }
    else if (e.key === "Escape") { setTool("select"); select(null); }
    else if (!mod && S.sel && e.key.startsWith("Arrow")) {
      e.preventDefault();
      const step = e.shiftKey ? 1000 : 100;
      nudge(e.key === "ArrowLeft" ? -step : e.key === "ArrowRight" ? step : 0, e.key === "ArrowUp" ? -step : e.key === "ArrowDown" ? step : 0);
    } else if (!mod && !e.altKey && S.ed) {
      const t = { t: "text", s: "sticky", d: "stamp", h: "highlight", r: "rect", e: "ellipse", l: "line", v: "select" }[k];
      if (t) setTool(t);
      else if (e.key === "PageDown") gotoPage(Math.min(S.info.length - 1, S.cur + 1));
      else if (e.key === "PageUp") gotoPage(Math.max(0, S.cur - 1));
    }
  });
}

window.__ezpzxdw = S; // for automated tests
boot();
