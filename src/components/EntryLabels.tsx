import { RepeatIcon } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import type { Entry } from "@/lib/api";

/** A descrição do lançamento, com a marca de Recorrência ou a posição no Parcelamento. */
export function EntryDescription({ entry }: { entry: Entry }) {
  return (
    <span className="flex min-w-0 items-center gap-1.5">
      <span className="truncate">{entry.description}</span>
      {entry.installment && (
        <span className="shrink-0 text-muted-foreground tabular-nums">
          {entry.installment.number}/{entry.installment.count}
        </span>
      )}
      {entry.recurrenceId && (
        <RepeatIcon
          role="img"
          aria-label="Recorrência"
          className="size-3.5 shrink-0 text-muted-foreground"
        />
      )}
    </span>
  );
}

/** Previsto ou Atrasado; um lançamento efetivado não leva marca. */
export function EntryStatusBadge({ entry }: { entry: Entry }) {
  if (entry.status === "settled") return null;
  return entry.overdue ? (
    <Badge variant="destructive">Atrasado</Badge>
  ) : (
    <Badge variant="secondary">Previsto</Badge>
  );
}
