import { type FormEvent, useState } from "react";
import { Field, FormError } from "@/components/Field";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
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

/** Como um lançamento novo se repete: avulso, por Recorrência ou por Parcelamento. */
type Repeat = "none" | "monthly" | "installments";

/**
 * Cria um lançamento (avulso, recorrente ou parcelado) ou, quando recebe
 * `entry`, edita o existente.
 */
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
  // Enquanto ninguém marca nada, a situação acompanha a data: futuro é previsto.
  const [settledChoice, setSettledChoice] = useState<boolean | null>(
    entry ? entry.status === "settled" : null,
  );
  const [repeat, setRepeat] = useState<Repeat>("none");
  const [endDate, setEndDate] = useState("");
  const [count, setCount] = useState("2");
  const [following, setFollowing] = useState(false);
  const [localError, setLocalError] = useState<string | null>(null);

  const categories = useCategories();
  const members = useMembers();
  const isAdmin = me.role === "admin";
  const settled = settledChoice ?? date <= today();

  // Uma categoria arquivada só aparece se já for a deste lançamento.
  const options = (categories.data ?? []).filter(
    (category) =>
      category.kind === kind &&
      (!category.archived || category.id === entry?.categoryId),
  );
  const assignees = (members.data ?? []).filter(
    (member) => member.active || member.id === entry?.assigneeId,
  );

  const save = useHouseholdMutation(async (amountCents: number) => {
    const assignee = isAdmin ? assigneeId : null;
    if (repeat === "monthly") {
      await api.createRecurrence({
        kind,
        amountCents,
        description,
        categoryId,
        assigneeId: assignee,
        firstDate: date,
        endMonth: endDate ? endDate.slice(0, 7) : null,
      });
    } else if (repeat === "installments") {
      await api.createInstallmentPlan({
        description,
        totalCents: amountCents,
        count: Number(count),
        firstDate: date,
        categoryId,
        assigneeId: assignee,
      });
    } else {
      const input = {
        kind,
        amountCents,
        date,
        description,
        categoryId,
        assigneeId: assignee,
        status: settled ? ("settled" as const) : ("planned" as const),
      };
      await (entry
        ? api.updateEntry(entry.id, input, following)
        : api.createEntry(input));
    }
  }, onClose);

  function changeKind(next: EntryKind) {
    setKind(next);
    setCategoryId("");
    if (next === "income" && repeat === "installments") setRepeat("none");
  }

  function submit(event: FormEvent) {
    event.preventDefault();
    const amountCents = parseMoney(amount);
    if (amountCents === null) {
      setLocalError("Informe o valor em reais, como 1.234,56.");
      return;
    }
    if (repeat === "installments" && !/^\d+$/.test(count)) {
      setLocalError("Informe o número de parcelas.");
      return;
    }
    setLocalError(null);
    save.mutate(amountCents);
  }

  const error = localError ?? (save.error && errorMessage(save.error));

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
            <Field
              label={repeat === "installments" ? "Valor total (R$)" : "Valor (R$)"}
              htmlFor="entry-amount"
            >
              <Input
                id="entry-amount"
                autoFocus
                inputMode="decimal"
                placeholder="0,00"
                value={amount}
                onChange={(e) => setAmount(e.target.value)}
              />
            </Field>
            <Field
              label={repeat === "none" ? "Data" : "Primeira data"}
              htmlFor="entry-date"
            >
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

          {!entry && (
            <div className="grid grid-cols-2 gap-3">
              <Field label="Repetição" htmlFor="entry-repeat">
                <NativeSelect
                  id="entry-repeat"
                  className="w-full"
                  value={repeat}
                  onChange={(e) => setRepeat(e.target.value as Repeat)}
                >
                  <NativeSelectOption value="none">Não repete</NativeSelectOption>
                  <NativeSelectOption value="monthly">Todo mês</NativeSelectOption>
                  {kind === "expense" && (
                    <NativeSelectOption value="installments">
                      Parcelado
                    </NativeSelectOption>
                  )}
                </NativeSelect>
              </Field>
              {repeat === "monthly" && (
                <Field label="Até (opcional)" htmlFor="entry-end">
                  <Input
                    id="entry-end"
                    type="date"
                    min={date}
                    value={endDate}
                    onChange={(e) => setEndDate(e.target.value)}
                  />
                </Field>
              )}
              {repeat === "installments" && (
                <Field label="Parcelas" htmlFor="entry-count">
                  <Input
                    id="entry-count"
                    inputMode="numeric"
                    value={count}
                    onChange={(e) => setCount(e.target.value)}
                  />
                </Field>
              )}
            </div>
          )}

          {repeat === "none" ? (
            <div className="flex items-center gap-2">
              <Checkbox
                id="entry-settled"
                checked={settled}
                onCheckedChange={setSettledChoice}
              />
              <Label htmlFor="entry-settled" className="font-normal">
                Já aconteceu (efetivado)
              </Label>
            </div>
          ) : (
            <p className="text-sm text-muted-foreground">
              {repeat === "monthly"
                ? "Cada mês entra como previsto, e você efetiva quando acontecer."
                : "Cada parcela entra como prevista, e você efetiva quando for paga."}
            </p>
          )}

          {entry?.recurrenceId && (
            <div className="flex items-center gap-2">
              <Checkbox
                id="entry-following"
                checked={following}
                onCheckedChange={setFollowing}
              />
              <Label htmlFor="entry-following" className="font-normal">
                Aplicar também aos próximos previstos desta recorrência
              </Label>
            </div>
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
