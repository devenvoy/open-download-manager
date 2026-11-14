import { describe, expect, it } from "vitest";
import {
  defaultPortForProtocol,
  formatProxyDisplay,
  formatRouteBadge,
  formatRouteLabel,
  proxyModeLabel,
  proxySourceLabel,
} from "./proxy-view";

describe("proxy-view model", () => {
  it("formats proxy display strings", () => {
    expect(formatProxyDisplay(null)).toBe("Direct (No Proxy)");
    expect(
      formatProxyDisplay({ enabled: false, protocol: "http", host: "1.2.3.4", port: 8080 }),
    ).toBe("Direct (No Proxy)");
    expect(
      formatProxyDisplay({ enabled: true, protocol: "direct", host: "1.2.3.4", port: 8080 }),
    ).toBe("Direct (No Proxy)");
    expect(
      formatProxyDisplay({
        enabled: true,
        protocol: "http",
        host: "proxy.internal",
        port: 3128,
      }),
    ).toBe("HTTP proxy.internal:3128");
    expect(
      formatProxyDisplay({
        enabled: true,
        protocol: "socks5",
        host: "127.0.0.1",
        port: 1080,
        username: "alice",
      }),
    ).toBe("SOCKS5 alice@127.0.0.1:1080");
  });

  it("labels proxy sources", () => {
    expect(proxySourceLabel("system")).toBe("System");
    expect(proxySourceLabel("environment")).toBe("Environment");
    expect(proxySourceLabel({ browser: "Chrome" })).toBe("Chrome");
    expect(proxySourceLabel({} as never)).toBe("System");
  });

  it("labels proxy modes", () => {
    expect(proxyModeLabel("pac")).toBe("PAC Script");
    expect(proxyModeLabel("manual")).toBe("Manual Proxy");
    expect(proxyModeLabel("direct")).toBe("Direct Connection");
  });

  it("adjusts default ports for protocol", () => {
    expect(defaultPortForProtocol("socks5", 8080)).toBe(1080);
    expect(defaultPortForProtocol("socks5", undefined)).toBe(1080);
    expect(defaultPortForProtocol("http", 1080)).toBe(8080);
    expect(defaultPortForProtocol("https", 1080)).toBe(8080);
    expect(defaultPortForProtocol("socks5", 9050)).toBe(9050);
  });

  it("formats route labels and badges", () => {
    expect(
      formatRouteLabel({
        protocol: "http",
        host: "10.0.0.1",
        port: 8080,
        supported: true,
        reason: null,
      }),
    ).toBe("HTTP: 10.0.0.1:8080");

    expect(
      formatRouteBadge({
        protocol: "ftp",
        host: "10.0.0.1",
        port: 21,
        supported: false,
        reason: "FTP unsupported",
      }),
    ).toEqual({ label: "FTP unsupported", supported: false });

    expect(
      formatRouteBadge({
        protocol: "http",
        host: "10.0.0.1",
        port: 80,
        supported: true,
        reason: null,
      }),
    ).toEqual({ label: "Supported", supported: true });
  });
});
