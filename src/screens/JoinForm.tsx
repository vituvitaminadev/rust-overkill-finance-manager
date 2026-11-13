import { type FormEvent, useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { ChevronRightIcon } from "lucide-react";
import { Field, FormError } from "@/components/Field";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { api, errorMessage, type HostPreview } from "@/lib/api";
import { useSessionAction } from "@/lib/session";

/**
 * Entrar numa residência hospedada em outro computador: achar o Anfitrião,
 * conferir que é ele mesmo e apresentar o Convite.
 */
export function JoinForm({ onCancel }: { onCancel: () => void }) {
  const [host, setHost] = useState<{ address: string; preview: HostPreview } | null>(
    null,
  );
  return host ? (
    <CredentialsStep host={host} onBack={() => setHost(null)} />
  ) : (
    <FindHostStep onFound={setHost} onCancel={onCancel} />
  );
}

function FindHostStep({
  onFound,
  onCancel,
}: {
  onFound: (host: { address: string; preview: HostPreview }) => void;
  onCancel: () => void;
}) {
  const [address, setAddress] = useState("");
  // A rede leva um instante para anunciar os Anfitriões, então a lista se refaz sozinha.
  const discovered = useQuery({
    queryKey: ["discover"],
    queryFn: api.discover,
    refetchInterval: 2000,
  });
  const inspect = useMutation({
    mutationFn: async (target: string) => ({
      address: target,
      preview: await api.inspectHost(target),
    }),
    onSuccess: onFound,
  });
  const hosts = discovered.data ?? [];

  function submit(event: FormEvent) {
    event.preventDefault();
    inspect.mutate(address);
  }

  return (
    <div className="grid gap-4">
      <div className="grid gap-2">
        <span className="text-sm font-medium">Residências nesta rede</span>
        {hosts.length === 0 ? (
          <p className="text-sm text-muted-foreground">
            Procurando… O app precisa estar aberto no computador que hospeda a
            residência, na mesma rede.
          </p>
        ) : (
          hosts.map((found) => (
            <Button
              key={found.householdId}
              variant="outline"
              className="h-11 justify-between"
              disabled={inspect.isPending}
              onClick={() => inspect.mutate(found.address)}
            >
              {found.name}
              <ChevronRightIcon />
            </Button>
          ))
        )}
      </div>
      <form onSubmit={submit} className="grid gap-2">
        <Field label="Ou informe o endereço do Anfitrião" htmlFor="host-address">
          <div className="flex gap-2">
            <Input
              id="host-address"
              placeholder="192.168.0.10:47800"
              value={address}
              onChange={(e) => setAddress(e.target.value)}
            />
            <Button type="submit" variant="outline" disabled={inspect.isPending}>
              Conectar
            </Button>
          </div>
        </Field>
      </form>
      <FormError error={inspect.error && errorMessage(inspect.error)} />
      <Button variant="ghost" onClick={onCancel}>
        Voltar
      </Button>
    </div>
  );
}

function CredentialsStep({
  host,
  onBack,
}: {
  host: { address: string; preview: HostPreview };
  onBack: () => void;
}) {
  const [code, setCode] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [remember, setRemember] = useState(true);
  const [mismatch, setMismatch] = useState(false);
  const join = useSessionAction(() =>
    api.joinHousehold({
      address: host.address,
      fingerprint: host.preview.fingerprint,
      code,
      username,
      password,
      remember,
    }),
  );

  function submit(event: FormEvent) {
    event.preventDefault();
    const differs = password !== confirmation;
    setMismatch(differs);
    if (!differs) join.mutate(undefined);
  }

  const error = mismatch
    ? "As senhas não são iguais."
    : join.error && errorMessage(join.error);

  return (
    <form onSubmit={submit} className="grid gap-4">
      <div className="grid gap-1 rounded-lg bg-muted/50 p-3 text-sm">
        <span>
          Residência: <strong>{host.preview.household.name}</strong>
        </span>
        <span>
          Código de segurança:{" "}
          <strong className="font-mono">{host.preview.securityCode}</strong>
        </span>
        <span className="text-muted-foreground">
          Confira se é o mesmo código mostrado no Anfitrião, em Membros. Se for
          diferente, não continue.
        </span>
      </div>
      <Field label="Código do convite" htmlFor="invite-code">
        <Input
          id="invite-code"
          autoFocus
          className="font-mono uppercase"
          value={code}
          onChange={(e) => setCode(e.target.value)}
        />
      </Field>
      <Field label="Seu usuário nesta residência" htmlFor="join-username">
        <Input
          id="join-username"
          autoComplete="username"
          value={username}
          onChange={(e) => setUsername(e.target.value)}
        />
      </Field>
      <Field label="Senha (mínimo de 8 caracteres)" htmlFor="join-password">
        <Input
          id="join-password"
          type="password"
          autoComplete="new-password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
        />
      </Field>
      <Field label="Repita a senha" htmlFor="join-confirmation">
        <Input
          id="join-confirmation"
          type="password"
          autoComplete="new-password"
          value={confirmation}
          onChange={(e) => setConfirmation(e.target.value)}
        />
      </Field>
      <div className="flex items-center gap-2">
        <Checkbox id="join-remember" checked={remember} onCheckedChange={setRemember} />
        <Label htmlFor="join-remember" className="font-normal">
          Manter conectado neste computador
        </Label>
      </div>
      <FormError error={error || null} />
      <Button type="submit" disabled={join.isPending}>
        Entrar na residência
      </Button>
      <Button type="button" variant="ghost" onClick={onBack}>
        Escolher outro Anfitrião
      </Button>
    </form>
  );
}
