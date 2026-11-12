import type { Totals } from "@/lib/api";
import { formatMoney } from "@/lib/format";

/**
 * Barras horizontais de uma única série, da maior para a menor, com o total
 * escrito ao lado. A parte prevista de cada barra aparece mais clara.
 */
export function BarList({
  items,
  empty,
}: {
  items: (Totals & { id: string; name: string })[];
  empty: string;
}) {
  if (items.length === 0) {
    return <p className="py-6 text-sm text-muted-foreground">{empty}</p>;
  }
  const totalOf = (item: Totals) => item.settledCents + item.plannedCents;
  const max = Math.max(...items.map(totalOf));

  return (
    <ul className="grid grid-cols-[minmax(0,auto)_minmax(0,1fr)_auto] items-center gap-x-3 gap-y-2.5 text-sm">
      {items.map((item) => (
        <li
          key={item.id}
          className="group col-span-3 grid grid-cols-subgrid items-center"
          title={`${item.name}: ${formatMoney(item.settledCents)} efetivado, ${formatMoney(item.plannedCents)} previsto`}
        >
          <span className="truncate text-muted-foreground group-hover:text-foreground">
            {item.name}
          </span>
          <span className="flex h-3.5 gap-0.5 transition-opacity group-hover:opacity-80">
            {item.settledCents > 0 && (
              <span
                className="min-w-1 bg-(--series-1) last:rounded-r-[4px]"
                style={{ width: `${(item.settledCents / max) * 100}%` }}
              />
            )}
            {item.plannedCents > 0 && (
              <span
                className="min-w-1 rounded-r-[4px] bg-(--series-1) opacity-40"
                style={{ width: `${(item.plannedCents / max) * 100}%` }}
              />
            )}
          </span>
          <span className="text-right font-medium tabular-nums">
            {formatMoney(totalOf(item))}
          </span>
        </li>
      ))}
    </ul>
  );
}

/** A legenda que explica a parte mais clara das barras. */
export function PlannedKey() {
  return (
    <span className="flex items-center gap-1.5 text-sm text-muted-foreground">
      <span
        aria-hidden
        className="size-2.5 rounded-[2px] bg-foreground opacity-40"
      />
      Mais claro: previsto
    </span>
  );
}
