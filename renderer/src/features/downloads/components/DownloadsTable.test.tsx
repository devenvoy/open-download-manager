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
    onAction: vi.fn(),
    onOpenProperties: vi.fn(),
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

  it("shows the legacy status labels", () => {
    setup();
    expect(screen.getByText("Stopped")).toBeInTheDocument();
  });

  it("toggles row selection on click", () => {
    const props = setup();
    fireEvent.click(screen.getByText("file1.zip"));
    expect(props.onToggle).toHaveBeenCalledWith("1");
  });

  it("opens properties on double click", () => {
    const props = setup();
    fireEvent.doubleClick(screen.getByText("file1.zip"));
    expect(props.onOpenProperties).toHaveBeenCalledWith("1");
  });

  it("filters rows using the search box", () => {
    setup();
    const searchInput = screen.getByLabelText("Search downloads");
    fireEvent.change(searchInput, { target: { value: "file2" } });
    expect(screen.queryByText("file1.zip")).not.toBeInTheDocument();
    expect(screen.getByText("file2.mp4")).toBeInTheDocument();
  });

  it("shows the empty state message when no downloads match", () => {
    setup({ downloads: [] });
    expect(screen.getByText("No downloads to display.")).toBeInTheDocument();
  });
});
