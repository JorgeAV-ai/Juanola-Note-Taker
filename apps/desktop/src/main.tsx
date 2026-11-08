import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { ErrorBoundary } from "./components/ErrorBoundary";
import "./styles/tokens.css";
import { invoke } from "@tauri-apps/api/core";

function render() {
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <ErrorBoundary>
        <App />
      </ErrorBoundary>
    </React.StrictMode>,
  );
  // Recorrido automático para probar la .app sin tocarla (JUANOLA_AUTOTEST=1).
  invoke<string | null>("autotest_config")
    .then((mode) => {
      if (mode) import("./dev/autotest").then((m) => setTimeout(() => m.runAutotest(mode), 1500));
    })
    .catch(() => {});
}

// En un navegador normal (sin Tauri) y en desarrollo, backend simulado.
if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) {
  import("./dev/mockIpc").then((m) => {
    m.installMockIpc();
    render();
  });
} else {
  render();
}
