---
number: 25
title: "Enhance YouTube video session capture before playback"
state: OPEN
author: devenvoy
created_at: 2026-11-05T20:04:52Z
updated_at: 2026-11-05T20:04:52Z
url: https://github.com/devenvoy/open-download-manager/issues/25
labels: ["bug"]
---

# Issue #25: Enhance YouTube video session capture before playback

- **Status**: `OPEN`
- **Author**: [@devenvoy](https://github.com/devenvoy)
- **Created**: `2026-11-05T20:04:52Z`
- **Labels**: `bug`
- **Issue Link**: [#25 on GitHub](https://github.com/devenvoy/open-download-manager/issues/25)

---

## Description

## Problem
When clicking the download action on a YouTube video before the video starts playing, the browser extension cannot capture the active streaming session, prompting the user to play the video first.

## Objective
Improve the session capture workflow on YouTube:
- Clarify in-page prompts when streaming media manifests have not yet been initialized by the player.
- Streamline automatic session capture upon playback start so format selection is available immediately.

