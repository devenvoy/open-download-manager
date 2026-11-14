import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, screen, waitFor } from "@testing-library/react";
import { makeSnapshot } from "@/test/fixtures";
import { renderWithQuery } from "@/test/render";

const {
  snapshotMock,
  integrationInfoMock,
  advancedCapabilityMock,
  browserSetupMock,
  closeWindowMock,
} = vi.hoisted(() => ({
  snapshotMock: vi.fn(),
  integrationInfoMock: vi.fn(),
  advancedCapabilityMock: vi.fn(),
  browserSetupMock: vi.fn(),
  closeWindowMock: vi.fn(),
}));

vi.mock("@/lib/ipc", () => ({
  call: vi.fn(),
  ipc: {
    snapshot: snapshotMock,
    integrationInfo: integrationInfoMock,
    advancedCapability: advancedCapabilityMock,
    browserSetupAction: browserSetupMock,
    enableAdvancedIntegration: vi.fn(),
    disableAdvancedIntegration: vi.fn(),
    repairAdvancedIntegration: vi.fn(),
    closeAuxiliaryWindow: closeWindowMock,
  },
}));

import { IntegrationScreen } from "./IntegrationScreen";

describe("IntegrationScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    snapshotMock.mockResolvedValue(makeSnapshot());
    integrationInfoMock.mockResolvedValue({
      port: 19834,
      connectedBrowsers: ["Chrome"],
      browsers: [
        {
          name: "Chrome",
          installed: true,
          connected: true,
          setupState: "connected",
          extensionInstalled: true,
          binaryPath: "/Applications/Google Chrome.app",
        },
        {
          name: "Firefox",
          installed: true,
          connected: false,
          setupState: "setup",
          extensionInstalled: false,
          binaryPath: "/Applications/Firefox.app",
        },
      ],
      connectionError: null,
      extensionPath: "/path/to/ext",
      extensionAvailable: true,
      integrationPath: "",
      packaged: true,
      executable: "/path/to/bin",
    });
    advancedCapabilityMock.mockResolvedValue({
      supported: true,
      enabled: false,
      status: "ready",
      requiresElevation: false,
      issues: [],
    });
    browserSetupMock.mockResolvedValue({
      success: true,
      message: "Setup OK",
    });
  });

  it("renders the browser list and connection summary", async () => {
    renderWithQuery(<IntegrationScreen />);

    expect(await screen.findByText("Chrome")).toBeInTheDocument();
    expect(screen.getByText("Firefox")).toBeInTheDocument();
    expect(screen.getByText("1 browser connected")).toBeInTheDocument();
  });

  it("selects a browser to open details and navigates back", async () => {
    renderWithQuery(<IntegrationScreen />);

    const chromeCard = await screen.findByText("Chrome");
    fireEvent.click(chromeCard);

    expect(await screen.findByText("‹ All browsers")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "‹ All browsers" }));

    expect(await screen.findByText("Choose your browser")).toBeInTheDocument();
  });

  it("closes the window on Close button click", async () => {
    renderWithQuery(<IntegrationScreen />);

    const closeButtons = await screen.findAllByRole("button", { name: "Close" });
    fireEvent.click(closeButtons[closeButtons.length - 1]!);

    await waitFor(() => {
      expect(closeWindowMock).toHaveBeenCalled();
    });
  });

  it("displays error state when integration query fails", async () => {
    integrationInfoMock.mockRejectedValue(new Error("Native host down"));
    renderWithQuery(<IntegrationScreen />);

    expect(
      await screen.findByText(/Could not read the browser integration/),
    ).toBeInTheDocument();
  });
});
