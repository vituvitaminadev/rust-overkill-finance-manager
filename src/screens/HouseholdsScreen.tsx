import { type FormEvent, useState } from "react";
import { ChevronRightIcon } from "lucide-react";
import { Field, FormError } from "@/components/Field";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
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
import { api, errorMessage, type Status } from "@/lib/api";
import { useSessionAction } from "@/lib/session";

/** As residências de quem está conectado, com a opção de criar mais uma. */
export function HouseholdsScreen({ status }: { status: Status }) {
  const [creating, setCreating] = useState(false);
  const select = useSessionAction(api.selectHousehold);
  const logout = useSessionAction(api.logout);

  return (
    <main className="flex min-h-screen items-center justify-center p-4">
      <Card className="w-full max-w-sm">
        <CardHeader>
          <CardTitle>Suas residências</CardTitle>
          <CardDescription>Conectado como {status.username}.</CardDescription>
        </CardHeader>
        <CardContent className="grid gap-2">
          {status.households.map((household) => (
            <Button
              key={household.id}
              variant="outline"
              className="h-11 justify-between"
              disabled={select.isPending}
              onClick={() => select.mutate(household.id)}
            >
              {household.name}
              <ChevronRightIcon />
            </Button>
          ))}
          <FormError error={select.error && errorMessage(select.error)} />
          <Button className="mt-2" onClick={() => setCreating(true)}>
            Nova residência
          </Button>
          <Button variant="ghost" onClick={() => logout.mutate(undefined)}>
            Sair
          </Button>
        </CardContent>
      </Card>
      {creating && <NewHouseholdDialog onClose={() => setCreating(false)} />}
    </main>
  );
}

function NewHouseholdDialog({ onClose }: { onClose: () => void }) {
  const [name, setName] = useState("");
  const create = useSessionAction(() =>
    api.createHousehold({ name, remember: true }),
  );

  function submit(event: FormEvent) {
    event.preventDefault();
    create.mutate(undefined);
  }

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent>
        <form onSubmit={submit} className="grid gap-4">
          <DialogHeader>
            <DialogTitle>Nova residência</DialogTitle>
          </DialogHeader>
          <Field label="Nome da residência" htmlFor="new-household-name">
            <Input
              id="new-household-name"
              autoFocus
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </Field>
          <FormError error={create.error && errorMessage(create.error)} />
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              Cancelar
            </Button>
            <Button type="submit" disabled={create.isPending}>
              Criar
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
