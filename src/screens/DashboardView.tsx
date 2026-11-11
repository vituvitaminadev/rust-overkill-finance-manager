import { useState } from "react";
import { PlusIcon } from "lucide-react";
import { BarList } from "@/components/charts/BarList";
import { FlowChart } from "@/components/charts/FlowChart";
import { EntryDialog } from "@/components/EntryDialog";
import { FormError } from "@/components/Field";
import { MonthPicker } from "@/components/MonthPicker";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { errorMessage, type Member } from "@/lib/api";
import { currentMonth, formatMoney } from "@/lib/format";
import { useDashboard } from "@/lib/queries";

export function DashboardView({ me }: { me: Member }) {
  const [month, setMonth] = useState(currentMonth);
  const [adding, setAdding] = useState(false);
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
            <StatTile label="Receitas do mês" cents={data.incomeCents} />
            <StatTile label="Despesas do mês" cents={data.expenseCents} />
            <StatTile label="Saldo do mês" cents={data.balanceCents} />
          </div>

          <Card>
            <CardHeader>
              <CardTitle>Fluxo mensal</CardTitle>
              <CardDescription>
                Os seis meses até o mês selecionado.
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
              </CardHeader>
              <CardContent>
                <BarList
                  empty="Nenhuma despesa neste mês."
                  items={data.expensesByCategory.map((item) => ({
                    id: item.categoryId,
                    name: item.name,
                    totalCents: item.totalCents,
                  }))}
                />
              </CardContent>
            </Card>
            <Card>
              <CardHeader>
                <CardTitle>Despesas por responsável</CardTitle>
              </CardHeader>
              <CardContent>
                <BarList
                  empty="Nenhuma despesa neste mês."
                  items={data.expensesByAssignee.map((item) => ({
                    id: item.memberId,
                    name: item.name,
                    totalCents: item.totalCents,
                  }))}
                />
              </CardContent>
            </Card>
          </div>
        </div>
      )}

      {adding && <EntryDialog me={me} onClose={() => setAdding(false)} />}
    </div>
  );
}

function StatTile({ label, cents }: { label: string; cents: number }) {
  return (
    <Card>
      <CardContent className="grid gap-1">
        <span className="text-sm text-muted-foreground">{label}</span>
        <span className="text-2xl font-semibold">{formatMoney(cents)}</span>
      </CardContent>
    </Card>
  );
}
