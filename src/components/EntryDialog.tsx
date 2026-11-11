import { type FormEvent, useState } from "react";
import { Field, FormError } from "@/components/Field";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import {
  NativeSelect,
  NativeSelectOption,
} from "@/components/ui/native-select";
import {
  api,
  errorMessage,
  type Entry,
  type EntryKind,
  type Member,
} from "@/lib/api";
import { formatAmountInput, parseMoney, today } from "@/lib/format";
import {
  useCategories,
  useHouseholdMutation,
  useMembers,
} from "@/lib/queries";

const KINDS: { kind: EntryKind; label: string }[] = [
  { kind: "expense", label: "Despesa" },
  { kind: "income", label: "Receita" },
];

/** Cria um lançamento ou, quando recebe `entry`, edita o existente. */
export function EntryDialog({
  entry,
  me,
  onClose,
}: {
  entry?: Entry;
  me: Member;
  onClose: () => void;
}) {
  const [kind, setKind] = useState<EntryKind>(entry?.kind ?? "expense");
  const [amount, setAmount] = useState(
    entry ? formatAmountInput(entry.amountCents) : "",
  );
  const [date, setDate] = useState(entry?.date ?? today());
  const [description, setDescription] = useState(entry?.description ?? "");
  const [categoryId, setCategoryId] = useState(entry?.categoryId ?? "");
  const [assigneeId, setAssigneeId] = useState(entry?.assigneeId ?? me.id);
  const [amountError, setAmountError] = useState(false);

  const categories = useCategories();
  const members = useMembers();
  const isAdmin = me.role === "admin";

  // Uma categoria arquivada só aparece se já for a deste lançamento.
  const options = (categories.data ?? []).filter(
    (category) =>
      category.kind === kind &&
      (!category.archived || category.id === entry?.categoryId),
  );
  const assignees = (members.data ?? []).filter(
    (member) => member.active || member.id === entry?.assigneeId,
  );

  const save = useHouseholdMutation((amountCents: number) => {
    const input = {
      kind,
      amountCents,
      date,
      description,
      categoryId,
      assigneeId: isAdmin ? assigneeId : null,
    };
    return entry ? api.updateEntry(entry.id, input) : api.createEntry(input);
  }, onClose);

  function changeKind(next: EntryKind) {
    setKind(next);
    setCategoryId("");
  }

  function submit(event: FormEvent) {
    event.preventDefault();
    const amountCents = parseMoney(amount);
    setAmountError(amountCents === null);
    if (amountCents !== null) save.mutate(amountCents);
  }

  const error = amountError
    ? "Informe o valor em reais, como 1.234,56."
    : save.error && errorMessage(save.error);

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent>
        <form onSubmit={submit} className="grid gap-4">
          <DialogHeader>
            <DialogTitle>
              {entry ? "Editar lançamento" : "Novo lançamento"}
            </DialogTitle>
          </DialogHeader>

          <div
            role="radiogroup"
            aria-label="Tipo"
            className="grid grid-cols-2 gap-1 rounded-lg bg-muted p-1"
          >
            {KINDS.map((option) => (
              <Button
                key={option.kind}
                type="button"
                role="radio"
                aria-checked={kind === option.kind}
                variant={kind === option.kind ? "outline" : "ghost"}
                onClick={() => changeKind(option.kind)}
              >
                {option.label}
              </Button>
            ))}
          </div>

          <div className="grid grid-cols-2 gap-3">
            <Field label="Valor (R$)" htmlFor="entry-amount">
              <Input
                id="entry-amount"
                autoFocus
                inputMode="decimal"
                placeholder="0,00"
                value={amount}
                onChange={(e) => setAmount(e.target.value)}
              />
            </Field>
            <Field label="Data" htmlFor="entry-date">
              <Input
                id="entry-date"
                type="date"
                required
                value={date}
                onChange={(e) => setDate(e.target.value)}
              />
            </Field>
          </div>

          <Field label="Descrição" htmlFor="entry-description">
            <Input
              id="entry-description"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
            />
          </Field>

          <Field label="Categoria" htmlFor="entry-category">
            <NativeSelect
              id="entry-category"
              className="w-full"
              required
              value={categoryId}
              onChange={(e) => setCategoryId(e.target.value)}
            >
              <NativeSelectOption value="" disabled>
                Escolha uma categoria
              </NativeSelectOption>
              {options.map((category) => (
                <NativeSelectOption key={category.id} value={category.id}>
                  {category.name}
                </NativeSelectOption>
              ))}
            </NativeSelect>
          </Field>

          {isAdmin && (
            <Field label="Responsável" htmlFor="entry-assignee">
              <NativeSelect
                id="entry-assignee"
                className="w-full"
                value={assigneeId}
                onChange={(e) => setAssigneeId(e.target.value)}
              >
                {assignees.map((member) => (
                  <NativeSelectOption key={member.id} value={member.id}>
                    {member.username}
                  </NativeSelectOption>
                ))}
              </NativeSelect>
            </Field>
          )}

          <FormError error={error || null} />
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              Cancelar
            </Button>
            <Button type="submit" disabled={save.isPending}>
              Salvar
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
