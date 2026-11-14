import { beforeEach, describe, expect, it } from "vitest";
import { makeDownload } from "@/test/fixtures";
import {
  filterDownloads,
  useDownloadsStore,
  visibleSelection,
} from "./downloads-store";

const downloads = [
  makeDownload({ id: "1", category: "Video" }),
  makeDownload({ id: "2", category: "Audio" }),
  makeDownload({ id: "3", category: null }),
];

describe("downloads store", () => {
  beforeEach(() => {
    useDownloadsStore.setState({ selectedIds: [], category: "all" });
  });

  it("toggles a selection on and off", () => {
    const { toggleSelected } = useDownloadsStore.getState();
    toggleSelected("1");
    expect(useDownloadsStore.getState().selectedIds).toEqual(["1"]);
    toggleSelected("1");
    expect(useDownloadsStore.getState().selectedIds).toEqual([]);
  });

  it("deduplicates a selection", () => {
    useDownloadsStore.getState().setSelection(["1", "1", "2"]);
    expect(useDownloadsStore.getState().selectedIds).toEqual(["1", "2"]);
  });

  it("filters by category and keeps uncategorised rows out", () => {
    expect(filterDownloads(downloads, "all")).toHaveLength(3);
    expect(filterDownloads(downloads, "Video").map((d) => d.id)).toEqual(["1"]);
    expect(filterDownloads(downloads, "Audio").map((d) => d.id)).toEqual(["2"]);
    expect(filterDownloads(downloads, "Other")).toHaveLength(0);
  });

  it("drops selected ids that are not visible", () => {
    expect(visibleSelection(["1", "2"], [downloads[0]!])).toEqual(["1"]);
  });
});
