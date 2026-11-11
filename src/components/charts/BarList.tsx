import { formatMoney } from "@/lib/format";

/**
 * Barras horizontais de uma única série, da maior para a menor, com o valor
 * escrito ao lado de cada uma.
 */
export function BarList({
  items,
  empty,
}: {
  items: { id: string; name: string; totalCents: number }[];
  empty: string;
}) {
  if (items.length === 0) {
    return <p className="py-6 text-sm text-muted-foreground">{empty}</p>;
  }
  const max = Math.max(...items.map((item) => item.totalCents));
  const total = items.reduce((sum, item) => sum + item.totalCents, 0);

  return (
    <ul className="grid grid-cols-[minmax(0,auto)_minmax(0,1fr)_auto] items-center gap-x-3 gap-y-2.5 text-sm">
      {items.map((item) => {
        const share = Math.round((item.totalCents / total) * 100);
        return (
          <li
            key={item.id}
            className="group col-span-3 grid grid-cols-subgrid items-center"
            title={`${item.name}: ${formatMoney(item.totalCents)} (${share}% do total)`}
          >
            <span className="truncate text-muted-foreground group-hover:text-foreground">
              {item.name}
            </span>
            <span className="h-3.5">
              <span
                className="block h-full min-w-1 rounded-r-[4px] bg-(--series-1) transition-opacity group-hover:opacity-80"
                style={{ width: `${(item.totalCents / max) * 100}%` }}
              />
            </span>
            <span className="text-right font-medium tabular-nums">
              {formatMoney(item.totalCents)}
            </span>
          </li>
        );
      })}
    </ul>
  );
}
