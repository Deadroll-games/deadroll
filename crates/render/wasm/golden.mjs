// WASM parity check: render the golden warrior through the WASM module and compare SHA-256
// with the native render. Run `cargo test -p deadroll-render --test golden` first.
// Usage: node crates/render/wasm/golden.mjs <path/to/module.wasm>
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";

const [wasmPath] = process.argv.slice(2);
if (!wasmPath) {
  console.error("usage: node golden.mjs <module.wasm>");
  process.exit(2);
}
const dir = new URL("../../../target/render-golden/", import.meta.url);
const read = (f) => new Uint8Array(readFileSync(new URL(f, dir)));
const args = { sprites: read("sprites.bin"), palette: read("palette.bin"), state: read("state.bin") };
const expected = readFileSync(new URL("expected.sha256", dir), "utf8").trim();

const { instance } = await WebAssembly.instantiate(readFileSync(wasmPath), {});
const x = instance.exports;

/** render(sprites, palette, state, asset, mode), FORMAT.md 7. */
function run({ sprites, palette, state }, mode, asset = new Uint8Array(32)) {
  const parts = [sprites, palette, state, asset];
  const total = parts.reduce((n, p) => n + p.length, 0);
  const base = x.alloc(total);
  const ptrs = [];
  let at = base;
  for (const p of parts) {
    new Uint8Array(x.memory.buffer, at, p.length).set(p);
    ptrs.push(at);
    at += p.length;
  }
  const out = x.render(ptrs[0], sprites.length, ptrs[1], palette.length, ptrs[2], state.length, ptrs[3], mode);
  x.dealloc(base, total);
  if (out === 0) return { error: x.last_error() };
  const res = new Uint8Array(x.memory.buffer, out, x.result_len()).slice();
  return { bytes: res, width: x.result_width(), height: x.result_height() };
}

let failed = false;
const png = run(args, 1);
const sha = createHash("sha256").update(png.bytes).digest("hex");
console.log(`png ${png.width}x${png.height}, ${png.bytes.length} bytes, sha256 ${sha}`);
if (sha !== expected) {
  console.error(`MISMATCH: native ${expected}`);
  failed = true;
}
// Repeated renders give the same bytes; errors are codes, not traps.
for (let i = 0; i < 3; i++) {
  const again = createHash("sha256").update(run(args, 1).bytes).digest("hex");
  if (again !== sha) { console.error("non-deterministic"); failed = true; }
}
const raw = run(args, 0);
if (raw.width !== 64 || raw.height !== 64 || raw.bytes.length !== 64 * 64 * 4) {
  console.error(`raw rgba: ${raw.width}x${raw.height}`); failed = true;
}
const checks = [
  [{ ...args, sprites: args.sprites.subarray(0, args.sprites.length - 1) }, 0, 1, "truncated book"],
  [{ ...args, state: args.state.subarray(0, 10) }, 0, 9, "short state"],
  [args, 2, 9, "bad mode"],
];
for (const [a, mode, code, what] of checks) {
  const r = run(a, mode);
  if (r.error !== code) { console.error(`${what}: expected error ${code}, got ${JSON.stringify(r.error ?? "ok")}`); failed = true; }
}
console.log(failed ? "FAIL" : "OK: WASM output matches native");
process.exit(failed ? 1 : 0);
