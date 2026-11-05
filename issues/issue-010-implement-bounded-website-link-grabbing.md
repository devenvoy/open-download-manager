---
number: 10
title: "Implement bounded website/link grabbing"
state: OPEN
author: devenvoy
created_at: 2026-11-05T19:50:01Z
updated_at: 2026-11-05T19:50:01Z
url: https://github.com/devenvoy/open-download-manager/issues/10
labels: ["help wanted"]
---

# Issue #10: Implement bounded website/link grabbing

- **Status**: `OPEN`
- **Author**: [@devenvoy](https://github.com/devenvoy)
- **Created**: `2026-11-05T19:50:01Z`
- **Labels**: `help wanted`
- **Issue Link**: [#10 on GitHub](https://github.com/devenvoy/open-download-manager/issues/10)

---

## Description

> *Mirrored from upstream issue [#20](https://github.com/Ahmadkhattak1/awesome-downloader/issues/20) originally opened by @Ahmadkhattak1 on 2026-10-25T21:07:38Z*

## Scope and acceptance
Replace the current placeholder with a preview-and-select flow for downloadable links. Propose limits and supported page types first. Bound pages, depth, concurrency and duplicate collection; prevent unintended cross-site crawling and automatic downloads before confirmation. Support cancellation and clear partial failures; never collect browser profiles or expose session credentials. Separate discovery/fixtures from desktop review/queue integration.


## Working agreement
Approved objective. Claim this issue with an approach and test plan before starting to avoid duplicate work. One focused PR per independently testable change; use draft PRs for dependencies. Each PR must link its issue, include regression evidence and receive maintainer review. Keep main usable on Windows. No unrelated dependency upgrades, credentials, proprietary code or automatic release publishing. New scope beyond this issue goes through the maintainer. Passing unit tests alone does not establish platform/browser support.

