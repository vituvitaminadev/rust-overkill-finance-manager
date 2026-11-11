import { useEffect, useState } from "react";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

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
  const installing = phase.kind === "installing";

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
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open && !installing) setPhase({ kind: "idle" });
      }}
    >
      <DialogContent showCloseButton={false}>
        <DialogHeader>
          <DialogTitle>Nova versão disponível</DialogTitle>
          <DialogDescription>
            A versão {update.version} está pronta para instalar. Você está na{" "}
            {update.currentVersion}.
          </DialogDescription>
        </DialogHeader>

        {installing && (
          <p className="text-sm text-muted-foreground">
            {phase.percent === null
              ? "Baixando…"
              : `Baixando… ${phase.percent}%`}
          </p>
        )}
        {phase.kind === "failed" && (
          <p role="alert" className="text-sm text-destructive">
            Não foi possível atualizar. Você pode tentar de novo ou continuar
            usando esta versão.
          </p>
        )}

        {!installing && (
          <DialogFooter>
            <Button variant="outline" onClick={() => setPhase({ kind: "idle" })}>
              Agora não
            </Button>
            <Button onClick={install}>
              {phase.kind === "failed"
                ? "Tentar de novo"
                : "Atualizar e reiniciar"}
            </Button>
          </DialogFooter>
        )}
      </DialogContent>
    </Dialog>
  );
}
