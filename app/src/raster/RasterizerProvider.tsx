import { createContext, useContext, useEffect, useState, type ReactNode } from "react";
import { rpcAccountSource } from "../chain/accounts";
import type { RenderRequest } from "../chain/types";
import { DATA_SOURCE } from "../config";
import { client } from "../wallet/client";
import { idbWasmCache } from "./idb";
import { loadRender, type LoadedRender } from "./loader";
import { mockRenderSource } from "./mockSource";
import { characterAppearance, renderRequest } from "../chain/appearance";
import { dressed, starterEquipment } from "../chain/items";
import { Aspect, Class, Race } from "../chain/types";

export type RasterizerState =
  | { status: "loading" }
  | { status: "error"; error: string }
  | { status: "ready"; render: LoadedRender; sample: RenderRequest | null };

const Ctx = createContext<RasterizerState>({ status: "loading" });

async function load(): Promise<{ render: LoadedRender; sample: RenderRequest | null }> {
  if (DATA_SOURCE === "mock") {
    const { source, sample } = await mockRenderSource();
    return { render: await loadRender(source, { cache: idbWasmCache }), sample };
  }
  const render = await loadRender(rpcAccountSource(client.rpc), { cache: idbWasmCache });
  // The demo: a new human warrior in the starter kit, as tools/art draws it.
  return { render, sample: renderRequest(dressed(characterAppearance(Race.Human, Aspect.Piercing), starterEquipment(Class.Warrior))) };
}

export function RasterizerProvider({ children }: { children: ReactNode }) {
  const [state, setState] = useState<RasterizerState>({ status: "loading" });
  useEffect(() => {
    let live = true;
    load().then(
      (r) => live && setState({ status: "ready", ...r }),
      (e: unknown) => live && setState({ status: "error", error: e instanceof Error ? e.message : String(e) }),
    );
    return () => {
      live = false;
    };
  }, []);
  return <Ctx.Provider value={state}>{children}</Ctx.Provider>;
}

export function useRasterizer(): RasterizerState {
  return useContext(Ctx);
}

/** Renders a request to RGBA ImageData at scale 1, or null if the rasterizer is not ready or fails. */
export function useRenderImage(req: RenderRequest | null): { image: ImageData | null; error: string | null } {
  const state = useRasterizer();
  if (state.status !== "ready" || !req) return { image: null, error: state.status === "error" ? state.error : null };
  try {
    const { rasterizer, palette, books } = state.render;
    const out = rasterizer.render(req, palette, books.map((b) => b.body));
    return { image: new ImageData(new Uint8ClampedArray(out.bytes), out.width, out.height), error: null };
  } catch (e) {
    return { image: null, error: e instanceof Error ? e.message : String(e) };
  }
}
