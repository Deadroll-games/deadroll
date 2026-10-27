import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import { App } from "./App";
import { RasterizerProvider } from "./raster/RasterizerProvider";
import { AppWalletProvider } from "./wallet/AppWallet";
import { ToastProvider } from "./components/toast/Toast";
import { ActiveCharacterProvider } from "./game/ActiveCharacter";
import "./styles.css";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <BrowserRouter>
      <ToastProvider>
        <AppWalletProvider>
          <ActiveCharacterProvider>
            <RasterizerProvider>
              <App />
            </RasterizerProvider>
          </ActiveCharacterProvider>
        </AppWalletProvider>
      </ToastProvider>
    </BrowserRouter>
  </StrictMode>,
);
