import { useEffect, useState } from "react";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";

type Phase =
  | { kind: "idle" }
  | { kind: "available"; update: Update }
  | { kind: "installing"; update: Update; percent: number | null }
  | { kind: "failed"; update: Update };

export function UpdatePrompt() {
  const [phase, setPhase] = useState<Phase>({ kind: "idle" });

  useEffect(() => {
    let cancelled = false;
    check()
      .then((update) => {
        if (update && !cancelled) setPhase({ kind: "available", update });
      })
      // Sem internet ou sem release publicado: o app abre normalmente.
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, []);

  if (phase.kind === "idle") return null;

  const { update } = phase;

  async function install() {
    setPhase({ kind: "installing", update, percent: null });
    let total = 0;
    let downloaded = 0;
    try {
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") {
          total = event.data.contentLength ?? 0;
        } else if (event.event === "Progress") {
          downloaded += event.data.chunkLength;
          if (total > 0) {
            const percent = Math.round((downloaded / total) * 100);
            setPhase({ kind: "installing", update, percent });
          }
        }
      });
      await relaunch();
    } catch {
      setPhase({ kind: "failed", update });
    }
  }

  return (
    <div className="update-backdrop">
      <div className="update-dialog" role="dialog" aria-modal="true">
        <h2>Nova versão disponível</h2>
        <p>
          A versão {update.version} está pronta para instalar. Você está na{" "}
          {update.currentVersion}.
        </p>

        {phase.kind === "installing" && (
          <p className="update-status">
            {phase.percent === null
              ? "Baixando…"
              : `Baixando… ${phase.percent}%`}
          </p>
        )}
        {phase.kind === "failed" && (
          <p className="update-status update-error">
            Não foi possível atualizar. Você pode tentar de novo ou continuar
            usando esta versão.
          </p>
        )}

        {phase.kind !== "installing" && (
          <div className="update-actions">
            <button onClick={() => setPhase({ kind: "idle" })}>
              Agora não
            </button>
            <button className="primary" onClick={install}>
              {phase.kind === "failed"
                ? "Tentar de novo"
                : "Atualizar e reiniciar"}
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
