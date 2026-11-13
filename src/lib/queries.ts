import {
  keepPreviousData,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { api, type EntryFilter } from "./api";

const HOUSEHOLD_DATA = [
  "entries",
  "dashboard",
  "categories",
  "recurrences",
  "installmentPlans",
  "members",
];

export function useMembers() {
  return useQuery({ queryKey: ["members"], queryFn: api.members });
}

export function useCategories() {
  return useQuery({ queryKey: ["categories"], queryFn: api.categories });
}

export function useEntries(filter: EntryFilter) {
  return useQuery({
    queryKey: ["entries", filter],
    queryFn: () => api.entries(filter),
    // Ao trocar um filtro, a lista anterior fica na tela até a nova chegar.
    placeholderData: keepPreviousData,
  });
}

export function useDashboard(month: string) {
  return useQuery({
    queryKey: ["dashboard", month],
    queryFn: () => api.dashboard(month),
    placeholderData: keepPreviousData,
  });
}

export function useRecurrences() {
  return useQuery({ queryKey: ["recurrences"], queryFn: api.recurrences });
}

export function useInstallmentPlans() {
  return useQuery({
    queryKey: ["installmentPlans"],
    queryFn: api.installmentPlans,
  });
}

/** Uma alteração nos dados da residência: ao concluir, tudo o que depende deles é recarregado. */
export function useHouseholdMutation<Args, Result>(
  action: (args: Args) => Promise<Result>,
  onDone?: () => void,
) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: action,
    onSuccess: () => {
      for (const key of HOUSEHOLD_DATA) {
        queryClient.invalidateQueries({ queryKey: [key] });
      }
      onDone?.();
    },
  });
}
