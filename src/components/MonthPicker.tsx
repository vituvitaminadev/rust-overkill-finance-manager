import { ChevronLeftIcon, ChevronRightIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { currentMonth, monthLabel, shiftMonth } from "@/lib/format";

export function MonthPicker({
  month,
  onChange,
}: {
  month: string;
  onChange: (month: string) => void;
}) {
  return (
    <div className="flex items-center gap-1">
      <Button
        variant="outline"
        size="icon"
        aria-label="Mês anterior"
        onClick={() => onChange(shiftMonth(month, -1))}
      >
        <ChevronLeftIcon />
      </Button>
      <span className="min-w-40 text-center text-sm font-medium first-letter:uppercase">
        {monthLabel(month)}
      </span>
      <Button
        variant="outline"
        size="icon"
        aria-label="Próximo mês"
        onClick={() => onChange(shiftMonth(month, 1))}
      >
        <ChevronRightIcon />
      </Button>
      {month !== currentMonth() && (
        <Button variant="ghost" onClick={() => onChange(currentMonth())}>
          Hoje
        </Button>
      )}
    </div>
  );
}
