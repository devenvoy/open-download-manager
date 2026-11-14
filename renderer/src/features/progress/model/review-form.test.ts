import { describe, expect, it } from "vitest";
import { makeDownload } from "@/test/fixtures";
import {
  buildReviewPatch,
  draftFromDownload,
  validateReviewDraft,
  type ReviewDraft,
} from "./review-form";

function draft(overrides: Partial<ReviewDraft> = {}): ReviewDraft {
  return {
    filename: "video.mp4",
    directory: "/Users/test/Downloads",
    category: "Video",
    description: "",
    scheduledAt: "",
    authEnabled: false,
    authUser: "",
    authPass: "",
    ...overrides,
  };
}

describe("draftFromDownload", () => {
  it("prefills the fields the review form shows", () => {
    expect(
      draftFromDownload(
        makeDownload({
          filename: "clip.mp4",
          directory: "/Users/test/Videos",
          category: "Video",
          description: "A clip",
        }),
      ),
    ).toMatchObject({
      filename: "clip.mp4",
      directory: "/Users/test/Videos",
      category: "Video",
      description: "A clip",
      scheduledAt: "",
    });
  });
});

describe("validateReviewDraft", () => {
  it("accepts a complete draft", () => {
    expect(validateReviewDraft(draft())).toBeNull();
  });

  it("rejects a relative folder", () => {
    expect(validateReviewDraft(draft({ directory: "relative/path" }))).toMatch(
      /absolute/,
    );
  });

  it("rejects a past or invalid start time", () => {
    expect(
      validateReviewDraft(draft({ scheduledAt: "2020-01-01T00:00" }), Date.now()),
    ).toMatch(/future/);
    expect(validateReviewDraft(draft({ scheduledAt: "not-a-date" }))).toMatch(
      /valid/,
    );
  });

  it("requires a username when authentication is on", () => {
    expect(validateReviewDraft(draft({ authEnabled: true }))).toMatch(/username/);
    expect(
      validateReviewDraft(draft({ authEnabled: true, authUser: "alice" })),
    ).toBeNull();
  });
});

describe("buildReviewPatch", () => {
  it("sends definite values for schedule and description", () => {
    const patch = buildReviewPatch(draft(), false);
    expect(patch).toMatchObject({
      queueOnly: false,
      filename: "video.mp4",
      directory: "/Users/test/Downloads",
      category: "Video",
      description: "",
      scheduledAt: null,
    });
    expect(patch).not.toHaveProperty("credentials");
  });

  it("converts the local schedule to an ISO timestamp", () => {
    const patch = buildReviewPatch(draft({ scheduledAt: "2030-01-02T03:04" }), true);
    expect(patch.queueOnly).toBe(true);
    expect(new Date(patch.scheduledAt as string).getFullYear()).toBe(2030);
  });

  it("passes credentials only when the toggle is on", () => {
    const patch = buildReviewPatch(
      draft({ authEnabled: true, authUser: " alice ", authPass: "secret" }),
      false,
    );
    expect(patch.credentials).toEqual({ username: "alice", password: "secret" });
  });
});
