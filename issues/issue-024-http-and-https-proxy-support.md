---
number: 24
title: "HTTP and HTTPS proxy support"
state: OPEN
author: devenvoy
created_at: 2026-11-05T20:04:45Z
updated_at: 2026-11-05T20:04:45Z
url: https://github.com/devenvoy/open-download-manager/issues/24
labels: ["enhancement"]
---

# Issue #24: HTTP and HTTPS proxy support

- **Status**: `OPEN`
- **Author**: [@devenvoy](https://github.com/devenvoy)
- **Created**: `2026-11-05T20:04:45Z`
- **Labels**: `enhancement`
- **Issue Link**: [#24 on GitHub](https://github.com/devenvoy/open-download-manager/issues/24)

---

## Description

## Objective
Allow configuring HTTP and HTTPS proxy servers for network requests and downloads.

## Acceptance Criteria
- Add proxy configuration fields to application settings (Host, Port, Protocol, and optional authentication).
- Configure the native HTTP transfer engine (`reqwest`) to route connections through the configured proxy.
- Provide a connection test button in settings to verify proxy reachability.

