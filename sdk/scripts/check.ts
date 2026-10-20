// Fails when the committed client is stale:
//   1. target/idl (if built) differs from sdk/idl  -> run `pnpm generate`;
//   2. Codama output for sdk/idl differs from src/generated -> run `pnpm generate --no-copy`.

import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, relative, resolve } from "node:path";
import { IDL_DIR, PROGRAM_IDLS, render, SDK } from "./generate.ts";

const TARGET_IDL = resolve(SDK, "..", "target", "idl");
const GENERATED = join(SDK, "src", "generated");

function files(dir: string): string[] {
  return readdirSync(dir).flatMap((f) => {
    const p = join(dir, f);
    return statSync(p).isDirectory() ? files(p) : [p];
  });
}

const stale: string[] = [];
if (existsSync(TARGET_IDL)) {
  for (const file of Object.values(PROGRAM_IDLS)) {
    if (!readFileSync(join(TARGET_IDL, file)).equals(readFileSync(join(IDL_DIR, file)))) stale.push(`idl/${file}`);
  }
} else {
  console.log("no target/idl: skipping the IDL comparison");
}

const tmp = mkdtempSync(join(tmpdir(), "deadroll-sdk-"));
try {
  await render(tmp);
  const want = new Map(files(tmp).map((p) => [relative(tmp, p), readFileSync(p)]));
  const have = new Map(files(GENERATED).map((p) => [relative(GENERATED, p), readFileSync(p)]));
  for (const [path, bytes] of want) if (!have.get(path)?.equals(bytes)) stale.push(`src/generated/${path}`);
  for (const path of have.keys()) if (!want.has(path)) stale.push(`src/generated/${path} (extra)`);
} finally {
  rmSync(tmp, { recursive: true, force: true });
}

if (stale.length) {
  console.error(`sdk is stale (run \`pnpm --filter @deadroll/sdk generate\`):\n  ${stale.join("\n  ")}`);
  process.exit(1);
}
console.log("sdk is up to date with the IDLs");
