import { useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { api, type HouseholdInfo, type Member } from "@/lib/api";
import { useSessionAction } from "@/lib/session";
import { CategoriesView } from "./CategoriesView";
import { DashboardView } from "./DashboardView";
import { EntriesView } from "./EntriesView";
import { MembersView } from "./MembersView";
import { PlanningView } from "./PlanningView";

const TABS = [
  { id: "dashboard", label: "Resumo" },
  { id: "entries", label: "Lançamentos" },
  { id: "planning", label: "Recorrências e parcelas" },
  { id: "categories", label: "Categorias" },
  { id: "members", label: "Participantes" },
] as const;

type TabId = (typeof TABS)[number]["id"];

/** A residência aberta: cabeçalho, navegação e a tela escolhida. */
export function Workspace({
  household,
  me,
}: {
  household: HouseholdInfo;
  me: Member;
}) {
  const [tab, setTab] = useState<TabId>("dashboard");
  const close = useSessionAction(api.closeHousehold);
  const logout = useSessionAction(api.logout);

  return (
    <div className="min-h-screen">
      <header className="sticky top-0 z-10 border-b bg-background">
        <div className="mx-auto flex max-w-6xl flex-wrap items-center gap-x-6 gap-y-2 px-4 py-2.5">
          <h1 className="font-heading text-base font-semibold">
            {household.name}
          </h1>
          <nav className="flex gap-1">
            {TABS.map((item) => (
              <Button
                key={item.id}
                variant={tab === item.id ? "secondary" : "ghost"}
                aria-current={tab === item.id ? "page" : undefined}
                onClick={() => setTab(item.id)}
              >
                {item.label}
              </Button>
            ))}
          </nav>
          <div className="ml-auto flex flex-wrap items-center gap-2 text-sm">
            <span>{me.username}</span>
            <Badge variant="secondary">
              {me.role === "admin" ? "Administrador" : "Comum"}
            </Badge>
            <Button variant="ghost" onClick={() => close.mutate(undefined)}>
              Trocar residência
            </Button>
            <Button variant="ghost" onClick={() => logout.mutate(undefined)}>
              Sair
            </Button>
          </div>
        </div>
      </header>
      <main className="mx-auto max-w-6xl p-4">
        {tab === "dashboard" && <DashboardView me={me} />}
        {tab === "entries" && <EntriesView me={me} />}
        {tab === "planning" && <PlanningView />}
        {tab === "categories" && <CategoriesView me={me} />}
        {tab === "members" && <MembersView me={me} />}
      </main>
    </div>
  );
}
