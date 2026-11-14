import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, screen, waitFor } from "@testing-library/react";
import { makeDownload, makeSnapshot } from "@/test/fixtures";
import { renderWithQuery } from "@/test/render";

const {
  snapshotMock,
  actionMock,
  updateSettingsMock,
  retryActionsMock,
  reviewMock,
  chooseDirectoryMock,
  resizeMock,
  closeMock,
} = vi.hoisted(() => ({
  snapshotMock: vi.fn(),
  actionMock: vi.fn(),
  updateSettingsMock: vi.fn(),
  retryActionsMock: vi.fn(),
  reviewMock: vi.fn(),
  chooseDirectoryMock: vi.fn(),
  resizeMock: vi.fn(),
  closeMock: vi.fn(),
}));

vi.mock("@/lib/ipc", () => ({
  call: vi.fn(),
  ipc: {
    snapshot: snapshotMock,
    downloadAction: actionMock,
    updateSettings: updateSettingsMock,
    retryDownloadActions: retryActionsMock,
    reviewDownload: reviewMock,
    chooseDirectory: chooseDirectoryMock,
    resizeProgressWindow: resizeMock,
    closeAuxiliaryWindow: closeMock,
    installMediaTool: vi.fn(),
  },
}));

import { ProgressScreen } from "./ProgressScreen";

const download = makeDownload({
  id: "progress-1",
  filename: "file.zip",
  status: "downloading",
  total: 1024,
  downloaded: 512,
  speed: 1024,
});

function snapshotWith(overrides = {}) {
  return makeSnapshot({ downloads: [makeDownload({ ...download, ...overrides })] });
}

describe("ProgressScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    snapshotMock.mockResolvedValue(snapshotWith());
    actionMock.mockResolvedValue(undefined);
    updateSettingsMock.mockResolvedValue(undefined);
    retryActionsMock.mockResolvedValue(undefined);
    reviewMock.mockResolvedValue(["progress-1"]);
    chooseDirectoryMock.mockResolvedValue(null);
    resizeMock.mockResolvedValue(undefined);
    closeMock.mockResolvedValue(undefined);
  });

  it("renders the title, facts and connections of a running download", async () => {
    renderWithQuery(<ProgressScreen id="progress-1" />);

    expect(await screen.findByText("50% file.zip")).toBeInTheDocument();
    expect(screen.getAllByText("Receiving data…").length).toBeGreaterThan(0);
    expect(screen.getByText("512 B (50.00%)")).toBeInTheDocument();
    expect(screen.getByText("1 connection")).toBeInTheDocument();
  });

  it("pauses and opens the folder through download_action", async () => {
    renderWithQuery(<ProgressScreen id="progress-1" />);
    await screen.findByText("50% file.zip");

    fireEvent.click(screen.getByRole("button", { name: "Pause" }));
    await waitFor(() => expect(actionMock).toHaveBeenCalledWith("progress-1", "pause"));

    fireEvent.mouseDown(screen.getByRole("tab", { name: "Options on completion" }));
    fireEvent.click(screen.getByRole("button", { name: "Open folder" }));
    await waitFor(() =>
      expect(actionMock).toHaveBeenCalledWith("progress-1", "reveal"),
    );
  });

  it("offers retry on a failed download", async () => {
    snapshotMock.mockResolvedValue(
      snapshotWith({ status: "error", error: "socket closed" }),
    );
    renderWithQuery(<ProgressScreen id="progress-1" />);

    expect(await screen.findByText("Download Interrupted")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Retry download" }));

    await waitFor(() =>
      expect(actionMock).toHaveBeenCalledWith("progress-1", "retry"),
    );
  });

  it("saves the global speed limit from the speed tab", async () => {
    renderWithQuery(<ProgressScreen id="progress-1" />);
    await screen.findByText("50% file.zip");

    fireEvent.mouseDown(screen.getByRole("tab", { name: "Speed Limiter" }));
    fireEvent.change(screen.getByLabelText("Global speed limit (KB/s)"), {
      target: { value: "512" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Apply" }));

    await waitFor(() =>
      expect(updateSettingsMock).toHaveBeenCalledWith({ speedLimitKbps: 512 }),
    );
  });

  it("submits the review form when the download waits for confirmation", async () => {
    snapshotMock.mockResolvedValue(snapshotWith({ needsReview: true }));
    renderWithQuery(<ProgressScreen id="progress-1" />);

    expect(await screen.findByText("Download File Info")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Start Download" }));

    await waitFor(() =>
      expect(reviewMock).toHaveBeenCalledWith(
        "progress-1",
        expect.objectContaining({ queueOnly: false, scheduledAt: null }),
      ),
    );
    expect(closeMock).not.toHaveBeenCalled();
  });

  it("removes the download when the review is cancelled", async () => {
    snapshotMock.mockResolvedValue(snapshotWith({ needsReview: true }));
    renderWithQuery(<ProgressScreen id="progress-1" />);

    await screen.findByText("Download File Info");
    fireEvent.click(screen.getByRole("button", { name: "Cancel download" }));

    await waitFor(() =>
      expect(actionMock).toHaveBeenCalledWith("progress-1", "remove"),
    );
    await waitFor(() => expect(closeMock).toHaveBeenCalled());
  });
});
