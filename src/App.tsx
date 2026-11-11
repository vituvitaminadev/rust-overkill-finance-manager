import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { UpdatePrompt } from "./UpdatePrompt";
import "./App.css";

function App() {
  const [version, setVersion] = useState("");

  useEffect(() => {
    getVersion().then(setVersion);
  }, []);

  return (
    <main className="shell">
      <h1>Finanças da Residência</h1>
      <p className="version">{version && `Versão ${version}`}</p>
      <UpdatePrompt />
    </main>
  );
}

export default App;
