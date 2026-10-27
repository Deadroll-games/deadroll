import { lazy, Suspense } from "react";
import { Navigate, NavLink, Route, Routes, useSearchParams } from "react-router-dom";
import { DATA_SOURCE, DEV_TOOLS } from "./config";
import { NetworkBusy } from "./components/NetworkBusy";
import { ConnectButton } from "./wallet/ConnectButton";
import { GameShell } from "./game/GameShell";

// The wiki and the bestiary are built from the repo at build time (app/scripts/wiki.ts): no RPC.
const WikiPage = lazy(() => import("./wiki/WikiPage"));
const BestiaryPage = lazy(() => import("./wiki/BestiaryPage"));

// Mock-data demo pages; not even emitted in the devnet build (the bundler folds an inline MODE check, not DEV_TOOLS).
const none = () => null;
const GridDemo = import.meta.env.MODE !== "devnet" ? lazy(() => import("./components/grid/Demo")) : none;
const DiceDemo = import.meta.env.MODE !== "devnet" ? lazy(() => import("./components/dice/Demo")) : none;
const FormsDemo = import.meta.env.MODE !== "devnet" ? lazy(() => import("./components/forms/Demo")) : none;

/** Old links (`/party?s=...`) open the same party in the game screen. */
function OldParty() {
  const [params] = useSearchParams();
  const s = params.get("s");
  return <Navigate to={s ? `/?view=dungeon&s=${s}` : "/"} replace />;
}

export function App() {
  return (
    <div className="app">
      <header className="topbar">
        <NavLink to="/" className="brand" end>
          Deadroll
        </NavLink>
        <nav>
          <NavLink to="/wiki">Wiki</NavLink>
          <NavLink to="/bestiary">Bestiary</NavLink>
        </nav>
        <span className={`badge ${DATA_SOURCE}`}>{DATA_SOURCE === "mock" ? "mock data" : "chain"}</span>
        <NetworkBusy />
        <ConnectButton />
      </header>
      <Suspense fallback={<p className="muted">Loading...</p>}>
        <Routes>
          <Route path="/" element={<GameShell />} />
          <Route path="/wiki" element={<WikiPage />} />
          <Route path="/bestiary" element={<BestiaryPage />} />
          <Route path="/party" element={<OldParty />} />
          <Route path="/character/*" element={<Navigate to="/" replace />} />
          <Route path="/map" element={<Navigate to="/" replace />} />
          {DEV_TOOLS && (
            <>
              <Route path="/dev/grid" element={<GridDemo />} />
              <Route path="/dev/dice" element={<DiceDemo />} />
              <Route path="/dev/forms" element={<FormsDemo />} />
            </>
          )}
          <Route path="*" element={<p>Not found</p>} />
        </Routes>
      </Suspense>
    </div>
  );
}
