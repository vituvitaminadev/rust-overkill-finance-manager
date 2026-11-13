import { useState } from "react";
import { ConflictsDialog } from "@/components/ConflictsDialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { SyncInfo } from "@/lib/api";
import { useSyncInfo } from "@/lib/queries";

/** O estado da sincronização em poucas palavras, e a porta de entrada dos Conflitos. */
export function SyncBadge() {
  const info = useSyncInfo().data;
  const [resolving, setResolving] = useState(false);
  if (!info) return null;

  return (
    <>
      <Badge variant="outline" title={describe(info).hint}>
        {describe(info).label}
      </Badge>
      {info.conflicts > 0 && (
        <Button variant="destructive" size="sm" onClick={() => setResolving(true)}>
          {plural(info.conflicts, "conflito", "conflitos")}
        </Button>
      )}
      {resolving && <ConflictsDialog onClose={() => setResolving(false)} />}
    </>
  );
}

function plural(count: number, one: string, many: string): string {
  return `${count} ${count === 1 ? one : many}`;
}

function describe(info: SyncInfo): { label: string; hint: string } {
  if (!info.replica) {
    return {
      label: `Anfitrião · ${plural(info.connected, "cópia conectada", "cópias conectadas")}`,
      hint: "Este computador guarda os dados da residência e os serve aos outros.",
    };
  }
  const pending =
    info.pending > 0
      ? ` · ${plural(info.pending, "alteração pendente", "alterações pendentes")}`
      : "";
  switch (info.link) {
    case "online":
      return {
        label: info.pending > 0 ? `Enviando${pending}` : "Sincronizado",
        hint: "Conectado ao Anfitrião.",
      };
    case "protocol_mismatch":
      return {
        label: `Atualize para sincronizar${pending}`,
        hint: "Este computador e o Anfitrião estão em versões diferentes do app.",
      };
    case "unauthorized":
      return {
        label: `Sem acesso ao Anfitrião${pending}`,
        hint: "Saia e entre de novo com o Anfitrião ligado. Se você foi removido da residência, fale com um administrador.",
      };
    default:
      return {
        label: `Offline${pending}`,
        hint: "O Anfitrião está fora de alcance. Seus lançamentos ficam guardados e são enviados quando ele voltar.",
      };
  }
}
