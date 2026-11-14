import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, screen, waitFor } from "@testing-library/react";
import { makeDownload, makeSnapshot } from "@/test/fixtures";
import { renderWithQuery } from "@/test/render";

const { snapshotMock, actionMock } = vi.hoisted(() => ({
  snapshotMock: vi.fn(),
  actionMock: vi.fn(),
}));

vi.mock("@/lib/ipc", () => ({
  call: vi.fn(),
  ipc: {
    snapshot: snapshotMock,
    downloadAction: actionMock,
    addDownload: vi.fn(),
    updateSettings: vi.fn(),
    removeCompleted: vi.fn(),
    openProgressWindow: vi.fn(),
    openOptionsWindow: vi.fn(),
    openAddWindow: vi.fn(),
    openSchedulerWindow: vi.fn(),
    openIntegrationWindow: vi.fn(),
    openTorrentWindow: vi.fn(),
    closeAuxiliaryWindow: vi.fn(),
    chooseDirectory: vi.fn(),
    readClipboard: vi.fn(),
  },
}));

import { DownloadsScreen } from "./DownloadsScreen";
import { useDownloadsStore } from "../state/downloads-store";

const downloads = [
  makeDownload({ id: "1", filename: "file1.zip" }),
  makeDownload({ id: "2", filename: "file2.mp4", category: "Video" }),
];

describe("DownloadsScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useDownloadsStore.setState({ selectedIds: [], category: "all" });
    snapshotMock.mockResolvedValue(makeSnapshot({ downloads }));
    actionMock.mockResolvedValue(undefined);
  });

  it("renders the rows from the snapshot query", async () => {
    renderWithQuery(<DownloadsScreen />);

    expect(await screen.findByText("file1.zip")).toBeInTheDocument();
    expect(screen.getByText("file2.mp4")).toBeInTheDocument();
    expect(snapshotMock).toHaveBeenCalledTimes(1);
  });

  it("filters rows from the category sidebar", async () => {
    renderWithQuery(<DownloadsScreen />);
    await screen.findByText("file1.zip");

    fireEvent.click(screen.getByRole("button", { name: "Video" }));

    expect(screen.queryByText("file1.zip")).not.toBeInTheDocument();
    expect(screen.getByText("file2.mp4")).toBeInTheDocument();
  });

  it("deletes the selected downloads through download_action", async () => {
    renderWithQuery(<DownloadsScreen />);
    await screen.findByText("file1.zip");

    fireEvent.click(screen.getByText("file1.zip"));
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));

    await waitFor(() =>
      expect(actionMock).toHaveBeenCalledWith("1", "remove"),
    );
    expect(actionMock).toHaveBeenCalledTimes(1);
  });

  it("displays status bar counts", async () => {
    renderWithQuery(<DownloadsScreen />);
    expect(await screen.findByText("2 downloads")).toBeInTheDocument();
  });
});
