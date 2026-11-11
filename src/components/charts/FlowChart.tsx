import {
  Bar,
  CartesianGrid,
  ComposedChart,
  Line,
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

const SERIES = [
  { key: "incomeCents", label: "Receitas", color: "var(--series-1)", line: false },
  { key: "expenseCents", label: "Despesas", color: "var(--series-2)", line: false },
  {
    key: "accumulatedBalanceCents",
    label: "Saldo acumulado",
    color: "var(--series-3)",
    line: true,
  },
] as const;

const TICK = { fill: "var(--muted-foreground)", fontSize: 12 };

/** Receitas e despesas de cada mês, com o saldo acumulado no período. */
export function FlowChart({ flow }: { flow: MonthFlow[] }) {
  return (
    <div className="grid gap-3">
      <ul className="flex flex-wrap gap-x-4 gap-y-1 text-sm text-muted-foreground">
        {SERIES.map((series) => (
          <li key={series.key} className="flex items-center gap-1.5">
            <SeriesKey color={series.color} line={series.line} />
            {series.label}
          </li>
        ))}
      </ul>

      <div className="h-64">
        <ResponsiveContainer width="100%" height="100%">
          <ComposedChart
            data={flow}
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
                  <FlowTooltip row={payload[0].payload as MonthFlow} />
                ) : null
              }
            />
            <Bar
              dataKey="incomeCents"
              fill="var(--series-1)"
              radius={[4, 4, 0, 0]}
              maxBarSize={24}
              isAnimationActive={false}
            />
            <Bar
              dataKey="expenseCents"
              fill="var(--series-2)"
              radius={[4, 4, 0, 0]}
              maxBarSize={24}
              isAnimationActive={false}
            />
            <Line
              dataKey="accumulatedBalanceCents"
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
        <table className="mt-2 w-full tabular-nums">
          <thead>
            <tr className="text-left text-muted-foreground">
              <th className="py-1 font-normal">Mês</th>
              {SERIES.map((series) => (
                <th key={series.key} className="py-1 text-right font-normal">
                  {series.label}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {flow.map((row) => (
              <tr key={row.month} className="border-t">
                <td className="py-1 first-letter:uppercase">
                  {monthLabel(row.month)}
                </td>
                {SERIES.map((series) => (
                  <td key={series.key} className="py-1 text-right">
                    {formatMoney(row[series.key])}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </details>
    </div>
  );
}

function SeriesKey({ color, line }: { color: string; line: boolean }) {
  return (
    <span
      aria-hidden
      className={line ? "h-0.5 w-3.5 rounded-full" : "size-2.5 rounded-[2px]"}
      style={{ background: color }}
    />
  );
}

function FlowTooltip({ row }: { row: MonthFlow }) {
  return (
    <div className="rounded-lg bg-popover p-3 text-sm text-popover-foreground shadow-md ring-1 ring-foreground/10">
      <p className="mb-1.5 text-muted-foreground first-letter:uppercase">
        {monthLabel(row.month)}
      </p>
      <ul className="grid gap-1">
        {SERIES.map((series) => (
          <li key={series.key} className="flex items-center gap-2">
            <span
              aria-hidden
              className="h-0.5 w-3 rounded-full"
              style={{ background: series.color }}
            />
            <span className="font-medium tabular-nums">
              {formatMoney(row[series.key])}
            </span>
            <span className="text-muted-foreground">{series.label}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}
