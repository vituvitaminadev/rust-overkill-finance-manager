import {
  keepPreviousData,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { useEffect } from "react";
import { api, type EntryFilter } from "./api";

const HOUSEHOLD_DATA = [
  "entries",
  "dashboard",
  "categories",
  "recurrences",
  "installmentPlans",
  "members",
  "invites",
  "syncInfo",
  "conflicts",
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

export function useInvites(enabled: boolean) {
  return useQuery({ queryKey: ["invites"], queryFn: api.invites, enabled });
}

export function useSyncInfo() {
  return useQuery({
    queryKey: ["syncInfo"],
    queryFn: api.syncInfo,
    // O número de cópias conectadas muda sem que nenhum dado mude.
    refetchInterval: 5000,
  });
}

export function useConflicts(enabled: boolean) {
  return useQuery({ queryKey: ["conflicts"], queryFn: api.conflicts, enabled });
}

/**
 * Mantém a tela em dia com o que chega pela rede: quando o backend avisa que a
 * residência ou o estado da sincronização mudou, os dados são relidos.
 */
export function useLiveHousehold() {
  const queryClient = useQueryClient();
  useEffect(() => {
    const refresh = () => {
      for (const key of HOUSEHOLD_DATA) {
        queryClient.invalidateQueries({ queryKey: [key] });
      }
    };
    const listeners = [
      listen("household-changed", refresh),
      listen("sync-changed", refresh),
    ];
    return () => {
      for (const listener of listeners) listener.then((stop) => stop());
    };
  }, [queryClient]);
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
