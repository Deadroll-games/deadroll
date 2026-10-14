/// <reference types="vitest/config" />
import { createReadStream, existsSync } from "node:fs";
import { resolve } from "node:path";
import { defineConfig, loadEnv, type Plugin } from "vite";
import { findBundledRpcKey, isBundleSafeRpc } from "./src/rpcSettings";
import react from "@vitejs/plugin-react";

const root = resolve(import.meta.dirname, "..");

/**
 * Mock mode only: serves local build outputs under /dev-assets/ so the loader can run its normal
 * verify path without a validator. Production builds always read the chain.
 */
const DEV_ASSETS: Record<string, string> = {
  "rasterizer.wasm": "target/render-wasm/deadroll_render.wasm",
  "golden-input.bin": "target/render-golden/input.bin",
  "palette.bin": "target/render-golden/palette.bin",
  "book.bin": "target/render-golden/book.bin",
  "combat.wasm": "target/wasm32-unknown-unknown/wasm/deadroll_combat_wasm.wasm",
};

function devAssets(): Plugin {
  return {
    name: "deadroll-dev-assets",
    apply: "serve",
    configureServer(server) {
      server.middlewares.use("/dev-assets", (req, res, next) => {
        const name = (req.url ?? "").replace(/^\//, "").split("?")[0] ?? "";
        const rel = DEV_ASSETS[name];
        if (!rel) return next();
        const file = resolve(root, rel);
        if (!existsSync(file)) {
          res.statusCode = 404;
          res.end(`missing ${rel}; build it first (see app/README.md)`);
          return;
        }
        res.setHeader("Content-Type", name.endsWith(".wasm") ? "application/wasm" : "application/octet-stream");
        createReadStream(file).pipe(res);
      });
    },
  };
}

/**
 * A production bundle is public: a provider URL with an API key in VITE_RPC_URL or VITE_RPC_WS_URL would
 * ship to every visitor. Builds accept only the public devnet endpoints or localhost, and every emitted
 * chunk is scanned for provider hosts in case one is hardcoded. The devnet build (the hosted site) also
 * refuses mock data and the mock VRF oracle.
 */
function noBundledRpcKey(mode: string): Plugin {
  return {
    name: "deadroll-no-bundled-rpc-key",
    apply: "build",
    buildStart() {
      const env = loadEnv(mode, import.meta.dirname, "VITE_");
      for (const name of ["VITE_RPC_URL", "VITE_RPC_WS_URL"]) {
        const value = env[name];
        if (value && !isBundleSafeRpc(value)) {
          throw new Error(`${name} would be bundled into the site. Use the public devnet RPC for hosted builds; keep a private URL in the Custom RPC setting or an untracked .env.*.local for dev runs.`);
        }
      }
      if (mode === "devnet" && env.VITE_DATA === "mock") throw new Error("VITE_DATA=mock is not allowed in the devnet build.");
      if (mode === "devnet" && env.VITE_MOCK_VRF === "1") throw new Error("VITE_MOCK_VRF=1 is not allowed in the devnet build.");
    },
    generateBundle(_options, bundle) {
      for (const [file, out] of Object.entries(bundle)) {
        const code = out.type === "chunk" ? out.code : typeof out.source === "string" ? out.source : "";
        const hit = findBundledRpcKey(code);
        if (hit) throw new Error(`${file} contains a provider RPC reference (${hit}). Hosted builds may only use the public devnet RPC.`);
      }
    },
  };
}

export default defineConfig(({ mode }) => ({
  plugins: [react(), devAssets(), noBundledRpcKey(mode)],
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test-setup.ts"],
  },
}));
