const money = new Intl.NumberFormat("pt-BR", {
  style: "currency",
  currency: "BRL",
});
const compact = new Intl.NumberFormat("pt-BR", {
  notation: "compact",
  maximumFractionDigits: 1,
});
const longMonth = new Intl.DateTimeFormat("pt-BR", {
  month: "long",
  year: "numeric",
  timeZone: "UTC",
});
const shortMonth = new Intl.DateTimeFormat("pt-BR", {
  month: "short",
  year: "2-digit",
  timeZone: "UTC",
});

export function formatMoney(cents: number): string {
  return money.format(cents / 100);
}

/** Valor abreviado para eixos de gráfico: 1,5 mil, 2 mi. */
export function formatCompactMoney(cents: number): string {
  return compact.format(cents / 100);
}

/** Valor sem o símbolo da moeda, para preencher um campo de edição: 1234,56. */
export function formatAmountInput(cents: number): string {
  return (cents / 100).toFixed(2).replace(".", ",");
}

/** Lê "1.234,56", "1234,5" ou "1234.56" como centavos; `null` se não for um valor. */
export function parseMoney(text: string): number | null {
  let clean = text.replace(/[R$\s]/g, "");
  if (clean.includes(",")) clean = clean.replace(/\./g, "").replace(",", ".");
  if (!/^\d+(\.\d{1,2})?$/.test(clean)) return null;
  return Math.round(Number(clean) * 100);
}

/** "2026-11-10" vira "10/11/2026". */
export function formatDate(iso: string): string {
  const [year, month, day] = iso.split("-");
  return `${day}/${month}/${year}`;
}

export function today(): string {
  const now = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
}

export function currentMonth(): string {
  return today().slice(0, 7);
}

export function shiftMonth(month: string, by: number): string {
  const [year, index] = month.split("-").map(Number);
  const date = new Date(Date.UTC(year, index - 1 + by, 1));
  return date.toISOString().slice(0, 7);
}

/** "2026-11" vira "novembro de 2026". */
export function monthLabel(month: string): string {
  return longMonth.format(new Date(`${month}-01T00:00:00Z`));
}

/** "2026-11" vira "nov/26". */
export function monthShortLabel(month: string): string {
  const label = shortMonth.format(new Date(`${month}-01T00:00:00Z`));
  return label.replace(". de ", "/").replace(" de ", "/").replace(".", "");
}

const dateTime = new Intl.DateTimeFormat("pt-BR", {
  day: "2-digit",
  month: "2-digit",
  hour: "2-digit",
  minute: "2-digit",
});

/** Um instante em ISO vira "13/11, 20:15", no horário deste computador. */
export function formatDateTime(iso: string): string {
  return dateTime.format(new Date(iso));
}
