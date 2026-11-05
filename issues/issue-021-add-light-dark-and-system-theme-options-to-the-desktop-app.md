---
number: 21
title: "Add light, dark, and system theme options to the desktop app"
state: OPEN
author: devenvoy
created_at: 2026-11-05T20:04:13Z
updated_at: 2026-11-05T20:04:13Z
url: https://github.com/devenvoy/open-download-manager/issues/21
labels: ["enhancement"]
---

# Issue #21: Add light, dark, and system theme options to the desktop app

- **Status**: `OPEN`
- **Author**: [@devenvoy](https://github.com/devenvoy)
- **Created**: `2026-11-05T20:04:13Z`
- **Labels**: `enhancement`
- **Issue Link**: [#21 on GitHub](https://github.com/devenvoy/open-download-manager/issues/21)

---

## Description

## Problem
The desktop application currently uses a fixed visual theme. Users cannot select a light or dark appearance or have the application follow the OS theme automatically.

## Proposed Change
Add an application theme setting with three options:
- **System**: automatically follow the operating system light/dark preference
- **Light**: fixed light mode
- **Dark**: fixed dark mode

## Acceptance Criteria
- `System` should be the default option.
- The selected preference should persist across application restarts.
- When `System` is selected, the interface responds to OS theme changes dynamically.
- Text, controls, borders, focus indicators, disabled states, and progress elements remain readable with high contrast in both themes.
- Existing compact layout and keyboard accessibility are preserved.

