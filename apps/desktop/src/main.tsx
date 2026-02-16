import { StrictMode } from "react";
import ReactDOM from "react-dom/client";
import { HashRouter } from "react-router-dom";
import "./i18n";
import "@fontsource/ibm-plex-sans/400.css";
import "@fontsource/ibm-plex-sans/600.css";
import "@fontsource/ibm-plex-sans/700.css";
import "@fontsource/ibm-plex-mono/400.css";
import "@fontsource/ibm-plex-mono/500.css";
import App from "./App";
import { EngineProvider } from "./lib/engineContext";
import "./styles/global.css";
import "./styles/ui.css";
import "./styles/shell.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <StrictMode>
    <EngineProvider>
      <HashRouter>
        <App />
      </HashRouter>
    </EngineProvider>
  </StrictMode>,
);
