import { lazy, Suspense } from "react";
import { NavLink, Route, Routes } from "react-router-dom";
import { DATA_SOURCE } from "./config";
import { ConnectButton } from "./wallet/ConnectButton";
import { Home } from "./routes/Home";
import { CharacterPage } from "./routes/Character";
import { MapBuilderPage } from "./routes/MapBuilder";
import { PartyPage } from "./routes/Party";

const GridDemo = lazy(() => import("./components/grid/Demo"));
const DiceDemo = lazy(() => import("./components/dice/Demo"));
const FormsDemo = lazy(() => import("./components/forms/Demo"));

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
        <ConnectButton />
      </header>
      <main>
        <Suspense fallback={<p className="muted">Loading...</p>}>
          <Routes>
            <Route path="/" element={<Home />} />
            <Route path="/character" element={<CharacterPage />} />
            <Route path="/map" element={<MapBuilderPage />} />
            <Route path="/party" element={<PartyPage />} />
            <Route path="/dev/grid" element={<GridDemo />} />
            <Route path="/dev/dice" element={<DiceDemo />} />
            <Route path="/dev/forms" element={<FormsDemo />} />
            <Route path="*" element={<p>Not found</p>} />
          </Routes>
        </Suspense>
      </main>
    </div>
  );
}
