import React from "react";
import ReactDOM from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import App from "./App";
import "./index.css";

// O tema acompanha o do sistema operacional.
const prefersDark = window.matchMedia("(prefers-color-scheme: dark)");
const applyTheme = () =>
  document.documentElement.classList.toggle("dark", prefersDark.matches);
applyTheme();
prefersDark.addEventListener("change", applyTheme);

const queryClient = new QueryClient({
  defaultOptions: {
    // Os dados são locais: não há por que repetir uma consulta que falhou.
    queries: { retry: false, refetchOnWindowFocus: false },
  },
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <App />
    </QueryClientProvider>
  </React.StrictMode>,
);
