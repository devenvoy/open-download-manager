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

## Comments (4)

### Comment #1 by [@frpboy](https://github.com/frpboy) • `2026-10-28T10:14:45Z` (CONTRIBUTOR)
[Original Comment Link](https://github.com/Ahmadkhattak1/awesome-downloader/issues/13#issuecomment-6466312842)

I'd like to take a focused first step on #13: add a CI regression check that parses the renderer JavaScript (and, where practical, verifies basic renderer startup wiring) so syntax errors in renderer/app.js cannot pass solely because the current npm test glob covers tests/integration/extension files.

Proposed scope:
- Add a small Node-based validation test/script for renderer/app.js syntax using the installed Node runtime (no new dependency).
- Wire it into the existing `npm test` or the Windows CI workflow with read-only permissions.
- Add a negative/fixture test if it can be done without executing arbitrary contributor code or weakening the assertion.
- Keep the diff limited to the CI/test concern; no release workflow or secret permissions changes.

Validation plan: `npm ci`, `npm test`, and confirm CI fails for an intentionally invalid renderer fixture while passing for the real renderer source. I can validate unit-level behavior, but I cannot claim Windows packaging or live app startup verification without a Windows run.

Could you confirm this scope and ownership before implementation, per CONTRIBUTING.md and docs/CONTRIBUTOR-START-HERE.md? I checked the current open backlog/PR list and didn't find an open PR implementing this exact renderer syntax check.

---

### Comment #2 by [@frpboy](https://github.com/frpboy) • `2026-10-28T10:53:09Z` (CONTRIBUTOR)
[Original Comment Link](https://github.com/Ahmadkhattak1/awesome-downloader/issues/13#issuecomment-6467103575)

CI/CD audit follow-up (read-only review of upstream `main` at `2200df124f6f9d662c1c891bce9343b0ffe8e10c`, checked 2026-10-28):

## Existing checks confirmed
- `.github/workflows/test.yml` is the only test workflow. It runs on `push`, `pull_request`, and `workflow_dispatch`; uses `windows-latest`, Node 22, `npm ci`, `npm test`, a 20-minute job timeout, and workflow-level `contents: read`.
- `package.json` defines `npm test` as `node --test tests/*.test.mjs integration/*.test.cjs extension/*.test.cjs`.
- The explicit test glob does not include `renderer/*.js`; there is no separate renderer syntax/startup validation command in the workflow or package scripts. This matches the narrow gap proposed above.
- `.github/workflows/release.yml` is manual `workflow_dispatch` only. It repeats install/tests, runs `npm run dist`, writes a SHA-256 checksum, and uploads an unsigned release-candidate artifact for 7 days. It does not publish a GitHub Release. Issue #41 already tracks the tag-triggered draft-prerelease work, so this audit does not propose a duplicate.
- The latest default-branch push run (#44, SHA `2200df1...`) succeeded. The latest PR runs for #49, #50, and #51 report `action_required`, not a test pass. Earlier PR runs report failure, but the accessible job endpoint returned no job details, so I cannot safely attribute those failures to tests versus setup/approval without a maintainer checking the Actions UI.

## Minimal implementation proposal for this issue
1. Add a no-new-dependency validation that parses `renderer/app.js` using the existing Node runtime.
2. Make the current PR test command execute that check, with a negative fixture proving invalid syntax fails.
3. Keep the existing Windows test workflow, triggers, read-only permissions, and release workflow unchanged in this focused change.
4. Validate with `npm ci`, `npm test`, a negative-fixture test, and the actual upstream Windows Actions run. Clearly mark platform startup and packaging checks that are not run.

No CI files have been changed. This is an evidence update and proposed scope only; per the contributor guide, I am still waiting for maintainer agreement on ownership/scope before implementation. Detailed local reports can be shared in the contribution thread if useful.

---

### Comment #3 by [@frpboy](https://github.com/frpboy) • `2026-10-30T10:36:17Z` (CONTRIBUTOR)
[Original Comment Link](https://github.com/Ahmadkhattak1/awesome-downloader/issues/13#issuecomment-6520645384)

## CI/CD inventory update — 2026-10-30

Audited the current upstream `main` at `44159908b02f890e8aa43e4c0af0f0e18ad54858`. This is an evidence update to the existing CI issue; no workflow files have been changed.

### Existing pipelines

**`.github/workflows/test.yml`** (`c744f990f98354ae265533bffd650e3d123fcb44`)
- Triggers: push, pull_request and workflow_dispatch.
- Runner/runtime: windows-latest, Node.js 22, npm.
- Steps: actions/checkout@v4, actions/setup-node@v4 with npm cache, `npm ci`, `npm test`.
- Safety: workflow permission is `contents: read`; 20-minute job timeout; no secrets, deployment, release publishing or `pull_request_target`.
- `npm test` runs Node's built-in runner over `tests/*.test.mjs integration/*.test.cjs extension/*.test.cjs`.

**`.github/workflows/release.yml`** (`0490974b6f86c02f6251c93f717421d7dd65e676`)
- Trigger: workflow_dispatch only.
- Runner/runtime: windows-latest, Node.js 22, npm.
- Steps: `npm ci`, `npm test`, `npm run dist`, SHA-256 checksum, unsigned installer + checksum artifact upload (7-day retention).
- Safety: `contents: read`; no pull-request trigger, no release-write token and no public release publishing. `npm run dist` is the local package command; the workflow uploads an artifact only.
- Tag-triggered draft-prerelease automation is already tracked separately by issue #41; this audit does not duplicate it.

### Existing coverage now present on main

- Merged #55 added manifest/HTML local-reference checks and syntax checks for `renderer/app.js` and browser entry scripts, plus a negative invalid-syntax assertion. The original renderer syntax gap in this issue is therefore covered.
- `integration/native-host.test.cjs` includes message framing, oversized/truncated input rejection, URL/field validation, authentication and handoff checks.
- `tests/engine.test.mjs` covers exact-byte transfer reconstruction, resume/restart, cancellation cleanup, server range failures, retries/Retry-After and exclusion of request credentials from persisted history/snapshots.
- The latest default-branch test run at this baseline, [run #40922272577](https://github.com/Ahmadkhattak1/awesome-downloader/actions/runs/40922272577), passed both `npm ci` and `npm test`. The release-tag push also ran the test workflow successfully: [run #40922498594](https://github.com/Ahmadkhattak1/awesome-downloader/actions/runs/40922498594).

### Remaining findings and limitations

1. The workflows reference `actions/checkout@v4`, `actions/setup-node@v4` and `actions/upload-artifact@v4` by mutable major-version tags, not verified full commit SHAs. This is a supply-chain hardening opportunity, not evidence of a current compromise.
2. The current package scripts do not define lint, formatting, TypeScript type-check or coverage commands. PR #36 proposes these alongside TypeScript/Yarn migration and a Windows/macOS matrix, but it is a broad 71-file/8-commit bundle. The maintainer's latest PR #36 comment requested that the Electron packaging fix be separated first and the TypeScript and npm-to-Yarn decisions remain separately reviewable. PR #70 already proposes the isolated Electron packaging fix. I have not duplicated or overwritten this work.
3. Workflow run inventory returned 98 records: 34 push successes, 10 PR successes, 16 `action_required`, and 38 failures across historical PR runs. The connected job-detail endpoint did not return job records for those historical failures, so their causes cannot safely be attributed. Recent main and the two latest frpboy PR test runs are successful.
4. The GitHub branch-protection endpoint returned 403 `Resource not accessible by integration`. Required status-check configuration and ruleset enforcement therefore remain unverified; this is not proof that protection is absent.
5. No coverage threshold/report, SBOM, dependency-audit job, YAML workflow linter or explicit concurrency cancellation appears in the two checked-in workflows. These are potential follow-ups, not all necessary for the current Windows preview.

### Narrow proposal for #13

Please confirm whether you want one isolated hardening PR that pins only the existing official actions in the two workflows to independently verified full commit SHAs, while preserving current triggers, Node/npm versions, read-only permissions, test commands, artifact behavior and release gates. No new actions, secrets, provider migration, release trigger or platform-support claim would be introduced.

I have not implemented this proposal because the contributor guide requires maintainer agreement on scope/ownership first. The broader PR #36 toolchain/matrix changes remain separate and should not be bundled into this hardening step.

---

### Comment #4 by [@abdullah129026](https://github.com/abdullah129026) • `2026-10-31T04:49:32Z`
[Original Comment Link](https://github.com/Ahmadkhattak1/awesome-downloader/issues/13#issuecomment-6539209282)

Hi Ahmad, thanks for the pointer from #5. I'd like to claim a slice of this one.

frpboy already has the renderer syntax/startup check covered, so I will not duplicate that. The slice I am offering is the native-bridge trust boundary tests plus the transfer regression tests, written the same way as the http tests in #5: small `node --test` suites, no new dependencies, running inside the existing `npm test`.

My plan:

1. Regression tests for the `core/bridge.mjs` trust boundary. The bridge takes input that should be treated as untrusted, so the tests feed it bad input (missing fields, wrong types, oversized payloads) and pin the expected behavior: clean rejection, no crash, no partial work.
2. The `core/http.mjs` regression tests from #5, brought in as their own focused PR under this issue (probe response parsing, redirect handling that does not leak auth headers, header cleaning).
3. For each check, negative evidence first: I will show it failing against a deliberately broken fixture, then passing on the real code, so the check is proven to catch the regression it claims to catch.

Process wise: no workflow permission changes, no `pull_request_target`, no secrets, nothing touching release publishing. One focused PR per change, each linked to this issue with before and after test output. I will run `npm ci` and `npm test` on my fork before opening anything.

Two questions before I start. Does this split sit fine next to frpboy's renderer work, and which would you rather see first, the bridge tests or the http tests?

Happy to adjust the scope if you want it cut differently.

---
