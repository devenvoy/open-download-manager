import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, screen, waitFor } from "@testing-library/react";
import { makeSnapshot } from "@/test/fixtures";
import { renderWithQuery } from "@/test/render";

const {
  snapshotMock,
  listQueuesMock,
  addDownloadMock,
  openProgressWindowMock,
  closeWindowMock,
  readClipboardMock,
} = vi.hoisted(() => ({
  snapshotMock: vi.fn(),
  listQueuesMock: vi.fn(),
  addDownloadMock: vi.fn(),
  openProgressWindowMock: vi.fn(),
  closeWindowMock: vi.fn(),
  readClipboardMock: vi.fn(),
}));

vi.mock("@/lib/ipc", () => ({
  call: vi.fn(),
  ipc: {
    snapshot: snapshotMock,
    listQueues: listQueuesMock,
    addDownload: addDownloadMock,
    openProgressWindow: openProgressWindowMock,
    closeAuxiliaryWindow: closeWindowMock,
    readClipboard: readClipboardMock,
    chooseDirectory: vi.fn(),
    checkDestinationConflict: vi
      .fn()
      .mockResolvedValue({ type: "resolved", path: "/test", policy: "ask" }),
    playlistInfo: vi.fn(),
    videoFormats: vi.fn(),
    addPlaylistDownloads: vi.fn(),
  },
}));

import { AddDownloadScreen } from "./AddDownloadScreen";

describe("AddDownloadScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    snapshotMock.mockResolvedValue(makeSnapshot());
    listQueuesMock.mockResolvedValue([
      {
        id: "default",
        name: "Default Queue",
        enabled: true,
        running: false,
        concurrencyLimit: 3,
        speedLimitKbps: 0,
        itemIds: [],
        schedule: null,
      },
    ]);
    addDownloadMock.mockResolvedValue(["dl-123"]);
    openProgressWindowMock.mockResolvedValue(undefined);
    closeWindowMock.mockResolvedValue(undefined);
    readClipboardMock.mockResolvedValue("https://example.com/test.zip");
  });

  it("renders the add download screen with form fields", async () => {
    renderWithQuery(<AddDownloadScreen paste={false} />);

    expect(await screen.findByText("Download File Info")).toBeInTheDocument();
    expect(screen.getByLabelText("URL")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Start Download" }),
    ).toBeInTheDocument();
  });

  it("reads and fills clipboard url when paste is true", async () => {
    renderWithQuery(<AddDownloadScreen paste={true} />);

    expect(
      await screen.findByDisplayValue("https://example.com/test.zip"),
    ).toBeInTheDocument();
  });

  it("validates empty url on submit", async () => {
    renderWithQuery(<AddDownloadScreen paste={false} />);

    const button = await screen.findByRole("button", {
      name: "Start Download",
    });
    fireEvent.submit(button.closest("form")!);

    expect(
      await screen.findByText("Enter at least one download address."),
    ).toBeInTheDocument();
  });

  it("submits download now and opens progress window", async () => {
    renderWithQuery(<AddDownloadScreen paste={true} />);

    await screen.findByDisplayValue("https://example.com/test.zip");
    const button = screen.getByRole("button", { name: "Start Download" });
    fireEvent.submit(button.closest("form")!);

    await waitFor(() => {
      expect(addDownloadMock).toHaveBeenCalledWith(
        expect.objectContaining({
          urls: ["https://example.com/test.zip"],
          queueOnly: false,
        }),
      );
      expect(openProgressWindowMock).toHaveBeenCalledWith("dl-123");
      expect(closeWindowMock).toHaveBeenCalled();
    });
  });

  it("submits download later without opening progress window", async () => {
    renderWithQuery(<AddDownloadScreen paste={true} />);

    await screen.findByDisplayValue("https://example.com/test.zip");
    fireEvent.click(screen.getByRole("button", { name: "Download Later" }));

    await waitFor(() => {
      expect(addDownloadMock).toHaveBeenCalledWith(
        expect.objectContaining({
          urls: ["https://example.com/test.zip"],
          queueOnly: true,
        }),
      );
      expect(openProgressWindowMock).not.toHaveBeenCalled();
      expect(closeWindowMock).toHaveBeenCalled();
    });
  });

  it("closes the window on Cancel", async () => {
    renderWithQuery(<AddDownloadScreen paste={false} />);

    const cancelBtn = await screen.findByRole("button", { name: "Cancel" });
    fireEvent.click(cancelBtn);

    await waitFor(() => {
      expect(closeWindowMock).toHaveBeenCalled();
    });
  });
});
