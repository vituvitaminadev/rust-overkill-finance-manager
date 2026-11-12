import { useState } from "react";
import { CheckIcon, PencilIcon, PlusIcon, Trash2Icon } from "lucide-react";
import { EntryDialog } from "@/components/EntryDialog";
import { EntryDescription, EntryStatusBadge } from "@/components/EntryLabels";
import { FormError } from "@/components/Field";
import { MonthPicker } from "@/components/MonthPicker";
import { SettleDialog } from "@/components/SettleDialog";
import { Badge } from "@/components/ui/badge";
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
import {
  NativeSelect,
  NativeSelectOption,
} from "@/components/ui/native-select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  api,
  errorMessage,
  type Entry,
  type EntryKind,
  type EntryStatus,
  type Member,
} from "@/lib/api";
import { currentMonth, formatDate, formatMoney } from "@/lib/format";
import {
  useCategories,
  useEntries,
  useHouseholdMutation,
  useMembers,
} from "@/lib/queries";

export function EntriesView({ me }: { me: Member }) {
  const [month, setMonth] = useState(currentMonth);
  const [kind, setKind] = useState<EntryKind | "">("");
  const [status, setStatus] = useState<EntryStatus | "">("");
  const [categoryId, setCategoryId] = useState("");
  const [assigneeId, setAssigneeId] = useState("");
  const [search, setSearch] = useState("");
  const [adding, setAdding] = useState(false);
  const [editing, setEditing] = useState<Entry | null>(null);
  const [deleting, setDeleting] = useState<Entry | null>(null);
  const [settling, setSettling] = useState<Entry | null>(null);

  const categories = useCategories();
  const members = useMembers();
  const entries = useEntries({
    month,
    kind: kind || null,
    status: status || null,
    categoryId: categoryId || null,
    assigneeId: assigneeId || null,
    search: search || null,
  });
  const rows = entries.data ?? [];
  const filtered = Boolean(
    kind || status || categoryId || assigneeId || search,
  );

  return (
    <div className="grid gap-4">
      <div className="flex flex-wrap items-center gap-2">
        <MonthPicker month={month} onChange={setMonth} />
        <NativeSelect
          aria-label="Tipo"
          value={kind}
          onChange={(e) => setKind(e.target.value as EntryKind | "")}
        >
          <NativeSelectOption value="">Todos os tipos</NativeSelectOption>
          <NativeSelectOption value="expense">Despesas</NativeSelectOption>
          <NativeSelectOption value="income">Receitas</NativeSelectOption>
        </NativeSelect>
        <NativeSelect
          aria-label="Situação"
          value={status}
          onChange={(e) => setStatus(e.target.value as EntryStatus | "")}
        >
          <NativeSelectOption value="">Todas as situações</NativeSelectOption>
          <NativeSelectOption value="planned">Previstos</NativeSelectOption>
          <NativeSelectOption value="settled">Efetivados</NativeSelectOption>
        </NativeSelect>
        <NativeSelect
          aria-label="Categoria"
          value={categoryId}
          onChange={(e) => setCategoryId(e.target.value)}
        >
          <NativeSelectOption value="">Todas as categorias</NativeSelectOption>
          {(categories.data ?? [])
            .filter((category) => !kind || category.kind === kind)
            .map((category) => (
              <NativeSelectOption key={category.id} value={category.id}>
                {category.name}
              </NativeSelectOption>
            ))}
        </NativeSelect>
        <NativeSelect
          aria-label="Responsável"
          value={assigneeId}
          onChange={(e) => setAssigneeId(e.target.value)}
        >
          <NativeSelectOption value="">Todos os responsáveis</NativeSelectOption>
          {(members.data ?? []).map((member) => (
            <NativeSelectOption key={member.id} value={member.id}>
              {member.username}
            </NativeSelectOption>
          ))}
        </NativeSelect>
        <Input
          type="search"
          aria-label="Buscar pela descrição"
          placeholder="Buscar pela descrição"
          className="w-52"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
        <Button className="ml-auto" onClick={() => setAdding(true)}>
          <PlusIcon />
          Novo lançamento
        </Button>
      </div>

      <FormError error={entries.error && errorMessage(entries.error)} />

      {entries.data && rows.length === 0 ? (
        <p className="py-12 text-center text-sm text-muted-foreground">
          {filtered
            ? "Nenhum lançamento corresponde aos filtros."
            : "Nenhum lançamento neste mês."}
        </p>
      ) : (
        <div
          className={`rounded-xl border transition-opacity ${
            entries.isPlaceholderData ? "opacity-60" : ""
          }`}
        >
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Data</TableHead>
                <TableHead>Descrição</TableHead>
                <TableHead>Categoria</TableHead>
                <TableHead>Responsável</TableHead>
                <TableHead>Tipo</TableHead>
                <TableHead>Situação</TableHead>
                <TableHead className="text-right">Valor</TableHead>
                <TableHead className="w-28" />
              </TableRow>
            </TableHeader>
            <TableBody>
              {rows.map((entry) => (
                <TableRow key={entry.id}>
                  <TableCell className="tabular-nums">
                    {formatDate(entry.date)}
                  </TableCell>
                  <TableCell className="max-w-64">
                    <EntryDescription entry={entry} />
                  </TableCell>
                  <TableCell>{entry.categoryName}</TableCell>
                  <TableCell>{entry.assigneeName}</TableCell>
                  <TableCell>
                    <Badge
                      variant={entry.kind === "income" ? "default" : "outline"}
                    >
                      {entry.kind === "income" ? "Receita" : "Despesa"}
                    </Badge>
                  </TableCell>
                  <TableCell>
                    <EntryStatusBadge entry={entry} />
                  </TableCell>
                  <TableCell className="text-right font-medium tabular-nums">
                    {entry.kind === "expense" ? "−" : "+"}
                    {formatMoney(entry.amountCents)}
                  </TableCell>
                  <TableCell className="text-right">
                    {entry.canEdit && (
                      <>
                        {entry.status === "planned" && (
                          <Button
                            variant="ghost"
                            size="icon-sm"
                            aria-label={`Efetivar ${entry.description}`}
                            title="Efetivar"
                            onClick={() => setSettling(entry)}
                          >
                            <CheckIcon />
                          </Button>
                        )}
                        <Button
                          variant="ghost"
                          size="icon-sm"
                          aria-label={`Editar ${entry.description}`}
                          onClick={() => setEditing(entry)}
                        >
                          <PencilIcon />
                        </Button>
                        <Button
                          variant="ghost"
                          size="icon-sm"
                          aria-label={`Excluir ${entry.description}`}
                          onClick={() => setDeleting(entry)}
                        >
                          <Trash2Icon />
                        </Button>
                      </>
                    )}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>
      )}

      {adding && <EntryDialog me={me} onClose={() => setAdding(false)} />}
      {editing && (
        <EntryDialog
          me={me}
          entry={editing}
          onClose={() => setEditing(null)}
        />
      )}
      {deleting && (
        <DeleteDialog entry={deleting} onClose={() => setDeleting(null)} />
      )}
      {settling && (
        <SettleDialog entry={settling} onClose={() => setSettling(null)} />
      )}
    </div>
  );
}

function DeleteDialog({
  entry,
  onClose,
}: {
  entry: Entry;
  onClose: () => void;
}) {
  const remove = useHouseholdMutation(() => api.deleteEntry(entry.id), onClose);

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Excluir lançamento?</DialogTitle>
          <DialogDescription>
            {entry.description}, {formatMoney(entry.amountCents)}, em{" "}
            {formatDate(entry.date)}. Esta ação não pode ser desfeita.
          </DialogDescription>
        </DialogHeader>
        <FormError error={remove.error && errorMessage(remove.error)} />
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            Cancelar
          </Button>
          <Button
            variant="destructive"
            disabled={remove.isPending}
            onClick={() => remove.mutate(undefined)}
          >
            Excluir
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
