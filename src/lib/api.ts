import { invoke } from "@tauri-apps/api/core";

export type Role = "admin" | "common";
export type EntryKind = "income" | "expense";
export type EntryStatus = "planned" | "settled";

export interface HouseholdInfo {
  id: string;
  name: string;
}

export interface Member {
  id: string;
  username: string;
  role: Role;
  active: boolean;
}

export interface Status {
  hasHouseholds: boolean;
  username: string | null;
  households: HouseholdInfo[];
  current: { household: HouseholdInfo; member: Member } | null;
}

export interface Category {
  id: string;
  name: string;
  kind: EntryKind;
  archived: boolean;
}

export interface Entry {
  id: string;
  kind: EntryKind;
  amountCents: number;
  date: string;
  description: string;
  categoryId: string;
  categoryName: string;
  assigneeId: string;
  assigneeName: string;
  status: EntryStatus;
  /** Previsto cuja data já passou. */
  overdue: boolean;
  recurrenceId: string | null;
  installment: { planId: string; number: number; count: number } | null;
  version: number;
  canEdit: boolean;
}

export interface EntryInput {
  kind: EntryKind;
  amountCents: number;
  date: string;
  description: string;
  categoryId: string;
  assigneeId: string | null;
  /** Sem valor, o backend decide pela data (novo) ou mantém a atual (edição). */
  status?: EntryStatus | null;
}

export interface EntryFilter {
  month?: string | null;
  kind?: EntryKind | null;
  status?: EntryStatus | null;
  categoryId?: string | null;
  assigneeId?: string | null;
  search?: string | null;
}

/** Um valor separado no que já aconteceu e no que ainda está previsto. */
export interface Totals {
  settledCents: number;
  plannedCents: number;
}

export interface Dashboard {
  month: string;
  income: Totals;
  expense: Totals;
  balance: Totals;
  expensesByCategory: (Totals & { categoryId: string; name: string })[];
  expensesByAssignee: (Totals & { memberId: string; name: string })[];
  flow: MonthFlow[];
  upcoming: Entry[];
  overdue: Entry[];
}

export interface MonthFlow {
  month: string;
  income: Totals;
  expense: Totals;
  accumulatedBalanceCents: number;
}

export interface Recurrence {
  id: string;
  kind: EntryKind;
  amountCents: number;
  description: string;
  categoryId: string;
  categoryName: string;
  assigneeId: string;
  assigneeName: string;
  dayOfMonth: number;
  startMonth: string;
  endMonth: string | null;
  canEdit: boolean;
}

export interface RecurrenceInput {
  kind: EntryKind;
  amountCents: number;
  description: string;
  categoryId: string;
  assigneeId: string | null;
  firstDate: string;
  endMonth: string | null;
}

export interface InstallmentPlan {
  id: string;
  description: string;
  totalCents: number;
  count: number;
  plannedCount: number;
  plannedCents: number;
  categoryName: string;
  assigneeId: string;
  assigneeName: string;
  firstDate: string;
  canEdit: boolean;
}

export interface InstallmentPlanInput {
  description: string;
  totalCents: number;
  count: number;
  firstDate: string;
  categoryId: string;
  assigneeId: string | null;
}

export const api = {
  status: () => invoke<Status>("status"),
  login: (username: string, password: string, remember: boolean) =>
    invoke<Status>("login", { username, password, remember }),
  logout: () => invoke<Status>("logout"),
  createHousehold: (args: {
    name: string;
    username?: string;
    password?: string;
    remember: boolean;
  }) => invoke<Status>("create_household", args),
  selectHousehold: (id: string) => invoke<Status>("select_household", { id }),
  closeHousehold: () => invoke<Status>("close_household"),

  members: () => invoke<Member[]>("members"),
  categories: () => invoke<Category[]>("categories"),
  createCategory: (input: { name: string; kind: EntryKind }) =>
    invoke<Category>("create_category", { input }),
  renameCategory: (id: string, name: string) =>
    invoke<Category>("rename_category", { id, name }),
  setCategoryArchived: (id: string, archived: boolean) =>
    invoke<Category>("set_category_archived", { id, archived }),

  entries: (filter: EntryFilter) => invoke<Entry[]>("entries", { filter }),
  createEntry: (input: EntryInput) => invoke<Entry>("create_entry", { input }),
  /** `following` estende a alteração aos previstos seguintes da mesma Recorrência. */
  updateEntry: (id: string, input: EntryInput, following: boolean) =>
    invoke<Entry>("update_entry", { id, input, following }),
  settleEntry: (id: string, amountCents: number) =>
    invoke<Entry>("settle_entry", { id, amountCents }),
  deleteEntry: (id: string) => invoke<void>("delete_entry", { id }),

  dashboard: (month: string) => invoke<Dashboard>("dashboard", { month }),

  recurrences: () => invoke<Recurrence[]>("recurrences"),
  createRecurrence: (input: RecurrenceInput) =>
    invoke<Recurrence>("create_recurrence", { input }),
  endRecurrence: (id: string) => invoke<void>("end_recurrence", { id }),
  installmentPlans: () => invoke<InstallmentPlan[]>("installment_plans"),
  createInstallmentPlan: (input: InstallmentPlanInput) =>
    invoke<InstallmentPlan>("create_installment_plan", { input }),
  deleteInstallmentPlan: (id: string) =>
    invoke<void>("delete_installment_plan", { id }),
};

/** A mensagem de um erro vindo de um comando, já pronta para exibir. */
export function errorMessage(error: unknown): string {
  if (typeof error === "object" && error !== null && "message" in error) {
    return String(error.message);
  }
  return typeof error === "string" ? error : "Algo deu errado. Tente de novo.";
}
