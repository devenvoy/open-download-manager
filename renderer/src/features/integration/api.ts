import { queryOptions, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { ipc } from "@/lib/ipc";
import { snapshotQueryKey } from "@/lib/ipc/queries";
import { type IntegrationStatus } from "@/types/ipc";

/** The integration feature's IPC seam. Components never import `@/lib/ipc`. */
export const integrationApi = {
  info: () => ipc.integrationInfo(),
  setup: (action: string, browser: string) =>
    ipc.browserSetupAction(action, browser),
  advanced: () => ipc.advancedCapability(),
  enableAdvanced: () => ipc.enableAdvancedIntegration(true),
  disableAdvanced: () => ipc.disableAdvancedIntegration(),
  repairAdvanced: () => ipc.repairAdvancedIntegration(),
};

export const integrationKeys = {
  info: ["integration", "info"] as const,
  advanced: ["integration", "advanced"] as const,
};

/**
 * The integration window polls quickly: a browser that is opened, or has the
 * extension loaded, answers within seconds and the cards must update.
 */
export const integrationInfoQuery = queryOptions({
  queryKey: integrationKeys.info,
  queryFn: () => integrationApi.info(),
  refetchInterval: 3_000,
});

export const advancedIntegrationQuery = queryOptions({
  queryKey: integrationKeys.advanced,
  queryFn: () => integrationApi.advanced(),
  staleTime: 10_000,
});

export function useIntegrationInfo() {
  return useQuery(integrationInfoQuery);
}

export function useAdvancedIntegration() {
  return useQuery(advancedIntegrationQuery);
}

export function useBrowserSetup() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: async ({
      action,
      browser,
    }: {
      action: string;
      browser: string;
    }) => {
      // "prepare" registers the connector before the browser is opened, so a
      // fresh browser finds it on first launch.
      if (action === "prepare") {
        const prepared = await integrationApi.setup("prepare", browser);
        await integrationApi.setup("open", browser);
        return prepared;
      }
      return integrationApi.setup(action, browser);
    },
    onSuccess: async () => {
      await client.invalidateQueries({ queryKey: integrationKeys.info });
    },
    onError: (error) =>
      toast.error(error instanceof Error ? error.message : "Browser setup failed."),
  });
}

export function useAdvancedIntegrationActions() {
  const client = useQueryClient();
  const setCapability = (capability: Awaited<ReturnType<typeof integrationApi.advanced>>) =>
    client.setQueryData(integrationKeys.advanced, capability);
  const refresh = () =>
    client.invalidateQueries({ queryKey: integrationKeys.advanced });

  const enable = useMutation({
    mutationFn: () => integrationApi.enableAdvanced(),
    onSuccess: setCapability,
    onError: async (error) => {
      await refresh();
      toast.error(error instanceof Error ? error.message : "Could not enable enhanced mode.");
    },
  });

  const disable = useMutation({
    mutationFn: () => integrationApi.disableAdvanced(),
    onSuccess: setCapability,
    onError: (error) =>
      toast.error(error instanceof Error ? error.message : "Could not disable enhanced mode."),
  });

  const repair = useMutation({
    mutationFn: () => integrationApi.repairAdvanced(),
    onSuccess: setCapability,
    onError: (error) =>
      toast.error(error instanceof Error ? error.message : "Could not repair enhanced mode."),
  });

  return { enable, disable, repair };
}

/** Registers a browser from the main window without opening this one. */
export function useRegisterBrowser() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: ({ browser, extensionId }: { browser: string; extensionId: string }) =>
      ipc.registerBrowser(browser, extensionId),
    onSuccess: async () => {
      await client.invalidateQueries({ queryKey: snapshotQueryKey });
      await client.invalidateQueries({ queryKey: integrationKeys.info });
    },
  });
}

/** Unregisters a browser native messaging manifest. */
export function useUnregisterBrowser() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: ({ browser }: { browser: string }) =>
      ipc.unregisterBrowser(browser),
    onSuccess: async () => {
      await client.invalidateQueries({ queryKey: snapshotQueryKey });
      await client.invalidateQueries({ queryKey: integrationKeys.info });
    },
  });
}

export type { IntegrationStatus };
