# Agent Guidance & Standing Rules

## STANDING RULES
- **Git Branching**: Work on a new git branch per task; never commit to `main`. One commit per sub-task, descriptive messages.
- **Scope Discipline**: Read only the plan sections named in the task (`docs/audit/update-plan.md`). Do not start other workstreams.
- **Minimal Changes**: No unrelated refactors or dependency changes. Ask before deleting files or adding dependencies.
- **Architecture & Licensing**: Keep Tauri v2 + React 19 + clean architecture. Do not copy MDM code or text (MDM is GPL-3.0; we are MIT). Clean-room implementation only.
- **Build Discipline**: Production/benchmark binaries MUST be built with embedded frontend assets. Never hardcode `custom-protocol` into `src-tauri/Cargo.toml` (which breaks `tauri dev` HMR). Build using `bun tauri build --no-bundle -f dev-tools` or `cargo build --release --features tauri/custom-protocol,dev-tools` after `bun run frontend:build`. Never benchmark against a binary connecting to `localhost:5173`.
- **Verification**: Run `cargo test`, `cargo clippy`, renderer tests (`bun run test` / `bun run typecheck`) before reporting. Tag claims `[VERIFIED-BY-READING]` / `[VERIFIED-BY-RUNNING]` / `[ESTIMATE]`.
- **Platform Integrity**: Windows behavior cannot be verified on this Mac: write a numbered manual checklist for the owner instead of claiming it works.
- **Reporting Contract**: When done, STOP and report: what changed, files touched, tests added, what is unverified, risks.
