import { ipc } from "@/lib/ipc";
import {
  type CredentialRule,
  type DetectedProxyConfig,
  type MediaToolStatus,
  type MediaToolType,
  type ProxySettings,
  type ProxyTestResult,
  type Settings,
  type SettingsPatch,
} from "@/types/ipc";

/** The settings feature's IPC surface (plan §9). */
export const settingsApi = {
  update: (patch: SettingsPatch): Promise<Settings> =>
    ipc.updateSettings(patch),
  chooseDirectory: (): Promise<string | null> => ipc.chooseDirectory(),
  close: (): Promise<void> => ipc.closeAuxiliaryWindow(),

  // Credentials
  listCredentialRules: (): Promise<CredentialRule[]> =>
    ipc.listCredentialRules(),
  saveCredentialRule: (rule: CredentialRule): Promise<CredentialRule> =>
    ipc.saveCredentialRule(rule),
  deleteCredentialRule: (id: string): Promise<void> =>
    ipc.deleteCredentialRule(id),

  // Media tools
  mediaToolsStatus: (): Promise<MediaToolStatus[]> => ipc.mediaToolsStatus(),
  installMediaTool: (tool: MediaToolType): Promise<MediaToolStatus> =>
    ipc.installMediaTool(tool),

  // Proxy
  testProxyConnection: (
    proxy: ProxySettings,
    testUrl?: string,
  ): Promise<ProxyTestResult> => ipc.testProxyConnection(proxy, testUrl),
  detectProxy: (): Promise<DetectedProxyConfig> => ipc.detectProxy(),
  applyDetectedProxy: (detected: DetectedProxyConfig): Promise<Settings> =>
    ipc.applyDetectedProxy(detected),
};
