import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, screen, waitFor } from "@testing-library/react";
import { makeDownload, makeSnapshot } from "@/test/fixtures";
import { renderWithQuery } from "@/test/render";
import { type DownloadQueue } from "@/types/ipc";

const {
  snapshotMock,
  listQueuesMock,
  createQueueMock,
  updateQueueMock,
  deleteQueueMock,
  startQueueMock,
  pauseQueueMock,
  closeWindowMock,
} = vi.hoisted(() => ({
  snapshotMock: vi.fn(),
  listQueuesMock: vi.fn(),
  createQueueMock: vi.fn(),
  updateQueueMock: vi.fn(),
  deleteQueueMock: vi.fn(),
  startQueueMock: vi.fn(),
  pauseQueueMock: vi.fn(),
  closeWindowMock: vi.fn(),
}));

vi.mock("@/lib/ipc", () => ({
  call: vi.fn(),
  ipc: {
    snapshot: snapshotMock,
    listQueues: listQueuesMock,
    createQueue: createQueueMock,
    updateQueue: updateQueueMock,
    deleteQueue: deleteQueueMock,
    startQueue: startQueueMock,
    pauseQueue: pauseQueueMock,
    openAddWindow: vi.fn(),
    closeAuxiliaryWindow: closeWindowMock,
  },
}));

import { SchedulerScreen } from "./SchedulerScreen";

const sampleQueues: DownloadQueue[] = [
  {
    id: "default",
    name: "Default Queue",
    enabled: true,
    running: false,
    concurrencyLimit: 3,
    speedLimitKbps: 0,
    itemIds: ["dl-1", "dl-2"],
    schedule: null,
  },
  {
    id: "nightly",
    name: "Nightly Downloads",
    enabled: true,
    running: true,
    concurrencyLimit: 2,
    speedLimitKbps: 500,
    itemIds: ["dl-3", "dl-4", "dl-5"],
    schedule: {
      enabled: true,
      startTime: "01:00",
      stopTime: "06:00",
      timezone: "local",
      daysOfWeek: [1, 2, 3, 4, 5],
      missedRunPolicy: "runOnStartup",
      nextRunAt: null,
      lastRunAt: null,
    },
  },
];

describe("SchedulerScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    snapshotMock.mockResolvedValue(
      makeSnapshot({
        downloads: [
          makeDownload({
            id: "sched-1",
            filename: "nightly_backup.tar",
            status: "scheduled",
            scheduledAt: "2026-11-15T02:00:00Z",
          }),
        ],
      }),
    );
    listQueuesMock.mockResolvedValue(sampleQueues);
    startQueueMock.mockResolvedValue({ ...sampleQueues[0], running: true });
    pauseQueueMock.mockResolvedValue({ ...sampleQueues[1], running: false });
    closeWindowMock.mockResolvedValue(undefined);
  });

  it("renders the queues tab with queue items", async () => {
    renderWithQuery(<SchedulerScreen />);

    expect(await screen.findByText(/Default Queue/)).toBeInTheDocument();
    expect(screen.getByText("Nightly Downloads")).toBeInTheDocument();
    expect(screen.getByText("+ Create Queue…")).toBeInTheDocument();
  });

  it("toggles queue state between start and pause", async () => {
    renderWithQuery(<SchedulerScreen />);

    await screen.findByText(/Default Queue/);
    const startButton = screen.getByRole("button", { name: "Start" });
    fireEvent.click(startButton);

    await waitFor(() => {
      expect(startQueueMock).toHaveBeenCalledWith("default");
    });
  });

  it("switches to one-off tab and shows scheduled downloads", async () => {
    renderWithQuery(<SchedulerScreen />);

    const oneOffTab = await screen.findByRole("tab", {
      name: "One-off Schedules",
    });
    fireEvent.mouseDown(oneOffTab);

    expect(await screen.findByText("nightly_backup.tar")).toBeInTheDocument();
  });

  it("closes the window on Close button click", async () => {
    renderWithQuery(<SchedulerScreen />);

    const closeButtons = await screen.findAllByRole("button", { name: "Close" });
    fireEvent.click(closeButtons[closeButtons.length - 1]!);

    await waitFor(() => {
      expect(closeWindowMock).toHaveBeenCalled();
    });
  });

  it("displays error state when listQueues fails", async () => {
    listQueuesMock.mockRejectedValue(new Error("Queues database locked"));
    renderWithQuery(<SchedulerScreen />);

    expect(
      await screen.findByText(/Could not load the queues/),
    ).toBeInTheDocument();
  });
});
