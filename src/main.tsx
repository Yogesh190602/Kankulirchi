import "@fontsource-variable/bricolage-grotesque/standard.css";
import "./index.css";

import { QueryClientProvider } from "@tanstack/react-query";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { HashRouter } from "react-router-dom";

import App from "./App";
import { queryClient } from "./services/queryClient";

const root = document.getElementById("root");
if (!root) throw new Error("The #root element is missing from index.html.");

createRoot(root).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <HashRouter>
        <App />
      </HashRouter>
    </QueryClientProvider>
  </StrictMode>,
);
