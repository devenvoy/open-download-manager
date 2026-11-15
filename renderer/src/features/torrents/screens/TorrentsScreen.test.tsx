import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, screen, waitFor } from "@testing-library/react";
import {
  makeSnapshot,
  makeTorrent,
  makeTorrentFile,
  makeTorrentSettings,
  makeTorrentStatus,
} from "@/test/fixtures";
import { renderWithQuery } from "@/test/render";

const {
  snapshotMock,
  statusMock,
  listMock,
  settingsGetMock,
  settingsSetMock,
  takeReviewMock,
  setWatchingMock,
  filesMock,
  trackersMock,
  actionMock,
  setFilesMock,
} = vi.hoisted(() => ({
  snapshotMock: vi.fn(),
  statusMock: vi.fn(),
  listMock: vi.fn(),
  settingsGetMock: vi.fn(),
  settingsSetMock: vi.fn(),
  takeReviewMock: vi.fn(),
  setWatchingMock: vi.fn(),
  filesMock: vi.fn(),
  trackersMock: vi.fn(),
  actionMock: vi.fn(),
  setFilesMock: vi.fn(),
}));

vi.mock("@/lib/ipc", () => ({
  call: vi.fn(),
  ipc: {
    snapshot: snapshotMock,
    torrentStatus: statusMock,
    torrentList: listMock,
    torrentSettingsGet: settingsGetMock,
    torrentSettingsSet: settingsSetMock,
    torrentTakeReview: takeReviewMock,
    torrentSetWatching: setWatchingMock,
    torrentFiles: filesMock,
    torrentTrackerStatus: trackersMock,
    torrentAction: actionMock,
    torrentSetFiles: setFilesMock,
    torrentAddMagnet: vi.fn(),
    torrentAddFile: vi.fn(),
    torrentRestartEngine: vi.fn(),
    chooseDirectory: vi.fn(),
    chooseFile: vi.fn(),
  },
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

import { TorrentsScreen } from "./TorrentsScreen";
import { useTorrentsStore } from "../state/torrents-store";

const torrents = [
  makeTorrent({ id: 1, infoHash: "aaa", name: "debian.iso" }),
  makeTorrent({ id: 2, infoHash: "bbb", name: "ubuntu.iso", state: "paused" }),
];

describe("TorrentsScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useTorrentsStore.getState().reset();
    useTorrentsStore.setState({ answeredReviews: new Set() });

    snapshotMock.mockResolvedValue(makeSnapshot());
    statusMock.mockResolvedValue(makeTorrentStatus());
    listMock.mockResolvedValue(torrents);
    settingsGetMock.mockResolvedValue(makeTorrentSettings());
    takeReviewMock.mockResolvedValue(null);
    setWatchingMock.mockResolvedValue(undefined);
    filesMock.mockResolvedValue([makeTorrentFile()]);
    trackersMock.mockResolvedValue([]);
    actionMock.mockResolvedValue(undefined);
    setFilesMock.mockResolvedValue(undefined);
  });

  it("renders the torrent rows and keeps the engine watched while open", async () => {
    renderWithQuery(<TorrentsScreen />);

    expect(await screen.findAllByText("debian.iso")).toHaveLength(2);
    expect(screen.getByText("ubuntu.iso")).toBeInTheDocument();
    expect(setWatchingMock).toHaveBeenCalledWith(true);
    expect(statusMock).toHaveBeenCalledTimes(1);
    expect(listMock).toHaveBeenCalledTimes(1);
  });

  it("pauses the selected torrent through torrent_action", async () => {
    renderWithQuery(<TorrentsScreen />);
    await screen.findAllByText("debian.iso");

    fireEvent.click(screen.getByRole("button", { name: "Pause" }));

    await waitFor(() =>
      expect(actionMock).toHaveBeenCalledWith(1, "pause"),
    );
    expect(actionMock).toHaveBeenCalledTimes(1);
  });

  it("offers the enable action when torrent support is off", async () => {
    statusMock.mockResolvedValue(
      makeTorrentStatus({ enabled: false, state: "disabled" }),
    );

    renderWithQuery(<TorrentsScreen />);
    await screen.findByText(/BitTorrent support is disabled in settings/);

    fireEvent.click(screen.getByRole("button", { name: "Enable" }));

    await waitFor(() =>
      expect(settingsSetMock).toHaveBeenCalledWith(
        expect.objectContaining({ enabled: true }),
      ),
    );
  });

  it("shows the empty state when there are no torrents", async () => {
    listMock.mockResolvedValue([]);
    renderWithQuery(<TorrentsScreen />);

    expect(await screen.findByText("No torrents yet")).toBeInTheDocument();
  });

  it("shows review panel in details pane and confirms start download", async () => {
    takeReviewMock.mockResolvedValue({
      id: 1,
      name: "debian.iso",
      infoHash: "aaa",
    });

    renderWithQuery(<TorrentsScreen />);
    expect(await screen.findByRole("region", { name: "Torrent download review" })).toBeInTheDocument();
    const startBtn = screen.getByRole("button", { name: "Start download" });
    await waitFor(() => expect(startBtn).toBeEnabled());

    fireEvent.click(startBtn);
    await waitFor(() =>
      expect(setFilesMock).toHaveBeenCalledWith(1, [0]),
    );
    await waitFor(() =>
      expect(actionMock).toHaveBeenCalledWith(1, "resume"),
    );
  });
});
