// The dungeon screen: the loading screen of the map's biome while the party and its sprites are
// read, then the battle (routes/PartyChain).
import { useEffect, useState } from "react";
import type { Address } from "@solana/kit";
import { DATA_SOURCE } from "../config";
import { PartyScreen } from "../routes/Party";
import { useLiveParty } from "../routes/PartyChain";
import { loadingScreen } from "./art";

/** The loading screen stays this long once the biome is known, so it does not flash. */
const MIN_LOADING_MS = 1_200;

export function Dungeon({ session, biome: hint }: { session: Address | null; biome: number | null }) {
  const live = useLiveParty(session);
  const loaded = DATA_SOURCE !== "chain" || !!live?.party || !!live?.error;
  // The biome comes with the quest that led here, else with the party's map.
  const biome = hint ?? live?.party?.map?.biome ?? null;
  const [shown, setShown] = useState(false);
  useEffect(() => setShown(false), [session]);
  useEffect(() => {
    if (!loaded || shown) return;
    const t = setTimeout(() => setShown(true), MIN_LOADING_MS);
    return () => clearTimeout(t);
  }, [loaded, shown]);

  if (!shown) {
    return (
      <div className="loading-screen">
        {biome !== null || loaded ? <img className="pixel" src={loadingScreen(biome ?? 0)} alt="" /> : <div className="loading-blank" />}
        <p>Descending into the dungeon…</p>
      </div>
    );
  }
  return <PartyScreen address={session} />;
}
