import { describe, expect, it } from "vitest";
import { extractExtension, getFileIconUrl } from "./file-icon";

describe("extractExtension", () => {
  it("extracts extension correctly", () => {
    expect(extractExtension("movie.mkv")).toBe("mkv");
    expect(extractExtension("archive.tar.gz")).toBe("gz");
    expect(extractExtension("no-extension")).toBe("");
    expect(extractExtension("")).toBe("");
    expect(extractExtension(null)).toBe("");
  });
});

describe("getFileIconUrl", () => {
  it("maps extensions to catalog icons", () => {
    expect(getFileIconUrl("video.mp4")).toBe("/catalog-icons/video.svg");
    expect(getFileIconUrl("song.mp3")).toBe("/catalog-icons/audio.svg");
    expect(getFileIconUrl("data.zip")).toBe("/catalog-icons/zip.svg");
    expect(getFileIconUrl("doc.pdf")).toBe("/catalog-icons/pdf.svg");
    expect(getFileIconUrl("app.exe")).toBe("/catalog-icons/exe.svg");
    expect(getFileIconUrl("unknown.xyz123")).toBe("/catalog-icons/document.svg");
  });

  it("prioritizes native fileIcons map when available", () => {
    const custom = { ".mp4": "data:image/svg+xml;base64,custom-mp4" };
    expect(getFileIconUrl("video.mp4", custom)).toBe(
      "data:image/svg+xml;base64,custom-mp4",
    );
  });
});
