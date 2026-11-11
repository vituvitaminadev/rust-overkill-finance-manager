import { type FormEvent, useState } from "react";
import { FormError } from "@/components/Field";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import {
  api,
  errorMessage,
  type Category,
  type EntryKind,
  type Member,
} from "@/lib/api";
import { useCategories, useHouseholdMutation } from "@/lib/queries";

const GROUPS: { kind: EntryKind; title: string }[] = [
  { kind: "expense", title: "Despesas" },
  { kind: "income", title: "Receitas" },
];

export function CategoriesView({ me }: { me: Member }) {
  const categories = useCategories();
  const isAdmin = me.role === "admin";

  return (
    <div className="grid gap-4">
      {!isAdmin && (
        <p className="text-sm text-muted-foreground">
          Só administradores criam, renomeiam e arquivam categorias.
        </p>
      )}
      <FormError error={categories.error && errorMessage(categories.error)} />
      <div className="grid items-start gap-4 lg:grid-cols-2">
        {GROUPS.map((group) => (
          <CategoryGroup
            key={group.kind}
            kind={group.kind}
            title={group.title}
            isAdmin={isAdmin}
            categories={(categories.data ?? []).filter(
              (category) => category.kind === group.kind,
            )}
          />
        ))}
      </div>
    </div>
  );
}

function CategoryGroup({
  kind,
  title,
  isAdmin,
  categories,
}: {
  kind: EntryKind;
  title: string;
  isAdmin: boolean;
  categories: Category[];
}) {
  const [name, setName] = useState("");
  const [renaming, setRenaming] = useState<Category | null>(null);
  const create = useHouseholdMutation(
    () => api.createCategory({ name, kind }),
    () => setName(""),
  );
  const archive = useHouseholdMutation((category: Category) =>
    api.setCategoryArchived(category.id, !category.archived),
  );

  function submit(event: FormEvent) {
    event.preventDefault();
    create.mutate(undefined);
  }

  const error = create.error ?? archive.error;

  return (
    <Card>
      <CardHeader>
        <CardTitle>{title}</CardTitle>
      </CardHeader>
      <CardContent className="grid gap-3">
        <ul className="divide-y">
          {categories.map((category) => (
            <li
              key={category.id}
              className="flex min-h-10 items-center gap-2 py-1 text-sm"
            >
              <span
                className={
                  category.archived ? "text-muted-foreground" : undefined
                }
              >
                {category.name}
              </span>
              {category.archived && <Badge variant="secondary">Arquivada</Badge>}
              {isAdmin && (
                <span className="ml-auto flex gap-1">
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => setRenaming(category)}
                  >
                    Renomear
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    disabled={archive.isPending}
                    onClick={() => archive.mutate(category)}
                  >
                    {category.archived ? "Restaurar" : "Arquivar"}
                  </Button>
                </span>
              )}
            </li>
          ))}
        </ul>

        {isAdmin && (
          <form onSubmit={submit} className="flex gap-2">
            <Input
              aria-label={`Nova categoria de ${title.toLowerCase()}`}
              placeholder="Nova categoria"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
            <Button type="submit" variant="outline" disabled={create.isPending}>
              Adicionar
            </Button>
          </form>
        )}
        <FormError error={error && errorMessage(error)} />
      </CardContent>
      {renaming && (
        <RenameDialog category={renaming} onClose={() => setRenaming(null)} />
      )}
    </Card>
  );
}

function RenameDialog({
  category,
  onClose,
}: {
  category: Category;
  onClose: () => void;
}) {
  const [name, setName] = useState(category.name);
  const rename = useHouseholdMutation(
    () => api.renameCategory(category.id, name),
    onClose,
  );

  function submit(event: FormEvent) {
    event.preventDefault();
    rename.mutate(undefined);
  }

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent>
        <form onSubmit={submit} className="grid gap-4">
          <DialogHeader>
            <DialogTitle>Renomear categoria</DialogTitle>
          </DialogHeader>
          <Input
            aria-label="Nome da categoria"
            autoFocus
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
          <FormError error={rename.error && errorMessage(rename.error)} />
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              Cancelar
            </Button>
            <Button type="submit" disabled={rename.isPending}>
              Salvar
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
