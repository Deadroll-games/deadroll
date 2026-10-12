import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import { App } from "./App";
import { RasterizerProvider } from "./raster/RasterizerProvider";
import { SessionKeyProvider } from "./session-key/SessionKeyProvider";
import { AppWalletProvider } from "./wallet/AppWallet";
import { ToastProvider } from "./components/toast/Toast";
import "./styles.css";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <BrowserRouter>
      <ToastProvider>
        <AppWalletProvider>
          <SessionKeyProvider>
            <RasterizerProvider>
              <App />
            </RasterizerProvider>
          </SessionKeyProvider>
        </AppWalletProvider>
      </ToastProvider>
    </BrowserRouter>
  </StrictMode>,
);
