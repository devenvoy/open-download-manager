import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { makeDownload } from "@/test/fixtures";
import { DownloadsTable } from "./DownloadsTable";

const downloads = [
  makeDownload({ id: "1", filename: "file1.zip", status: "downloading" }),
  makeDownload({ id: "2", filename: "file2.mp4", status: "paused" }),
];

function setup(overrides: Partial<Parameters<typeof DownloadsTable>[0]> = {}) {
  const props = {
    downloads,
    selectedIds: [] as string[],
    onToggle: vi.fn(),
    onToggleAll: vi.fn(),
    onAction: vi.fn(),
    ...overrides,
  };
  render(<DownloadsTable {...props} />);
  return props;
}

describe("DownloadsTable", () => {
  it("renders the rows", () => {
    setup();
    expect(screen.getByText("file1.zip")).toBeInTheDocument();
    expect(screen.getByText("file2.mp4")).toBeInTheDocument();
  });

  it("shows the real Rust status labels", () => {
    setup();
    expect(screen.getByText("Downloading")).toBeInTheDocument();
    expect(screen.getByText("Paused")).toBeInTheDocument();
  });

  it("pauses an active download", () => {
    const props = setup();
    fireEvent.click(screen.getByRole("button", { name: "Pause" }));
    expect(props.onAction).toHaveBeenCalledWith("1", "pause");
  });

  it("resumes a paused download", () => {
    const props = setup();
    fireEvent.click(screen.getByRole("button", { name: "Resume" }));
    expect(props.onAction).toHaveBeenCalledWith("2", "resume");
  });

  it("opens a completed download instead of resuming it", () => {
    const props = setup({
      downloads: [makeDownload({ id: "3", filename: "done.zip", status: "complete" })],
    });
    fireEvent.click(screen.getByRole("button", { name: "Open" }));
    expect(props.onAction).toHaveBeenCalledWith("3", "open");
  });

  it("retries a failed download", () => {
    const props = setup({
      downloads: [makeDownload({ id: "4", filename: "bad.zip", status: "error" })],
    });
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(props.onAction).toHaveBeenCalledWith("4", "retry");
  });

  it("toggles a row selection", () => {
    const props = setup();
    fireEvent.click(screen.getByLabelText("Select file1.zip"));
    expect(props.onToggle).toHaveBeenCalledWith("1");
  });

  it("toggles every visible row", () => {
    const props = setup();
    fireEvent.click(screen.getByLabelText("Select all downloads"));
    expect(props.onToggleAll).toHaveBeenCalledWith(true);
  });

  it("shows an empty state when there are no downloads", () => {
    setup({ downloads: [] });
    expect(screen.getByText(/no downloads yet/i)).toBeInTheDocument();
  });
});
