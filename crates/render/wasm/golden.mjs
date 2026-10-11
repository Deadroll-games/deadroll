// WASM parity check: render the golden input through the WASM module and compare SHA-256
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
const input = readFileSync(new URL("input.bin", dir));
const expected = readFileSync(new URL("expected.sha256", dir), "utf8").trim();

const { instance } = await WebAssembly.instantiate(readFileSync(wasmPath), {});
const x = instance.exports;

function run(bytes, mode, scale) {
  const ptr = x.alloc(bytes.length);
  new Uint8Array(x.memory.buffer, ptr, bytes.length).set(bytes);
  const out = x.render(ptr, bytes.length, mode, scale);
  x.dealloc(ptr, bytes.length);
  if (out === 0) return { error: x.last_error() };
  const res = new Uint8Array(x.memory.buffer, out, x.result_len()).slice();
  return { bytes: res, width: x.result_width(), height: x.result_height() };
}

let failed = false;
const png = run(input, 1, 8);
const sha = createHash("sha256").update(png.bytes).digest("hex");
console.log(`png ${png.width}x${png.height}, ${png.bytes.length} bytes, sha256 ${sha}`);
if (sha !== expected) {
  console.error(`MISMATCH: native ${expected}`);
  failed = true;
}
// Repeated renders give the same bytes; errors are codes, not traps.
for (let i = 0; i < 3; i++) {
  const again = createHash("sha256").update(run(input, 1, 8).bytes).digest("hex");
  if (again !== sha) { console.error("non-deterministic"); failed = true; }
}
const bad = run(input.subarray(0, 10), 0, 1);
if (bad.error !== 1) { console.error(`expected error 1, got ${JSON.stringify(bad)}`); failed = true; }
const badScale = run(input, 0, 17);
if (badScale.error !== 9) { console.error(`expected error 9, got ${JSON.stringify(badScale)}`); failed = true; }
console.log(failed ? "FAIL" : "OK: WASM output matches native");
process.exit(failed ? 1 : 0);
