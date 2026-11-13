import { type FormEvent, useState } from "react";
import { Field, FormError } from "@/components/Field";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { api, errorMessage } from "@/lib/api";
import { useSessionAction } from "@/lib/session";

/** Entrada no app: login, ou a criação da primeira residência desta máquina. */
export function AuthScreen({ hasHouseholds }: { hasHouseholds: boolean }) {
  const [mode, setMode] = useState<"login" | "create">(
    hasHouseholds ? "login" : "create",
  );

  return (
    <main className="flex min-h-screen items-center justify-center p-4">
      <Card className="w-full max-w-sm">
        {mode === "login" ? (
          <LoginForm onCreate={() => setMode("create")} />
        ) : (
          <CreateForm onLogin={hasHouseholds ? () => setMode("login") : null} />
        )}
      </Card>
    </main>
  );
}

function RememberMe({
  checked,
  onChange,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <div className="flex items-center gap-2">
      <Checkbox id="remember" checked={checked} onCheckedChange={onChange} />
      <Label htmlFor="remember" className="font-normal">
        Manter conectado neste computador
      </Label>
    </div>
  );
}

function LoginForm({ onCreate }: { onCreate: () => void }) {
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [remember, setRemember] = useState(true);
  const login = useSessionAction(() => api.login(username, password, remember));

  function submit(event: FormEvent) {
    event.preventDefault();
    login.mutate(undefined);
  }

  return (
    <>
      <CardHeader>
        <CardTitle>Finanças da Residência</CardTitle>
        <CardDescription>Entre com seu usuário e senha.</CardDescription>
      </CardHeader>
      <CardContent>
        <form onSubmit={submit} className="grid gap-4">
          <Field label="Usuário" htmlFor="username">
            <Input
              id="username"
              autoFocus
              autoComplete="username"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
            />
          </Field>
          <Field label="Senha" htmlFor="password">
            <Input
              id="password"
              type="password"
              autoComplete="current-password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
            />
          </Field>
          <RememberMe checked={remember} onChange={setRemember} />
          <FormError error={login.error && errorMessage(login.error)} />
          <Button type="submit" disabled={login.isPending}>
            Entrar
          </Button>
          <Button type="button" variant="ghost" onClick={onCreate}>
            Criar uma residência com outro usuário
          </Button>
        </form>
      </CardContent>
    </>
  );
}

function CreateForm({ onLogin }: { onLogin: (() => void) | null }) {
  const [name, setName] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [remember, setRemember] = useState(true);
  const [mismatch, setMismatch] = useState(false);
  const create = useSessionAction(() =>
    api.createHousehold({ name, username, password, remember }),
  );

  function submit(event: FormEvent) {
    event.preventDefault();
    const differs = password !== confirmation;
    setMismatch(differs);
    if (!differs) create.mutate(undefined);
  }

  const error = mismatch
    ? "As senhas não são iguais."
    : create.error && errorMessage(create.error);

  return (
    <>
      <CardHeader>
        <CardTitle>Criar residência</CardTitle>
        <CardDescription>
          Você será o administrador, e este computador guardará os dados dela.
        </CardDescription>
      </CardHeader>
      <CardContent>
        <form onSubmit={submit} className="grid gap-4">
          <Field label="Nome da residência" htmlFor="household-name">
            <Input
              id="household-name"
              autoFocus
              placeholder="Nossa casa"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </Field>
          <Field label="Seu usuário" htmlFor="username">
            <Input
              id="username"
              autoComplete="username"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
            />
          </Field>
          <Field label="Senha (mínimo de 8 caracteres)" htmlFor="password">
            <Input
              id="password"
              type="password"
              autoComplete="new-password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
            />
          </Field>
          <Field label="Repita a senha" htmlFor="confirmation">
            <Input
              id="confirmation"
              type="password"
              autoComplete="new-password"
              value={confirmation}
              onChange={(e) => setConfirmation(e.target.value)}
            />
          </Field>
          <RememberMe checked={remember} onChange={setRemember} />
          <FormError error={error || null} />
          <Button type="submit" disabled={create.isPending}>
            Criar residência
          </Button>
          {onLogin && (
            <Button type="button" variant="ghost" onClick={onLogin}>
              Já tenho usuário
            </Button>
          )}
        </form>
      </CardContent>
    </>
  );
}
