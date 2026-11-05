---
number: 22
title: "Improve the browser video format picker UI"
state: OPEN
author: devenvoy
created_at: 2026-11-05T20:04:30Z
updated_at: 2026-11-05T20:04:30Z
url: https://github.com/devenvoy/open-download-manager/issues/22
labels: ["enhancement"]
---

# Issue #22: Improve the browser video format picker UI

- **Status**: `OPEN`
- **Author**: [@devenvoy](https://github.com/devenvoy)
- **Created**: `2026-11-05T20:04:30Z`
- **Labels**: `enhancement`
- **Issue Link**: [#22 on GitHub](https://github.com/devenvoy/open-download-manager/issues/22)

---

## Description

## Problem
The browser video format picker currently displays formats as a plain text list. Long video titles can make each option difficult to scan, and important details like resolution, format, audio availability, and bitrate need clearer visual hierarchy.

## Proposed Change
Redesign the on-page format picker while preserving existing download behavior:
- Use a compact, modern floating panel with clear spacing and rounded borders.
- Display resolution badges prominently (e.g., 1080p, 4K, 720p).
- Group audio and video tracks cleanly with clear indicators.
- Preserve keyboard navigation, responsive positioning, and lightweight bundle size.

