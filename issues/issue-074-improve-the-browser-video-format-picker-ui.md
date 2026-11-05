---
number: 74
title: "Improve the browser video format picker UI"
state: OPEN
author: itzsaad09
created_at: 2026-10-30T17:47:35Z
updated_at: 2026-10-30T17:47:35Z
closed_at: null
url: https://github.com/Ahmadkhattak1/awesome-downloader/issues/74
labels: []
assignees: []
comments_count: 0
---

# Issue #74: Improve the browser video format picker UI

- **Status**: `OPEN`
- **Author**: [@itzsaad09](https://github.com/itzsaad09)
- **Created**: `2026-10-30T17:47:35Z`
- **Updated**: `2026-10-30T17:47:35Z`
- **Original Issue**: [#74 on GitHub](https://github.com/Ahmadkhattak1/awesome-downloader/issues/74)

---

## Description

## Problem

The browser video format picker currently displays formats as a basic numbered text list. Long titles make each option difficult to scan, and important details such as resolution, format, audio availability, and bitrate do not have a clear visual hierarchy.

## Proposed change

Redesign the on-page format picker while preserving its existing download behavior.

- Use a compact modern panel with rounded corners and clear spacing.
- Display resolution as a prominent badge.
- Separate format, audio state, and bitrate into supporting metadata.
- Truncate long media titles without hiding essential format information.
- Add clear hover, focus, disabled, loading, success, and error states.
- Keep the interface responsive on small video players.
- Preserve keyboard accessibility and compatibility with the existing closed Shadow DOM.

## Scope

This change is visual only. It will not modify media detection, format selection, authentication, or download behavior.

I would like to work on this issue.



---

## Comments (0)

*No comments on this issue yet.*
