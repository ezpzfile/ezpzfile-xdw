// Open an untrusted .xdw / .xbd without risking the page: the work runs in a
// Web Worker, the engine stops at the limits you give it, and a worker that
// runs past `timeoutMs` is terminated (its memory goes with it).
//
//   import { readXdwSafely } from "./worker/ezpzxdw-safe.js";
//   const { result, hiddenChars } = await readXdwSafely(bytes, {
//     want: "text",                      // "text" (one string per page) | "pages" | "check"
//     limits: { maxPages: 300, maxBytes: 200 << 20 },
//     timeoutMs: 15000,
//     includeHidden: false,              // true: keep text that a black box or picture covers
//   });
//
// Errors: "timeout" when the time ran out, otherwise the engine's message
// ("too large or damaged: …" when a limit was hit).

const WORKER_URL = new URL("./ezpzxdw-worker.js", import.meta.url);

export function readXdwSafely(bytes, { want = "text", limits = {}, timeoutMs = 15000, includeHidden = false } = {}) {
  return new Promise((resolve, reject) => {
    const w = new Worker(WORKER_URL, { type: "module" });
    const done = (fn, v) => {
      clearTimeout(timer);
      w.terminate();
      fn(v);
    };
    const timer = setTimeout(() => done(reject, new Error("timeout")), timeoutMs);
    w.onmessage = (e) => (e.data.ok ? done(resolve, e.data) : done(reject, new Error(e.data.error)));
    w.onerror = (e) => done(reject, new Error(e.message || "worker error"));
    // copy so the caller keeps its buffer
    w.postMessage({ id: 1, bytes: new Uint8Array(bytes), limits, want, includeHidden });
  });
}
