import { FormError } from "@/components/Field";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { api, errorMessage, type ConflictSide } from "@/lib/api";
import { formatDate, formatMoney } from "@/lib/format";
import { useConflicts, useHouseholdMutation } from "@/lib/queries";

/** Os lançamentos alterados aqui e no Anfitrião, lado a lado, para escolher qual fica. */
export function ConflictsDialog({ onClose }: { onClose: () => void }) {
  const conflicts = useConflicts(true);
  const resolve = useHouseholdMutation(
    ({ entryId, keepMine }: { entryId: string; keepMine: boolean }) =>
      api.resolveConflict(entryId, keepMine),
  );
  const list = conflicts.data ?? [];

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>Conflitos</DialogTitle>
          <DialogDescription>
            Estes lançamentos foram alterados neste computador e também no
            Anfitrião. Escolha qual versão fica.
          </DialogDescription>
        </DialogHeader>
        <FormError error={resolve.error && errorMessage(resolve.error)} />
        {conflicts.data && list.length === 0 && (
          <p className="text-sm text-muted-foreground">Nenhum conflito pendente.</p>
        )}
        <ul className="grid max-h-[60vh] gap-4 overflow-y-auto">
          {list.map((conflict) => (
            <li key={conflict.entryId} className="grid gap-2 rounded-lg border p-3">
              {conflict.reason && (
                <p className="text-sm">
                  O Anfitrião recusou a alteração: {conflict.reason}
                </p>
              )}
              <div className="grid gap-3 sm:grid-cols-2">
                <Side
                  title="Neste computador"
                  side={conflict.mine}
                  missing="Excluído neste computador."
                  disabled={resolve.isPending}
                  onChoose={() =>
                    resolve.mutate({ entryId: conflict.entryId, keepMine: true })
                  }
                />
                <Side
                  title="No Anfitrião"
                  side={conflict.theirs}
                  missing="O Anfitrião não tem este lançamento."
                  disabled={resolve.isPending}
                  onChoose={() =>
                    resolve.mutate({ entryId: conflict.entryId, keepMine: false })
                  }
                />
              </div>
            </li>
          ))}
        </ul>
      </DialogContent>
    </Dialog>
  );
}

function Side({
  title,
  side,
  missing,
  disabled,
  onChoose,
}: {
  title: string;
  side: ConflictSide | null;
  /** O que dizer quando o lançamento não existe deste lado. */
  missing: string;
  disabled: boolean;
  onChoose: () => void;
}) {
  return (
    <div className="grid content-start gap-2 rounded-md bg-muted/50 p-3 text-sm">
      <span className="font-medium">{title}</span>
      {side ? (
        <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-0.5">
          <dt className="text-muted-foreground">Descrição</dt>
          <dd className="truncate">{side.description}</dd>
          <dt className="text-muted-foreground">Valor</dt>
          <dd className="tabular-nums">
            {side.kind === "expense" ? "−" : "+"}
            {formatMoney(side.amountCents)}
          </dd>
          <dt className="text-muted-foreground">Data</dt>
          <dd className="tabular-nums">{formatDate(side.date)}</dd>
          <dt className="text-muted-foreground">Categoria</dt>
          <dd>{side.categoryName}</dd>
          <dt className="text-muted-foreground">Responsável</dt>
          <dd>{side.assigneeName}</dd>
          <dt className="text-muted-foreground">Situação</dt>
          <dd>{side.status === "settled" ? "Efetivado" : "Previsto"}</dd>
        </dl>
      ) : (
        <p className="text-muted-foreground">{missing}</p>
      )}
      <Button variant="outline" size="sm" disabled={disabled} onClick={onChoose}>
        Ficar com esta
      </Button>
    </div>
  );
}
