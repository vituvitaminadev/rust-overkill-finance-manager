import { useState } from "react";
import { PlusIcon } from "lucide-react";
import { BarList, PlannedKey } from "@/components/charts/BarList";
import { FlowChart } from "@/components/charts/FlowChart";
import { EntryDialog } from "@/components/EntryDialog";
import { EntryDescription } from "@/components/EntryLabels";
import { FormError } from "@/components/Field";
import { MonthPicker } from "@/components/MonthPicker";
import { SettleDialog } from "@/components/SettleDialog";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { errorMessage, type Entry, type Member, type Totals } from "@/lib/api";
import { currentMonth, formatDate, formatMoney } from "@/lib/format";
import { useDashboard } from "@/lib/queries";

export function DashboardView({ me }: { me: Member }) {
  const [month, setMonth] = useState(currentMonth);
  const [adding, setAdding] = useState(false);
  const [settling, setSettling] = useState<Entry | null>(null);
  const dashboard = useDashboard(month);
  const data = dashboard.data;

  return (
    <div className="grid gap-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <MonthPicker month={month} onChange={setMonth} />
        <Button onClick={() => setAdding(true)}>
          <PlusIcon />
          Novo lançamento
        </Button>
      </div>

      <FormError error={dashboard.error && errorMessage(dashboard.error)} />

      {data && (
        // Enquanto outro mês carrega, os números anteriores ficam esmaecidos no lugar.
        <div
          className={`grid gap-4 transition-opacity ${
            dashboard.isPlaceholderData ? "opacity-60" : ""
          }`}
        >
          <div className="grid gap-4 sm:grid-cols-3">
            <StatTile
              label="Receitas do mês"
              totals={data.income}
              plannedLabel="a receber"
            />
            <StatTile
              label="Despesas do mês"
              totals={data.expense}
              plannedLabel="a pagar"
            />
            <BalanceTile totals={data.balance} />
          </div>

          <Card>
            <CardHeader>
              <CardTitle>Fluxo mensal</CardTitle>
              <CardDescription>
                Os cinco meses anteriores ao selecionado e os seis seguintes.
              </CardDescription>
            </CardHeader>
            <CardContent>
              <FlowChart flow={data.flow} />
            </CardContent>
          </Card>

          <div className="grid gap-4 lg:grid-cols-2">
            <Card>
              <CardHeader>
                <CardTitle>Despesas por categoria</CardTitle>
                <CardAction>
                  <PlannedKey />
                </CardAction>
              </CardHeader>
              <CardContent>
                <BarList
                  empty="Nenhuma despesa neste mês."
                  items={data.expensesByCategory.map((item) => ({
                    ...item,
                    id: item.categoryId,
                  }))}
                />
              </CardContent>
            </Card>
            <Card>
              <CardHeader>
                <CardTitle>Despesas por responsável</CardTitle>
                <CardAction>
                  <PlannedKey />
                </CardAction>
              </CardHeader>
              <CardContent>
                <BarList
                  empty="Nenhuma despesa neste mês."
                  items={data.expensesByAssignee.map((item) => ({
                    ...item,
                    id: item.memberId,
                  }))}
                />
              </CardContent>
            </Card>
          </div>

          <div className="grid items-start gap-4 lg:grid-cols-2">
            <PlannedList
              title="Atrasados"
              description="Previstos cuja data já passou. Nada é efetivado sozinho."
              empty="Nada atrasado."
              entries={data.overdue}
              onSettle={setSettling}
            />
            <PlannedList
              title="Próximos previstos"
              description="O que vem pela frente, a partir de hoje."
              empty="Nenhum lançamento previsto."
              entries={data.upcoming}
              onSettle={setSettling}
            />
          </div>
        </div>
      )}

      {adding && <EntryDialog me={me} onClose={() => setAdding(false)} />}
      {settling && (
        <SettleDialog entry={settling} onClose={() => setSettling(null)} />
      )}
    </div>
  );
}

function StatTile({
  label,
  totals,
  plannedLabel,
}: {
  label: string;
  totals: Totals;
  plannedLabel: string;
}) {
  return (
    <Card>
      <CardContent className="grid gap-1">
        <span className="text-sm text-muted-foreground">{label}</span>
        <span className="text-2xl font-semibold">
          {formatMoney(totals.settledCents)}
        </span>
        <span className="text-sm text-muted-foreground">
          + {formatMoney(totals.plannedCents)} {plannedLabel}
        </span>
      </CardContent>
    </Card>
  );
}

function BalanceTile({ totals }: { totals: Totals }) {
  return (
    <Card>
      <CardContent className="grid gap-1">
        <span className="text-sm text-muted-foreground">Saldo do mês</span>
        <span className="text-2xl font-semibold">
          {formatMoney(totals.settledCents)}
        </span>
        <span className="text-sm text-muted-foreground">
          {formatMoney(totals.settledCents + totals.plannedCents)} com os
          previstos
        </span>
      </CardContent>
    </Card>
  );
}

function PlannedList({
  title,
  description,
  empty,
  entries,
  onSettle,
}: {
  title: string;
  description: string;
  empty: string;
  entries: Entry[];
  onSettle: (entry: Entry) => void;
}) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>{title}</CardTitle>
        <CardDescription>{description}</CardDescription>
      </CardHeader>
      <CardContent>
        {entries.length === 0 ? (
          <p className="py-2 text-sm text-muted-foreground">{empty}</p>
        ) : (
          <ul className="divide-y text-sm">
            {entries.map((entry) => (
              <li key={entry.id} className="flex min-h-11 items-center gap-3 py-1">
                <span className="text-muted-foreground tabular-nums">
                  {formatDate(entry.date)}
                </span>
                <span className="grid min-w-0 flex-1">
                  <EntryDescription entry={entry} />
                  <span className="truncate text-xs text-muted-foreground">
                    {entry.categoryName} · {entry.assigneeName}
                  </span>
                </span>
                <span className="font-medium tabular-nums">
                  {entry.kind === "expense" ? "−" : "+"}
                  {formatMoney(entry.amountCents)}
                </span>
                {entry.canEdit && (
                  <Button
                    variant="outline"
                    size="sm"
                    aria-label={`Efetivar ${entry.description}`}
                    onClick={() => onSettle(entry)}
                  >
                    Efetivar
                  </Button>
                )}
              </li>
            ))}
          </ul>
        )}
      </CardContent>
    </Card>
  );
}
