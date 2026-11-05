# Contributing

Issues and pull requests are welcome. This checkout is an early Rust + Tauri migration, not the current published
Windows preview or a finished replacement for every commercial downloader
feature.

Read the [contributor start guide](docs/CONTRIBUTOR-START-HERE.md) before choosing
work. It explains our roadmap, labels and implementation order. For new feature
work, pick an approved issue and agree your approach and ownership with the
maintainer before coding. Propose features outside the backlog for approval first.

## Development

Install Bun, Rust, and Tauri's platform prerequisites.
Fork this repository, clone your fork, and create a branch for one focused
change.

```powershell
bun install --frozen-lockfile
bun run test
bun run dev
```

`bun run build` builds the current Tauri app. Tauri packaging and the Rust
download service are still in progress; this branch is not ready to distribute.
Review the [migration plan](docs/TAURI-MIGRATION-PLAN.md) before choosing work
and [release requirements](docs/RELEASING.md) before distributing binaries.

## Pull requests

- Link the implementation issue and its parent goal, and identify prerequisites.
- Use `Closes #...` only for an issue fully completed by the PR; use `Refs #...`
  for partial work and parent goals. Keep each PR focused and wait for prerequisite
  changes to merge and pass validation before merging dependent work.
- Explain the problem, the change, and how you tested it.
- Add regressions for parsing, transfers, authentication, and lifecycle changes.
- Preserve the compact Windows layout and keyboard accessibility.
- Distinguish local fixture tests from live-browser verification.
- Keep captured URLs, cookies, tokens, browser profiles, and downloads out of Git.
- Do not include proprietary IDM code, extracted binaries, or DRM bypasses.
- Do not use `pull_request_target` to run untrusted contributor code with secrets.

By contributing, you agree to license your original contributions under the MIT
license. Third-party materials must retain compatible upstream notices.

Please be respectful. Do not post private media or session data in bug reports.
