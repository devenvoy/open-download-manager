import { describe, expect, it } from "vitest";
import { readWindowRoute } from "./window-mode";

describe("readWindowRoute", () => {
  describe("label-based routing (Tauri v2)", () => {
    it("routes by window label without query strings", () => {
      expect(readWindowRoute("options")).toEqual({ mode: "options" });
      expect(readWindowRoute("scheduler")).toEqual({ mode: "scheduler" });
      expect(readWindowRoute("integration")).toEqual({ mode: "integration" });
      expect(readWindowRoute("torrents")).toEqual({ mode: "torrents" });
      expect(readWindowRoute("add")).toEqual({
        mode: "add",
        paste: false,
        scheduledAt: undefined,
      });
      expect(readWindowRoute("main")).toEqual({ mode: "main" });
    });

    it("routes dynamic progress windows from label prefix", () => {
      expect(readWindowRoute("progress-abc-123")).toEqual({
        mode: "progress",
        downloadId: "abc-123",
      });
      expect(readWindowRoute("progress-task_99")).toEqual({
        mode: "progress",
        downloadId: "task_99",
      });
    });

    it("merges query options for the add window when present", () => {
      expect(readWindowRoute("add", "?paste=1&scheduled=2026-11-16T12%3A00")).toEqual({
        mode: "add",
        paste: true,
        scheduledAt: "2026-11-16T12:00",
      });
    });
  });

  describe("query-string fallback", () => {
    it("defaults to the main window", () => {
      expect(readWindowRoute("")).toEqual({ mode: "main" });
    });

    it("routes a progress window with its download id", () => {
      expect(readWindowRoute("?progress=abc-123")).toEqual({
        mode: "progress",
        downloadId: "abc-123",
      });
    });

    it("routes the options, scheduler, integration and torrents windows", () => {
      expect(readWindowRoute("?options=1").mode).toBe("options");
      expect(readWindowRoute("?scheduler=1").mode).toBe("scheduler");
      expect(readWindowRoute("?integration=1").mode).toBe("integration");
      expect(readWindowRoute("?torrents=1").mode).toBe("torrents");
    });

    it("keeps the add window flags", () => {
      expect(readWindowRoute("?add=1&paste=1&scheduled=2026-11-14T09%3A30")).toEqual({
        mode: "add",
        paste: true,
        scheduledAt: "2026-11-14T09:30",
      });
      expect(readWindowRoute("?add=1")).toEqual({
        mode: "add",
        paste: false,
        scheduledAt: undefined,
      });
    });

    it("ignores a flag that is not set to 1", () => {
      expect(readWindowRoute("?options=0").mode).toBe("main");
    });
  });
});
