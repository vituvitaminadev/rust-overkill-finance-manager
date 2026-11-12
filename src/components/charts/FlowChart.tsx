import {
  Bar,
  CartesianGrid,
  ComposedChart,
  Line,
  Rectangle,
  ReferenceLine,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import type { MonthFlow } from "@/lib/api";
import {
  formatCompactMoney,
  formatMoney,
  monthLabel,
  monthShortLabel,
} from "@/lib/format";
import { PlannedKey } from "./BarList";

const TICK = { fill: "var(--muted-foreground)", fontSize: 12 };
const PLANNED_OPACITY = 0.4;
const ROUNDED_TOP: [number, number, number, number] = [4, 4, 0, 0];

interface Row {
  month: string;
  incomeSettled: number;
  incomePlanned: number;
  expenseSettled: number;
  expensePlanned: number;
  accumulated: number;
}

/**
 * Receitas e despesas de cada mês, com o previsto empilhado em tom mais claro
 * sobre o efetivado, e o saldo acumulado no período.
 */
export function FlowChart({ flow }: { flow: MonthFlow[] }) {
  const rows: Row[] = flow.map((item) => ({
    month: item.month,
    incomeSettled: item.income.settledCents,
    incomePlanned: item.income.plannedCents,
    expenseSettled: item.expense.settledCents,
    expensePlanned: item.expense.plannedCents,
    accumulated: item.accumulatedBalanceCents,
  }));

  return (
    <div className="grid gap-3">
      <ul className="flex flex-wrap gap-x-4 gap-y-1 text-sm text-muted-foreground">
        <li className="flex items-center gap-1.5">
          <Swatch color="var(--series-1)" />
          Receitas
        </li>
        <li className="flex items-center gap-1.5">
          <Swatch color="var(--series-2)" />
          Despesas
        </li>
        <li>
          <PlannedKey />
        </li>
        <li className="flex items-center gap-1.5">
          <LineKey />
          Saldo acumulado
        </li>
      </ul>

      <div className="h-64">
        <ResponsiveContainer width="100%" height="100%">
          <ComposedChart
            data={rows}
            barGap={2}
            margin={{ top: 8, right: 8, bottom: 0, left: 0 }}
          >
            <CartesianGrid vertical={false} stroke="var(--chart-grid)" />
            <XAxis
              dataKey="month"
              tickFormatter={monthShortLabel}
              tick={TICK}
              tickLine={false}
              axisLine={{ stroke: "var(--chart-axis)" }}
            />
            <YAxis
              tickFormatter={formatCompactMoney}
              tick={TICK}
              tickLine={false}
              axisLine={false}
              width={56}
            />
            <ReferenceLine y={0} stroke="var(--chart-axis)" />
            <Tooltip
              cursor={{ fill: "var(--muted)", opacity: 0.6 }}
              isAnimationActive={false}
              content={({ active, payload }) =>
                active && payload?.[0] ? (
                  <FlowTooltip row={payload[0].payload as Row} />
                ) : null
              }
            />
            {(
              [
                ["income", "var(--series-1)"],
                ["expense", "var(--series-2)"],
              ] as const
            ).flatMap(([kind, color]) => [
              <Bar
                key={`${kind}Settled`}
                dataKey={`${kind}Settled`}
                stackId={kind}
                fill={color}
                maxBarSize={24}
                isAnimationActive={false}
                // A ponta arredondada fica no topo da pilha: só é do efetivado
                // quando não há previsto por cima.
                shape={(props: { payload?: Row }) => (
                  <Rectangle
                    {...props}
                    radius={
                      props.payload?.[`${kind}Planned`] ? undefined : ROUNDED_TOP
                    }
                  />
                )}
              />,
              <Bar
                key={`${kind}Planned`}
                dataKey={`${kind}Planned`}
                stackId={kind}
                fill={color}
                fillOpacity={PLANNED_OPACITY}
                radius={ROUNDED_TOP}
                maxBarSize={24}
                isAnimationActive={false}
              />,
            ])}
            <Line
              dataKey="accumulated"
              stroke="var(--series-3)"
              strokeWidth={2}
              strokeLinecap="round"
              strokeLinejoin="round"
              dot={{
                r: 4,
                fill: "var(--series-3)",
                stroke: "var(--card)",
                strokeWidth: 2,
              }}
              activeDot={{ r: 5, stroke: "var(--card)", strokeWidth: 2 }}
              isAnimationActive={false}
            />
          </ComposedChart>
        </ResponsiveContainer>
      </div>

      <details className="text-sm">
        <summary className="cursor-pointer text-muted-foreground hover:text-foreground">
          Ver como tabela
        </summary>
        <div className="overflow-x-auto">
          <table className="mt-2 w-full tabular-nums">
            <thead>
              <tr className="text-left text-muted-foreground">
                <th className="py-1 font-normal">Mês</th>
                {[
                  "Receitas efetivadas",
                  "Receitas previstas",
                  "Despesas efetivadas",
                  "Despesas previstas",
                  "Saldo acumulado",
                ].map((heading) => (
                  <th key={heading} className="py-1 pl-3 text-right font-normal">
                    {heading}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {rows.map((row) => (
                <tr key={row.month} className="border-t">
                  <td className="py-1 whitespace-nowrap first-letter:uppercase">
                    {monthLabel(row.month)}
                  </td>
                  {[
                    row.incomeSettled,
                    row.incomePlanned,
                    row.expenseSettled,
                    row.expensePlanned,
                    row.accumulated,
                  ].map((cents, index) => (
                    <td key={index} className="py-1 pl-3 text-right">
                      {formatMoney(cents)}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </details>
    </div>
  );
}

function Swatch({ color }: { color: string }) {
  return (
    <span
      aria-hidden
      className="size-2.5 rounded-[2px]"
      style={{ background: color }}
    />
  );
}

function LineKey() {
  return (
    <span aria-hidden className="h-0.5 w-3.5 rounded-full bg-(--series-3)" />
  );
}

function FlowTooltip({ row }: { row: Row }) {
  const lines = [
    {
      label: "Receitas",
      color: "var(--series-1)",
      settled: row.incomeSettled,
      planned: row.incomePlanned,
    },
    {
      label: "Despesas",
      color: "var(--series-2)",
      settled: row.expenseSettled,
      planned: row.expensePlanned,
    },
  ];
  return (
    <div className="rounded-lg bg-popover p-3 text-sm text-popover-foreground shadow-md ring-1 ring-foreground/10">
      <p className="mb-1.5 text-muted-foreground first-letter:uppercase">
        {monthLabel(row.month)}
      </p>
      <ul className="grid gap-1.5">
        {lines.map((line) => (
          <li key={line.label}>
            <div className="flex items-center gap-2">
              <span
                aria-hidden
                className="h-0.5 w-3 rounded-full"
                style={{ background: line.color }}
              />
              <span className="font-medium tabular-nums">
                {formatMoney(line.settled + line.planned)}
              </span>
              <span className="text-muted-foreground">{line.label}</span>
            </div>
            {line.planned > 0 && (
              <p className="pl-5 text-xs text-muted-foreground tabular-nums">
                {formatMoney(line.settled)} efetivado ·{" "}
                {formatMoney(line.planned)} previsto
              </p>
            )}
          </li>
        ))}
        <li className="flex items-center gap-2">
          <span
            aria-hidden
            className="h-0.5 w-3 rounded-full bg-(--series-3)"
          />
          <span className="font-medium tabular-nums">
            {formatMoney(row.accumulated)}
          </span>
          <span className="text-muted-foreground">Saldo acumulado</span>
        </li>
      </ul>
    </div>
  );
}
