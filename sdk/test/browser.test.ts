// The built client must load where there is no `process` (a browser; Vite does not pre-bundle a
// workspace package in dev). A child Node deletes `process` and imports every entry point of dist.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const pkg = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8")) as { exports: Record<string, { default: string }> };

test("every entry point loads without a process global", () => {
  const entries = Object.values(pkg.exports).map((e) => new URL(`../${e.default}`, import.meta.url).href);
  const script = `
    const out = process.stdout;
    delete globalThis.process;
    const failed = [];
    for (const e of ${JSON.stringify(entries)}) {
      try { await import(e); } catch (err) { failed.push(e + ": " + err.message); }
    }
    out.write(JSON.stringify(failed));
  `;
  // Dependencies resolve to their browser builds, as Vite resolves them.
  const run = spawnSync(process.execPath, ["--conditions=browser", "--input-type=module", "-e", script], { encoding: "utf8" });
  assert.equal(run.status, 0, run.stderr);
  assert.deepEqual(JSON.parse(run.stdout), [], "entry points that need process");
});
