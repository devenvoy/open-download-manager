import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { snapshotQueryKey, snapshotQueryOptions } from "@/lib/ipc/queries";
import {
  type CredentialRule,
  type DetectedProxyConfig,
  type MediaToolType,
  type ProxySettings,
  type SettingsPatch,
} from "@/types/ipc";
import { settingsApi } from "../api";

export const credentialRulesQueryKey = ["settings", "credential-rules"] as const;
export const mediaToolsQueryKey = ["settings", "media-tools"] as const;

/** Settings live in the snapshot; this screen does not fetch them twice. */
export function useSettings() {
  return useQuery(snapshotQueryOptions);
}

export function useUpdateSettings() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (patch: SettingsPatch) => settingsApi.update(patch),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: snapshotQueryKey }),
  });
}

export function useCredentialRules() {
  return useQuery({
    queryKey: credentialRulesQueryKey,
    queryFn: () => settingsApi.listCredentialRules(),
  });
}

export function useSaveCredentialRule() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (rule: CredentialRule) => settingsApi.saveCredentialRule(rule),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: credentialRulesQueryKey }),
  });
}

export function useDeleteCredentialRule() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => settingsApi.deleteCredentialRule(id),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: credentialRulesQueryKey }),
  });
}

export function useMediaToolsStatus() {
  return useQuery({
    queryKey: mediaToolsQueryKey,
    queryFn: () => settingsApi.mediaToolsStatus(),
  });
}

export function useInstallMediaTool() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (tool: MediaToolType) => settingsApi.installMediaTool(tool),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: mediaToolsQueryKey }),
  });
}

export function useTestProxy() {
  return useMutation({
    mutationFn: ({
      proxy,
      testUrl,
    }: {
      proxy: ProxySettings;
      testUrl?: string;
    }) => settingsApi.testProxyConnection(proxy, testUrl),
  });
}

export function useDetectProxy() {
  return useMutation({
    mutationFn: () => settingsApi.detectProxy(),
  });
}

export function useApplyDetectedProxy() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (detected: DetectedProxyConfig) =>
      settingsApi.applyDetectedProxy(detected),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: snapshotQueryKey }),
  });
}
