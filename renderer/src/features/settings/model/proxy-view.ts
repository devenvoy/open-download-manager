import {
  type DetectedProxyMode,
  type ProtocolRoute,
  type ProxyProtocol,
  type ProxySettings,
  type ProxySource,
} from "@/types/ipc";

/** Format proxy configuration into a readable string (matches legacy formatter). */
export function formatProxyDisplay(
  proxy?: Partial<ProxySettings> | null,
): string {
  if (!proxy || !proxy.enabled || proxy.protocol === "direct") {
    return "Direct (No Proxy)";
  }
  const userPrefix = proxy.username ? `${proxy.username}@` : "";
  const host = proxy.host || "";
  const port = proxy.port || 8080;
  return `${String(proxy.protocol || "http").toUpperCase()} ${userPrefix}${host}:${port}`;
}

export function proxySourceLabel(source: ProxySource): string {
  if (typeof source === "string") {
    return source.charAt(0).toUpperCase() + source.slice(1);
  }
  if (source && typeof source === "object" && "browser" in source) {
    return source.browser || "Browser";
  }
  return "System";
}

export function proxyModeLabel(mode: DetectedProxyMode): string {
  switch (mode) {
    case "pac":
      return "PAC Script";
    case "manual":
      return "Manual Proxy";
    case "direct":
    default:
      return "Direct Connection";
  }
}

export function defaultPortForProtocol(
  protocol: ProxyProtocol,
  currentPort?: number,
): number {
  if (protocol === "socks5" && (currentPort === 8080 || !currentPort)) {
    return 1080;
  }
  if ((protocol === "http" || protocol === "https") && currentPort === 1080) {
    return 8080;
  }
  return currentPort || (protocol === "socks5" ? 1080 : 8080);
}

export function formatRouteLabel(route: ProtocolRoute): string {
  return `${route.protocol.toUpperCase()}: ${route.host}:${route.port}`;
}

export function formatRouteBadge(route: ProtocolRoute): {
  label: string;
  supported: boolean;
} {
  return {
    label: route.supported ? "Supported" : route.reason || "Unsupported",
    supported: route.supported,
  };
}
