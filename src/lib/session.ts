import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type Status } from "./api";

const STATUS_KEY = ["status"];

export function useStatus() {
  return useQuery({ queryKey: STATUS_KEY, queryFn: api.status });
}

/**
 * Uma ação que muda quem está conectado ou qual residência está aberta.
 * O novo estado substitui o anterior e os dados da residência antiga são descartados.
 */
export function useSessionAction<Args>(action: (args: Args) => Promise<Status>) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: action,
    onSuccess: (status) => {
      queryClient.removeQueries({
        predicate: (query) => query.queryKey[0] !== STATUS_KEY[0],
      });
      queryClient.setQueryData(STATUS_KEY, status);
    },
  });
}
