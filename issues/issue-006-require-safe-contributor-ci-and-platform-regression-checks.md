---
number: 6
title: "Require safe contributor CI and platform regression checks"
state: OPEN
author: devenvoy
created_at: 2026-11-05T19:49:50Z
updated_at: 2026-11-05T19:58:27Z
url: https://github.com/devenvoy/open-download-manager/issues/6
labels: ["help wanted"]
---

# Issue #6: Require safe contributor CI and platform regression checks

- **Status**: `OPEN`
- **Author**: [@devenvoy](https://github.com/devenvoy)
- **Created**: `2026-11-05T19:49:50Z`
- **Labels**: `help wanted`
- **Issue Link**: [#6 on GitHub](https://github.com/devenvoy/open-download-manager/issues/6)

---

## Description

> *Mirrored from upstream issue [#13](https://github.com/Ahmadkhattak1/awesome-downloader/issues/13) originally opened by @Ahmadkhattak1 on 2026-10-25T21:07:19Z*

## Scope and acceptance
Review existing test/release workflows and add missing renderer syntax/startup, native-bridge trust-boundary and transfer-integrity checks. Run untrusted fork PR tests with read-only permissions and no release credentials; do not execute contributor code with pull_request_target secrets. Document required checks and maintainer review before merge. Release publishing remains a separate maintainer-controlled step. Verify checks actually fail on a renderer syntax error and unsafe handoff regression.


## Working agreement
Approved objective. Claim this issue with an approach and test plan before starting to avoid duplicate work. One focused PR per independently testable change; use draft PRs for dependencies. Each PR must link its issue, include regression evidence and receive maintainer review. Keep main usable on Windows. No unrelated dependency upgrades, credentials, proprietary code or automatic release publishing. New scope beyond this issue goes through the maintainer. Passing unit tests alone does not establish platform/browser support.

