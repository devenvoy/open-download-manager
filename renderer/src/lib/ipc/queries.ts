import { queryOptions } from "@tanstack/react-query";
import { ipc } from "./index";

/**
 * The one snapshot query (plan R1). Every screen reads this cache; no screen
 * keeps a second copy of the download list. Polling is a placeholder until
 * the Rust side emits transfer events (plan R2/R3).
 */
export const snapshotQueryOptions = queryOptions({
  queryKey: ["snapshot"] as const,
  queryFn: () => ipc.snapshot(),
  refetchInterval: 2000,
  staleTime: 1000,
});

export const snapshotQueryKey = snapshotQueryOptions.queryKey;
