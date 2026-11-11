import { invoke } from "@tauri-apps/api/core";

export type Role = "admin" | "common";
export type EntryKind = "income" | "expense";

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
}

export interface EntryFilter {
  month?: string | null;
  kind?: EntryKind | null;
  categoryId?: string | null;
  assigneeId?: string | null;
  search?: string | null;
}

export interface Dashboard {
  month: string;
  incomeCents: number;
  expenseCents: number;
  balanceCents: number;
  expensesByCategory: { categoryId: string; name: string; totalCents: number }[];
  expensesByAssignee: { memberId: string; name: string; totalCents: number }[];
  flow: MonthFlow[];
}

export interface MonthFlow {
  month: string;
  incomeCents: number;
  expenseCents: number;
  accumulatedBalanceCents: number;
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
  updateEntry: (id: string, input: EntryInput) =>
    invoke<Entry>("update_entry", { id, input }),
  deleteEntry: (id: string) => invoke<void>("delete_entry", { id }),

  dashboard: (month: string) => invoke<Dashboard>("dashboard", { month }),
};

/** A mensagem de um erro vindo de um comando, já pronta para exibir. */
export function errorMessage(error: unknown): string {
  if (typeof error === "object" && error !== null && "message" in error) {
    return String(error.message);
  }
  return typeof error === "string" ? error : "Algo deu errado. Tente de novo.";
}
