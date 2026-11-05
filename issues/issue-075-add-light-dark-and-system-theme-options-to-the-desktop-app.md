---
number: 75
title: "Add light, dark, and system theme options to the desktop app"
state: OPEN
author: itzsaad09
created_at: 2026-10-30T17:57:48Z
updated_at: 2026-10-30T17:57:48Z
closed_at: null
url: https://github.com/Ahmadkhattak1/awesome-downloader/issues/75
labels: []
assignees: []
comments_count: 0
---

# Issue #75: Add light, dark, and system theme options to the desktop app

- **Status**: `OPEN`
- **Author**: [@itzsaad09](https://github.com/itzsaad09)
- **Created**: `2026-10-30T17:57:48Z`
- **Updated**: `2026-10-30T17:57:48Z`
- **Original Issue**: [#75 on GitHub](https://github.com/Ahmadkhattak1/awesome-downloader/issues/75)

---

## Description

## Problem

The desktop application currently uses a fixed visual theme. Users cannot select a light or dark appearance or have the application follow the Windows theme automatically.

## Proposed change

Add an application theme setting with three options:

- System — automatically follow the Windows light/dark preference
- Light
- Dark

## Expected behavior

- `System` should be the default option.
- The selected preference should persist after restarting the application.
- When `System` is selected, the interface should respond to Windows theme changes without requiring a restart.
- All desktop windows and dialogs should use the same theme.
- Text, controls, borders, focus indicators, disabled states, and progress elements should remain readable in both themes.
- Existing compact layout and keyboard accessibility should be preserved.
- The browser extension and on-page format picker are outside the scope of this issue.

## Possible implementation

- Store the preference in the application’s existing configuration.
- Use Electron’s `nativeTheme` to detect and monitor the operating-system theme.
- Apply a theme attribute or class to each renderer window.
- Define shared CSS custom properties for light and dark color tokens.
- Notify all open windows whenever the preference or effective system theme changes.

## Validation

- Test all three options.
- Confirm that the preference persists after restart.
- Confirm that open windows update when the Windows theme changes in `System` mode.
- Verify keyboard focus and readable contrast in both themes.

I would like to work on this issue.



---

## Comments (0)

*No comments on this issue yet.*
