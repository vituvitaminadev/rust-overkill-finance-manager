import {
  keepPreviousData,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { api, type EntryFilter } from "./api";

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

/** Uma alteração nos dados da residência: ao concluir, tudo o que depende deles é recarregado. */
export function useHouseholdMutation<Args, Result>(
  action: (args: Args) => Promise<Result>,
  onDone?: () => void,
) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: action,
    onSuccess: () => {
      for (const key of ["entries", "dashboard", "categories"]) {
        queryClient.invalidateQueries({ queryKey: [key] });
      }
      onDone?.();
    },
  });
}
