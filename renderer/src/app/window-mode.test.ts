import { describe, expect, it } from "vitest";
import { readWindowRoute } from "./window-mode";

describe("readWindowRoute", () => {
  it("defaults to the main window", () => {
    expect(readWindowRoute("")).toEqual({ mode: "main" });
  });

  it("routes a progress window with its download id", () => {
    expect(readWindowRoute("?progress=abc-123")).toEqual({
      mode: "progress",
      downloadId: "abc-123",
    });
  });

  it("routes the options, scheduler and integration windows", () => {
    expect(readWindowRoute("?options=1").mode).toBe("options");
    expect(readWindowRoute("?scheduler=1").mode).toBe("scheduler");
    expect(readWindowRoute("?integration=1").mode).toBe("integration");
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
