---
number: 13
title: "Require safe contributor CI and platform regression checks"
state: OPEN
author: Ahmadkhattak1
created_at: 2026-10-25T21:07:19Z
updated_at: 2026-10-31T04:49:32Z
closed_at: null
url: https://github.com/Ahmadkhattak1/awesome-downloader/issues/13
labels: ["help wanted"]
assignees: []
comments_count: 4
---

# Issue #13: Require safe contributor CI and platform regression checks

- **Status**: `OPEN`
- **Author**: [@Ahmadkhattak1](https://github.com/Ahmadkhattak1)
- **Created**: `2026-10-25T21:07:19Z`
- **Updated**: `2026-10-31T04:49:32Z`
- **Labels**: `help wanted`
- **Original Issue**: [#13 on GitHub](https://github.com/Ahmadkhattak1/awesome-downloader/issues/13)

---

## Description

## Scope and acceptance
Review existing test/release workflows and add missing renderer syntax/startup, native-bridge trust-boundary and transfer-integrity checks. Run untrusted fork PR tests with read-only permissions and no release credentials; do not execute contributor code with pull_request_target secrets. Document required checks and maintainer review before merge. Release publishing remains a separate maintainer-controlled step. Verify checks actually fail on a renderer syntax error and unsafe handoff regression.


## Working agreement
Approved objective. Claim this issue with an approach and test plan before starting to avoid duplicate work. One focused PR per independently testable change; use draft PRs for dependencies. Each PR must link its issue, include regression evidence and receive maintainer review. Keep main usable on Windows. No unrelated dependency upgrades, credentials, proprietary code or automatic release publishing. New scope beyond this issue goes through the maintainer. Passing unit tests alone does not establish platform/browser support.



---

---

## Comments (0)

*No comments on this issue yet.*
