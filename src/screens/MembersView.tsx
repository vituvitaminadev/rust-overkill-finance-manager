import { type FormEvent, useState } from "react";
import { Field, FormError } from "@/components/Field";
import { Badge } from "@/components/ui/badge";
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
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { api, errorMessage, type Member } from "@/lib/api";
import { useHouseholdMutation, useMembers } from "@/lib/queries";

/** Quem faz parte da residência e a senha de quem está usando o app. */
export function MembersView({ me }: { me: Member }) {
  return (
    <div className="grid items-start gap-4 lg:grid-cols-2">
      <MembersCard me={me} />
      <PasswordCard />
    </div>
  );
}

function MembersCard({ me }: { me: Member }) {
  const members = useMembers();
  const isAdmin = me.role === "admin";
  const [resetting, setResetting] = useState<Member | null>(null);
  const [removing, setRemoving] = useState<Member | null>(null);
  const [name, setName] = useState("");
  const add = useHouseholdMutation(
    () => api.addMember(name),
    () => setName(""),
  );
  const setRole = useHouseholdMutation((member: Member) =>
    api.setMemberRole(member.id, member.role === "admin" ? "common" : "admin"),
  );
  const error = members.error ?? setRole.error ?? add.error;

  function submit(event: FormEvent) {
    event.preventDefault();
    add.mutate(undefined);
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>Participantes</CardTitle>
        <CardDescription>
          {isAdmin
            ? "Cada participante pode ser o responsável por lançamentos. Só entra no app quem tiver uma senha definida."
            : "Só administradores adicionam, promovem e removem participantes."}
        </CardDescription>
      </CardHeader>
      <CardContent className="grid gap-3">
        <ul className="divide-y text-sm">
          {(members.data ?? []).map((member) => (
            <li key={member.id} className="flex min-h-11 flex-wrap items-center gap-2 py-1">
              <span className={member.active ? undefined : "text-muted-foreground"}>
                {member.username}
                {member.id === me.id && " (você)"}
              </span>
              {member.active ? (
                <Badge variant="secondary">
                  {member.role === "admin" ? "Administrador" : "Comum"}
                </Badge>
              ) : (
                <Badge variant="outline">Removido</Badge>
              )}
              {isAdmin && member.active && (
                <span className="ml-auto flex flex-wrap gap-1">
                  <Button
                    variant="ghost"
                    size="sm"
                    disabled={setRole.isPending}
                    onClick={() => setRole.mutate(member)}
                  >
                    {member.role === "admin" ? "Tornar comum" : "Tornar administrador"}
                  </Button>
                  {member.id !== me.id && (
                    <>
                      <Button variant="ghost" size="sm" onClick={() => setResetting(member)}>
                        Definir senha
                      </Button>
                      <Button variant="ghost" size="sm" onClick={() => setRemoving(member)}>
                        Remover
                      </Button>
                    </>
                  )}
                </span>
              )}
            </li>
          ))}
        </ul>
        {isAdmin && (
          <form onSubmit={submit} className="flex gap-2">
            <Input
              aria-label="Nome do novo participante"
              placeholder="Nome do novo participante"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
            <Button type="submit" variant="outline" disabled={add.isPending}>
              Adicionar
            </Button>
          </form>
        )}
        <FormError error={error && errorMessage(error)} />
      </CardContent>
      {resetting && (
        <ResetPasswordDialog member={resetting} onClose={() => setResetting(null)} />
      )}
      {removing && <RemoveDialog member={removing} onClose={() => setRemoving(null)} />}
    </Card>
  );
}

function ResetPasswordDialog({ member, onClose }: { member: Member; onClose: () => void }) {
  const [password, setPassword] = useState("");
  const reset = useHouseholdMutation(
    () => api.resetPassword(member.id, password),
    onClose,
  );

  function submit(event: FormEvent) {
    event.preventDefault();
    reset.mutate(undefined);
  }

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent>
        <form onSubmit={submit} className="grid gap-4">
          <DialogHeader>
            <DialogTitle>Definir a senha de {member.username}</DialogTitle>
            <DialogDescription>
Com uma senha, {member.username} passa a poder entrar no app. A
              pessoa pode trocá-la depois, em Participantes.
            </DialogDescription>
          </DialogHeader>
          <Field label="Nova senha (mínimo de 8 caracteres)" htmlFor="reset-password">
            <Input
              id="reset-password"
              type="password"
              autoFocus
              autoComplete="new-password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
            />
          </Field>
          <FormError error={reset.error && errorMessage(reset.error)} />
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              Cancelar
            </Button>
            <Button type="submit" disabled={reset.isPending}>
              Definir
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function RemoveDialog({ member, onClose }: { member: Member; onClose: () => void }) {
  const remove = useHouseholdMutation(() => api.removeMember(member.id), onClose);
  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Remover {member.username}?</DialogTitle>
          <DialogDescription>
            A pessoa perde o acesso à residência. Os lançamentos dela continuam
            existindo e atribuídos a ela.
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
            Remover
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function PasswordCard() {
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [done, setDone] = useState(false);
  const change = useHouseholdMutation(
    () => api.changePassword(current, next),
    () => {
      setCurrent("");
      setNext("");
      setDone(true);
    },
  );

  function submit(event: FormEvent) {
    event.preventDefault();
    setDone(false);
    change.mutate(undefined);
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>Sua senha</CardTitle>
      </CardHeader>
      <CardContent>
        <form onSubmit={submit} className="grid gap-3">
          <Field label="Senha atual" htmlFor="current-password">
            <Input
              id="current-password"
              type="password"
              autoComplete="current-password"
              value={current}
              onChange={(e) => setCurrent(e.target.value)}
            />
          </Field>
          <Field label="Nova senha (mínimo de 8 caracteres)" htmlFor="new-password">
            <Input
              id="new-password"
              type="password"
              autoComplete="new-password"
              value={next}
              onChange={(e) => setNext(e.target.value)}
            />
          </Field>
          <FormError error={change.error && errorMessage(change.error)} />
          {done && <p className="text-sm text-muted-foreground">Senha alterada.</p>}
          <Button type="submit" variant="outline" disabled={change.isPending}>
            Alterar senha
          </Button>
        </form>
      </CardContent>
    </Card>
  );
}
