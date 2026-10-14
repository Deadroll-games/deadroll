import { lazy, Suspense } from "react";
import { NavLink, Route, Routes } from "react-router-dom";
import { DATA_SOURCE, DEV_TOOLS } from "./config";
import { NetworkBusy } from "./components/NetworkBusy";
import { RpcSettings } from "./components/RpcSettings";
import { ConnectButton } from "./wallet/ConnectButton";
import { SessionKeyButton } from "./session-key/SessionKeyProvider";
import { Home } from "./routes/Home";
import { CharacterPage } from "./routes/Character";
import { CharacterDetailPage } from "./routes/CharacterDetail";
import { MapBuilderPage } from "./routes/MapBuilder";
import { PartyPage } from "./routes/Party";

// Mock-data demo pages; not even emitted in the devnet build (the bundler folds an inline MODE check, not DEV_TOOLS).
const none = () => null;
const GridDemo = import.meta.env.MODE !== "devnet" ? lazy(() => import("./components/grid/Demo")) : none;
const DiceDemo = import.meta.env.MODE !== "devnet" ? lazy(() => import("./components/dice/Demo")) : none;
const FormsDemo = import.meta.env.MODE !== "devnet" ? lazy(() => import("./components/forms/Demo")) : none;

export function App() {
  return (
    <div className="app">
      <header className="topbar">
        <NavLink to="/" className="brand">
          Deadroll
        </NavLink>
        <nav>
          <NavLink to="/character">Character</NavLink>
          <NavLink to="/map">Map builder</NavLink>
          <NavLink to="/party">Party</NavLink>
        </nav>
        <span className={`badge ${DATA_SOURCE}`}>{DATA_SOURCE === "mock" ? "mock data" : "chain"}</span>
        <RpcSettings />
        <NetworkBusy />
        <SessionKeyButton />
        <ConnectButton />
      </header>
      <main>
        <Suspense fallback={<p className="muted">Loading...</p>}>
          <Routes>
            <Route path="/" element={<Home />} />
            <Route path="/character" element={<CharacterPage />} />
            <Route path="/character/:address" element={<CharacterDetailPage />} />
            <Route path="/map" element={<MapBuilderPage />} />
            <Route path="/party" element={<PartyPage />} />
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
      </main>
    </div>
  );
}
