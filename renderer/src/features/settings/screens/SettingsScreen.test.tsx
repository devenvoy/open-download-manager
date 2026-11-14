import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, screen, waitFor } from "@testing-library/react";
import { makeSettings, makeSnapshot } from "@/test/fixtures";
import { renderWithQuery } from "@/test/render";

const { snapshotMock, updateSettingsMock, closeWindowMock } = vi.hoisted(() => ({
  snapshotMock: vi.fn(),
  updateSettingsMock: vi.fn(),
  closeWindowMock: vi.fn(),
}));

vi.mock("@/lib/ipc", () => ({
  call: vi.fn(),
  ipc: {
    snapshot: snapshotMock,
    updateSettings: updateSettingsMock,
    addDownload: vi.fn(),
    downloadAction: vi.fn(),
    removeCompleted: vi.fn(),
    openProgressWindow: vi.fn(),
    closeAuxiliaryWindow: closeWindowMock,
    chooseDirectory: vi.fn(),
    readClipboard: vi.fn(),
    listCredentialRules: vi.fn().mockResolvedValue([]),
    saveCredentialRule: vi.fn(),
    deleteCredentialRule: vi.fn(),
    mediaToolsStatus: vi.fn().mockResolvedValue([]),
    installMediaTool: vi.fn(),
    testProxyConnection: vi.fn(),
    detectProxy: vi.fn(),
    applyDetectedProxy: vi.fn(),
  },
}));

import { SettingsScreen } from "./SettingsScreen";

describe("SettingsScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    snapshotMock.mockResolvedValue(makeSnapshot());
    updateSettingsMock.mockResolvedValue(makeSettings());
    closeWindowMock.mockResolvedValue(undefined);
  });

  it("renders the settings from the snapshot", async () => {
    renderWithQuery(<SettingsScreen />);

    const directory = await screen.findByLabelText("Default download folder:");
    expect(directory).toHaveValue("/Users/test/Downloads");
    expect(screen.getByText("Options")).toBeInTheDocument();
  });

  it("saves the options and closes auxiliary window on OK", async () => {
    renderWithQuery(<SettingsScreen />);
    await screen.findByLabelText("Default download folder:");

    fireEvent.click(screen.getByRole("button", { name: "OK" }));

    await waitFor(() => {
      expect(updateSettingsMock).toHaveBeenCalledWith(
        expect.objectContaining({
          defaultDirectory: "/Users/test/Downloads",
          theme: "system",
          maxConcurrent: 3,
          connections: 8,
        }),
      );
      expect(closeWindowMock).toHaveBeenCalled();
    });
  });

  it("shows the error state when the snapshot fails", async () => {
    snapshotMock.mockRejectedValue(new Error("ipc offline"));
    renderWithQuery(<SettingsScreen />);

    expect(
      await screen.findByText("Could not load settings"),
    ).toBeInTheDocument();
  });

  it("closes the window on Cancel", async () => {
    renderWithQuery(<SettingsScreen />);
    await screen.findByLabelText("Default download folder:");

    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));

    await waitFor(() => {
      expect(closeWindowMock).toHaveBeenCalled();
    });
  });
});
