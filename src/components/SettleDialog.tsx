import { type FormEvent, useState } from "react";
import { Field, FormError } from "@/components/Field";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { api, errorMessage, type Entry } from "@/lib/api";
import { formatAmountInput, formatDate, parseMoney } from "@/lib/format";
import { useHouseholdMutation } from "@/lib/queries";

/** Marca um lançamento previsto como efetivado, permitindo corrigir o valor. */
export function SettleDialog({
  entry,
  onClose,
}: {
  entry: Entry;
  onClose: () => void;
}) {
  const [amount, setAmount] = useState(formatAmountInput(entry.amountCents));
  const [invalid, setInvalid] = useState(false);
  const settle = useHouseholdMutation(
    (amountCents: number) => api.settleEntry(entry.id, amountCents),
    onClose,
  );

  function submit(event: FormEvent) {
    event.preventDefault();
    const amountCents = parseMoney(amount);
    setInvalid(amountCents === null);
    if (amountCents !== null) settle.mutate(amountCents);
  }

  const error = invalid
    ? "Informe o valor em reais, como 1.234,56."
    : settle.error && errorMessage(settle.error);

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent>
        <form onSubmit={submit} className="grid gap-4">
          <DialogHeader>
            <DialogTitle>Efetivar lançamento</DialogTitle>
            <DialogDescription>
              {entry.description}, previsto para {formatDate(entry.date)}.
              Ajuste o valor se ele veio diferente.
            </DialogDescription>
          </DialogHeader>
          <Field label="Valor real (R$)" htmlFor="settle-amount">
            <Input
              id="settle-amount"
              autoFocus
              inputMode="decimal"
              value={amount}
              onChange={(e) => setAmount(e.target.value)}
            />
          </Field>
          <FormError error={error || null} />
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              Cancelar
            </Button>
            <Button type="submit" disabled={settle.isPending}>
              Efetivar
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
