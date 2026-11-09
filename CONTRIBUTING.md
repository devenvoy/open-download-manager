# Contributing

Issues and pull requests are welcome! Open Download Manager is an open-source, high-performance desktop download manager built with Rust and Tauri v2.

Read the [contributor start guide](docs/CONTRIBUTOR-START-HERE.md) before choosing
work. It explains our architecture, issue guidelines, and development workflow. Propose large features or architectural changes in an issue for discussion first.

## Development

Install Bun, Rust, and Tauri's platform prerequisites.
Fork this repository, clone your fork, and create a branch for one focused
change.

```bash
bun install --frozen-lockfile
bun run test
bun run dev
```

`bun run build` builds the desktop application for your platform. See [INSTALL.md](docs/INSTALL.md) for platform distribution guidelines.

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
