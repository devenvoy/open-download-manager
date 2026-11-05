---
number: 42
title: "Play this YouTube video briefly so its streaming session can be captured, then try again. (BUG)"
state: OPEN
author: pubgokplayer-glitch
created_at: 2026-10-27T10:08:28Z
updated_at: 2026-10-30T17:55:19Z
closed_at: null
url: https://github.com/Ahmadkhattak1/awesome-downloader/issues/42
labels: []
assignees: []
comments_count: 3
---

# Issue #42: Play this YouTube video briefly so its streaming session can be captured, then try again. (BUG)

- **Status**: `OPEN`
- **Author**: [@pubgokplayer-glitch](https://github.com/pubgokplayer-glitch)
- **Created**: `2026-10-27T10:08:28Z`
- **Updated**: `2026-10-30T17:55:19Z`
- **Original Issue**: [#42 on GitHub](https://github.com/Ahmadkhattak1/awesome-downloader/issues/42)

---

## Description

When we click on the download drop down menu it says "Play this YouTube video briefly so its streaming session can be captured, then try again." making unable to download the video.

Please take a look at this issue



---

## Comments (3)

### Comment #1 by [@MuhammadHaroonRashid](https://github.com/MuhammadHaroonRashid) • `2026-10-27T13:12:53Z`
[Original Comment Link](https://github.com/Ahmadkhattak1/awesome-downloader/issues/42#issuecomment-6442767450)

I'd like to take this one. I've reproduced a root cause and have a fix with a regression test ready on my fork.

**Root cause.** The accepted SABR request (`youtubeSabrObserved` in `extension/background.js`) is held only in service-worker memory. When Chrome restarts the MV3 worker (idle timeout, extension reload/update, memory pressure), that capture is lost. Opening the menu re-sends player data through `refreshPlayer`, so formats are still listed (matching the symptom in #40). But `enqueue()` finds no SABR capture and shows "Play this YouTube video briefly…" until YouTube happens to send another SABR request. A paused or fully buffered video may not send one for a long time.

**Reproduction.** I used Playwright Chromium with the unpacked extension, with the video playing:
1. Before the restart, the selection reaches native handoff.
2. I stopped the extension worker via CDP while the video kept playing.
3. After the restart, the player was known again but the SABR capture was missing, and the menu showed the exact error.
4. It recovered only after a new SABR request.

**Proposed fix (about 30 lines):**
- Mirror the accepted SABR request into `chrome.storage.session`. It is memory-only, never written to disk, and not exposed to content scripts under the default access level.
- Fall back to it only when the in-memory capture is missing.
- Reuse it only for the same tab and the same `videoId`.
- Clear it on navigation and tab close, and bound the store to the 10 most recent tabs.
- Cookies and request headers stay in worker memory as they are today. Only the SABR URL and request body, which the worker already holds, would be stored.

Since `background.js` notes that credentials stay in worker memory, I'd like your agreement on this boundary before opening the PR.

**Testing:**
- **Regression test:** a new `background.test.cjs` case simulates a restart by sharing session storage between two harness instances. It covers same-video reuse, rejection for another video or another tab, and clearing on navigation. It fails on `main` and passes with the fix, and the full suite passes (65/65).
- **Live:** Chrome on Windows 10 x64 with the dev desktop app (`npm start`). I played a video, paused it, stopped the extension worker from `chrome://serviceworker-internals`, then picked 1080p from the menu. The download was handed off and completed (1080p video plus audio, muxed MP4). A normal download without a restart also completed.

Refs #40

---

### Comment #2 by [@zkaeshjm](https://github.com/zkaeshjm) • `2026-10-28T12:04:49Z`
[Original Comment Link](https://github.com/Ahmadkhattak1/awesome-downloader/issues/42#issuecomment-6468506366)

Root cause found and fixed in PR #37 (commit `f9215b4`).

The SABR streaming seed is captured from network traffic and cannot be rebuilt from the page, but it was kept only in the extension service worker's memory. Chrome stops the MV3 worker between playback and the download click, so the capture was already gone by click time — hence the message even after playing the video. A second problem compounded it: SABR formats were offered whenever player metadata existed, so a click with no capture could only fail.

The fix keeps one bounded snapshot per tab in `storage.session` (the seed only; cookies/authorization still never leave the worker), restores it on demand, and hides SABR formats until a capture exists so the menu explains how to capture one instead of failing after a click.

`npm test` passes with new regressions that replay this exact sequence. Live verification on a real YouTube page is still needed before closing — see PR #37.

---

### Comment #3 by [@itzsaad09](https://github.com/itzsaad09) • `2026-10-30T17:55:18Z` (CONTRIBUTOR)
[Original Comment Link](https://github.com/Ahmadkhattak1/awesome-downloader/issues/42#issuecomment-6529757999)

Additional diagnostic information:

The issue only occurs in the installed YouTube desktop PWA.

Steps tested:
1. Opened a YouTube video in the desktop PWA.
2. Played the video and sought to an unbuffered section.
3. The format picker continued showing “Play this video for a few seconds...”
4. Opened the same video in a normal Chrome tab.
5. The available video and audio formats appeared correctly.

This suggests that media requests from the PWA window are not being associated with the active YouTube frame/tab, while capture from a normal Chrome tab works.

---
