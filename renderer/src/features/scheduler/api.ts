import { queryOptions, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { ipc } from "@/lib/ipc";
import { snapshotQueryKey, snapshotQueryOptions } from "@/lib/ipc/queries";
import { type QueueSchedule } from "@/types/ipc";

/** The scheduler feature's IPC seam. */
export const schedulerApi = {
  queues: () => ipc.listQueues(),
  create: (
    name: string,
    concurrencyLimit: number,
    speedLimitKbps: number,
    schedule: QueueSchedule | null,
  ) => ipc.createQueue(name, { concurrencyLimit, speedLimitKbps, schedule }),
  update: (
    id: string,
    patch: {
      name?: string;
      concurrencyLimit?: number;
      speedLimitKbps?: number;
      schedule?: QueueSchedule | null;
    },
  ) => ipc.updateQueue(id, patch),
  remove: (id: string, disposition: "moveToDefault" | "removeDownloads") =>
    ipc.deleteQueue(id, disposition),
  start: (id: string) => ipc.startQueue(id),
  pause: (id: string) => ipc.pauseQueue(id),
  openAddWindow: (scheduled: string) => ipc.openAddWindow({ scheduled }),
};

export const schedulerKeys = {
  queues: ["scheduler", "queues"] as const,
};

export const queuesQuery = queryOptions({
  queryKey: schedulerKeys.queues,
  queryFn: () => schedulerApi.queues(),
  staleTime: 5_000,
});

/** Queue state plus the scheduled downloads (they arrive in the snapshot). */
export function useQueues() {
  return useQuery(queuesQuery);
}

export function useScheduledDownloads() {
  return useQuery(snapshotQueryOptions);
}

export function useQueueActions() {
  const client = useQueryClient();
  const refresh = async () => {
    await Promise.all([
      client.invalidateQueries({ queryKey: schedulerKeys.queues }),
      client.invalidateQueries({ queryKey: snapshotQueryKey }),
    ]);
  };

  const create = useMutation({
    mutationFn: ({
      name,
      concurrencyLimit,
      speedLimitKbps,
      schedule,
    }: {
      name: string;
      concurrencyLimit: number;
      speedLimitKbps: number;
      schedule: QueueSchedule | null;
    }) => schedulerApi.create(name, concurrencyLimit, speedLimitKbps, schedule),
    onSuccess: refresh,
    onError: (error) => toast.error(messageOf(error)),
  });

  const update = useMutation({
    mutationFn: ({
      id,
      ...patch
    }: {
      id: string;
      name?: string;
      concurrencyLimit?: number;
      speedLimitKbps?: number;
      schedule?: QueueSchedule | null;
    }) => schedulerApi.update(id, patch),
    onSuccess: refresh,
    onError: (error) => toast.error(messageOf(error)),
  });

  const remove = useMutation({
    mutationFn: (id: string) => schedulerApi.remove(id, "moveToDefault"),
    onSuccess: refresh,
    onError: (error) => toast.error(messageOf(error)),
  });

  const toggle = useMutation({
    mutationFn: ({ id, running }: { id: string; running: boolean }) =>
      running ? schedulerApi.pause(id) : schedulerApi.start(id),
    onSuccess: refresh,
    onError: (error) => toast.error(messageOf(error)),
  });

  return { create, update, remove, toggle };
}

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : "The queue action failed.";
}
