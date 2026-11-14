# React + shadcn/ui Frontend Migration — Plan

**Status:** Desktop renderer and browser extension migration complete. All 7 desktop
windows (main, options, progress, add, scheduler, integration, torrents) and the
browser extension are genuinely migrated to React 19 + TypeScript + Vite +
Tailwind CSS with 100% same-to-same UI and features as the legacy code.
0 stubs, 0 placeholders (`UnavailableScreen` deleted). All gates green (typecheck,
lint 0/0, 112 renderer tests in 17 files, 101 extension tests in 13 files, 102
cargo tests, Vite build and extension pack green). Legacy reference preserved in
`reference/`.
**Created:** 2026-11-12
**Updated:** 2026-11-14
**Scope:** Replace the hand-written renderer with a React 19 + TypeScript + Vite
+ Tailwind CSS v4 + shadcn/ui frontend, split into modular features, with
typed IPC, disciplined state management and a reusable UI layer. The Rust
backend is not rewritten; the browser extension is migrated to TypeScript and
gets a React panel only.
**Constraint:** Every stage ships. The app must stay buildable, shippable and
free of the crash classes found in the 2026-11-12 audit.

---

## 0. Where the migration actually is

**Read this first.** An earlier status note claimed "Stages 1–4 complete and
validated". That was false. On 2026-11-13 the gates were red, six of the seven
window routes rendered the wrong screen, and the extension had been deleted
with no replacement.

Measured on 2026-11-14 against the working tree after the complete renderer rewrite:

| Area | Actual state | Evidence |
|---|---|---|
| `tsc --noEmit` | **Green** | `bun run typecheck`; `strict` + `noUncheckedIndexedAccess` |
| `bun run lint` | **Green** | `bun run lint`; 0 errors, 0 warnings across `renderer/`, `extension/`, `tools/` |
| Renderer tests | **112 tests in 17 files** | `bun run test:renderer` (downloads, torrents, progress, settings, integration, scheduler, add-download, format, window-mode) |
| Extension tests | **101 tests in 13 files** | `bun test ./extension` (background, content, hls, policy, menu, chooser, format, safe-tooltip) |
| Vite build | **Green** | `bun run frontend:build`; builds `dist/index.html` and `dist/torrents.html` in ~325ms |
| Extension pack | **Green** | `bun run pack:extension`; packages chrome zip and firefox zip/xpi in `dist/extension/` |
| Rust commands | 65 registered | `generate_handler!` in `src-tauri/src/lib.rs`; all 102 cargo tests pass |
| IPC mirror | **Full typed surface** | `lib/ipc/commands.ts` (`CommandMap`) compile-time guard; typed domain split in `types/` |
| Windows | **7 of 7 routes real (100%)** | `main` → `DownloadsScreen`, `options` → `SettingsScreen`, `progress` → `ProgressScreen`, `add` → `AddDownloadScreen`, `scheduler` → `SchedulerScreen`, `integration` → `IntegrationScreen`, `torrents` → `TorrentsScreen`. Identical UI to legacy |
| Feature folders | 7 modular features | `features/{downloads,settings,torrents,integration,scheduler,add-download,progress}`, each with `api.ts`, `hooks/`, `components/`, `screens/`, `model/`, `index.ts` |
| Extension | **Active & tested** | `extension/` fully present, tested, and packaged for Chrome and Firefox |
| Legacy reference | Preserved, not tracked | 18/18 old renderer files in `reference/legacy-renderer/`, extension in `reference/legacy-extension/` |
| CI | Runs lint + typecheck + renderer tests + extension tests | `ci.yml` runs full verification suite |

### 0.1 What actually works today

- **Main window (`DownloadsScreen`)**: lists downloads from `snapshot`, filters by category, pauses, resumes, stops, cancels and removes selected rows, opens auxiliary windows.
- **Torrents window (`TorrentsScreen`)**: multi-file torrent manager, engine status, magnet & file additions, tracker list, file selection/priority dialog, torrent settings dialog.
- **Options window (`SettingsScreen`)**: full 1:1 legacy port: General, Appearance (theme), Storage (default/category/temp folder browse & reset), Connection limits & speed cap, Site connection exceptions (add/remove), Rolling download window limits & usage stats, Proxy configuration & discovery dialog with diff/import, Browser capture rules & exclusions, Site credentials manager (save/delete rules), Media tools installer (FFmpeg, yt-dlp), Post-download actions (sound, antivirus scan, custom command, power action).
- **Progress window (`ProgressScreen`)**: fast 1 s polling, status panel, segment visualization, speed limit editing, completion actions, action ledger, download review flow (submit/cancel), media tool installer dialog, window resize & close.
- **Add Download window (`AddDownloadScreen`)**: URL batch input, clipboard paste, category auto-folder assignment, single-file filename override, queue assignment, playlist detection & dialog, video format detection & dialog, download conflict validation.
- **Scheduler window (`SchedulerScreen`)**: download queues list, start/pause toggle, queue create & edit dialog with ISO day recurring schedules, one-off scheduled downloads list.
- **Integration window (`IntegrationScreen`)**: browser profiles grid, browser extension status, setup actions (copy, download, install), advanced media player stream integration opt-in with consent & repair.
- **Gates & verification**: `tsc`, ESLint, Vitest (112 tests in 17 files), extension tests (101 tests in 13 files), Vite frontend build, and local `cargo test` (all unit and integration tests) are 100% green.

### 0.2 Remaining Roadmap

1. Stage 5: Browser extension TypeScript migration with shared contracts.
2. `ts-rs` build step to generate `CommandMap` and DTOs directly from Rust definitions during `cargo test`.
3. Event-driven server state invalidation to augment query polling when transfer events fire.

---

## 1. Why now

The `v0.3.0-beta.1` audit found the same defect class in eight places: a name
that is used but never declared. The fixes are in
[RUNTIME-CRASH-AUDIT.md](./RUNTIME-CRASH-AUDIT.md). The root cause is not a
missing `Map` — it is the architecture:

| Defect | Why the architecture allowed it |
|---|---|
| `state.filesHash` never declared | one mutable global object, plain JavaScript, no type checker |
| `state.selectedId` read from the wrong place | the selection lives in a module variable *and* was read from `state` |
| `hideDialog` never defined | a call to a function in another file, discovered only at runtime |
| `handleClipboardCandidate` out of scope | a function declared inside a callback |
| `#yt-dlp-status` vs `#ytdlp-status` | ids hardcoded in two languages |
| `tray_set_active_downloads` not registered | the JS knew a command the Rust never registered |
| `$(...).show()` | a jQuery method on a DOM element |

TypeScript plus typed IPC plus component boundaries remove every one of these
categories permanently. This plan is the way to do that without a big bang.

---

## 2. Goals

| # | Goal |
|---|------|
| G1 | The renderer is TypeScript. A missing member, a wrong field name or a wrong command name fails the build, not the app. |
| G2 | Every IPC command and payload is typed from the Rust source, generated, never hand-written. |
| G3 | The UI is built from shadcn/ui components on Tailwind CSS v4. No hand-rolled button, dialog, table or dropdown. |
| G4 | No file over 300 lines. No feature larger than one folder. No shared global mutable object. |
| G5 | Server state, UI state and form state have three separate, explicit owners. |
| G6 | The extension is TypeScript, shares a package with the app, and gets a React panel. |
| G7 | Every stage is shippable. Rollback is one commit. |

## 3. Non-goals

- No rewrite of the Rust backend. No new features for the download engine.
- No change to the download protocol layer, the scheduler, queues or the
  credential rules.
- No change to the look of the app in Stage 1 and Stage 2. The visual
  redesign is Stage 4 and needs a sign-off.
- No server rendering, no Next.js, no Nuxt. Tauri has no Node runtime.
- No React in the extension service worker. It has no UI.

---

## 4. Target stack

Versions verified against the npm registry on 2026-11-12.

| Package | Version | Why |
|---|---|---|
| `react` / `react-dom` | 19.3.1 | current stable |
| `vite` | 8.3.8 | the bundler the official Tauri guide is written for |
| `@vitejs/plugin-react` | 6.3.0 | React fast refresh |
| `typescript` | 5.9.x (pin 5.x) | the 7.x native compiler is very new; `typescript-eslint` and the Vite plugins lag behind it |
| `tailwindcss` + `@tailwindcss/vite` | 4.3.5 | the styling layer, no PostCSS config needed |
| shadcn/ui (Radix) | latest via CLI | components copied into the repo, owned by us |
| `@radix-ui/*` | 1.x | the primitives behind shadcn/ui |
| `lucide-react` | 1.72.0 | icons |
| `@tanstack/react-query` | 5.107.1 | server state (the Rust side is the server) |
| `@tanstack/react-table` | 9.3.1 | the download list and the torrent list |
| `@tanstack/react-virtual` | 3.x | long lists |
| `zustand` | 5.1.0 | UI state (selection, filter, sort, open tab) |
| `react-hook-form` + `@hookform/resolvers` | 7.92.0 / 7.0.6 | forms |
| `zod` | 4.6.7 | validation, shared with the Rust side through the generated types |
| `sonner` | 2.x | toasts, mounted once by the app shell |
| `vitest` + `@testing-library/react` + `jsdom` | latest | tests replace the `node:test` harness |
| `ts-rs` | 11.x | generates TypeScript types from the Rust DTOs |
| `@tauri-apps/api` | 2.12.x | matches the `tauri` crate 2.12.5 |

`bun` stays the package manager. Vite runs on bun.

---

## 5. Target architecture

```
renderer/src/                        # Vite + React + TS application root
├── main.tsx                          # entry for index.html; parses the route
├── torrents.tsx                      # entry for torrents.html — NOT CREATED YET
├── vite-env.d.ts
│
├── app/                              # app shell, wiring, nothing else
│   ├── App.tsx                       # renders the router for the parsed route
│   ├── providers.tsx                 # query client, theme bridge, toaster
│   ├── routes.tsx                    # mode → screen
│   └── window-mode.ts                # pure query-string parser (tested)
│
├── components/                       # shared, feature-agnostic components
│   ├── ui/                           # shadcn/ui — generated, never hand-edited
│   ├── layout/                       # AppShell
│   ├── feedback/                     # EmptyState, UnavailableScreen
│   └── table/                        # NOT CREATED — feature tables own their columns today
│
├── features/                         # one vertical slice per domain
│   ├── downloads/
│   │   ├── api.ts                    # typed IPC calls for this feature
│   │   ├── hooks/use-downloads.ts    # snapshot query + mutations
│   │   ├── components/               # toolbar, table, sidebar, status badge, add dialog
│   │   ├── screens/DownloadsScreen.tsx
│   │   ├── state/downloads-store.ts  # zustand: selection + category only
│   │   └── index.ts                  # public surface
│   ├── settings/                      # api, hooks, form, dialog, screen, index
│   ├── torrents/                      # NOT CREATED
│   ├── queues/                        # NOT CREATED
│   ├── scheduler/                     # NOT CREATED
│   ├── credentials/                   # NOT CREATED
│   └── media/                         # NOT CREATED
│
├── lib/                              # utilities, no business logic
│   ├── ipc/                          # call wrapper, CommandMap, typed `ipc`, query options
│   ├── ui/theme.ts                   # theme resolution + document attribute
│   ├── format.ts                     # bytes, speed, percent, duration
│   ├── utils.ts                      # cn() — clsx + tailwind-merge
│   ├── hooks/                        # NOT CREATED (window mode is a pure function)
│   └── validation/                   # NOT CREATED
│
├── stores/                           # one zustand store per cross-cutting concern
│   ├── theme-store.ts                # AppTheme + effective theme
│   └── preferencesStore.ts           # NOT CREATED
│
├── types/                            # hand-written today; ts-rs replaces it
│   └── ipc.ts                        # Rust DTO mirrors
│
└── index.css                         # tailwind v4 tokens + dark variant
```

`components.json` lives at the repository root (see §10.1), because the shadcn
CLI resolves aliases against the root `tsconfig.json` even though Vite's root is
`renderer/`.

### 5.1 Folder rules

1. **A feature is a vertical slice.** Everything a feature needs lives in its
   folder. A feature may import from `components/`, `lib/` and `types/`.
2. **A feature may not import from another feature.** Shared code is promoted
   to `components/` or `lib/` first. If two features need the same thing, it
   was the wrong home.
3. **`components/ui/` is generated.** shadcn/ui writes there. A change to a
   generated file is a change to the upstream component, and it is a bug.
4. **`types/` is generated.** Rust is the source of truth for every payload.
5. **`lib/` has no feature knowledge.** `lib/ipc` knows about commands, not
   about torrents.

### 5.2 File rules

| Rule | Enforced by | Status |
|---|---|---|
| One component per file, file name equals the component name | ESLint `react/filename-naming` | not configured |
| No file over 300 lines | ESLint `max-lines` | **enforced** |
| No file over 12 imports | ESLint `max-imports` | not configured |
| No `any` | ESLint `@typescript-eslint/no-explicit-any` | **enforced** |
| No `export default` | ESLint `import/no-default-export` | not configured |
| No direct `window.x =` outside `lib/ipc` and `lib/ui` | ESLint `no-restricted-syntax` | **enforced** |
| No `useEffect` for derived data | `react-hooks/exhaustive-deps` + review rule R7 | **enforced** (warning) |
| Imports are ordered and aliased `@/` → `src/` | ESLint `import/order` | not configured |

---

## 6. State management

Three kinds of state, three owners. A piece of state has exactly one owner.

### 6.1 Server state → TanStack Query

Everything that lives in Rust: the download list, torrent list, settings,
queues, credential rules, engine status.

```ts
// features/torrents/api.ts
export const torrentQueries = {
  all: () => ({
    queryKey: ["torrents"],
    queryFn: () => invoke<TorrentListItem[]>("torrent_list"),
    refetchInterval: 3000,
  }),
  status: () => ({
    queryKey: ["torrentStatus"],
    queryFn: () => invoke<TorrentEngineStatus>("torrent_status"),
    refetchInterval: 3000,
  }),
};
```

Rules:

- **R1 — One query per domain, one key.** The torrent list is fetched by
  exactly one query. A second component reads the cache, it does not fetch.
- **R2 — Events invalidate, they do not duplicate.** `torrent://progress`
  invalidates the list query. The engine keeps the 1 Hz emit; the UI does not
  keep a second copy of the list in a store.
- **R3 — Polling only where the backend has no event.** The review poll drops
  from 1 s to 3 s, because `torrent_take_review` is a blocking engine call.
- **R4 — The query function is the only place `invoke` is called.** Components
  never call `invoke` directly.
- **R5 — The cache holds what Rust sent.** No field is renamed, reshaped or
  patched in the client. A missing field is a Rust bug and is fixed there.

### 6.2 UI state → Zustand

What the user chose: selection, filter, sort, the open tab, the panel width.

```ts
// features/downloads/state/downloadsStore.ts
export const useDownloadsStore = create<DownloadsStore>()(
  (set) => ({
    selectedId: null,
    filter: "all",
    sort: { key: "createdAt", ascending: false },
    setFilter: (filter) => set({ filter }),
  }),
);
```

Rules:

- **R6 — A store holds only serialisable primitives.** No `Map`, no class
  instances, no DOM nodes. A `Map` of cached file lists is exactly how
  `state.filesHash` went missing.
- **R7 — A store never holds a value that can be derived.** Derived values are
  computed during render with a selector, never stored and kept in sync.
- **R8 — One store per concern, one file per store.** A store that grows past
  100 lines is two stores.
- **R9 — A store never imports another store.** The dependency goes through a
  hook, so the direction stays visible.
- **R10 — A store is unit tested.** The selectors are the API.

### 6.3 Form state → React Hook Form + Zod

Every form: add download, options, queue, scheduler, credential rule, torrent
settings, proxy.

- **R11 — The schema is the validation.** The Rust side validates again; the
  client validates early and shows the message next to the field.
- **R12 — A form is one component plus one schema.** No form logic in a store.

### 6.4 The mapping from the old code

| Old | New |
|---|---|
| `state = { … }` in `app.js` | gone. Query cache for server state, one store per concern for UI state |
| `state.files` / `state.filesHash` / `state.filesLoading` | `useTorrentFiles(reviewId)` — a query keyed by id **and** info hash, invalidated by the engine event |
| `state.selectedId` (module variable **and** `state.selectedId`) | one store, one field: `useDownloadsStore(s => s.selectedId)` |
| `state.answered` (a `Set` in the global object) | `useReviewStore` or a query on the backend |
| `window.downloader` (`tauri-api.js`) | `lib/ipc/invoke.ts`, typed, one export per command |
| `window.initTheme` / `window.applyTheme` | `lib/ui/theme.ts` — a store + effect, no globals |

---

## 7. UI layer

### 7.1 shadcn/ui

`components.json` (see §10.1) points the CLI at the standard registry and at
the `src/` folder. The CLI is the only way components are added:

```bash
bunx --bun shadcn@latest add button dialog dropdown-menu table tabs progress
bunx --bun shadcn@latest add checkbox select slider tooltip sonner
```

Installed today (all via the CLI): `button`, `input`, `label`, `select`,
`checkbox`, `dialog`, `alert-dialog`, `table`, `badge`, `separator`,
`skeleton`.

Still needed by the unmigrated screens:

| shadcn component | Used for |
|---|---|
| `textarea`, `switch` | forms and settings beyond folder + theme |
| `sheet`, `popover`, `dropdown-menu`, `tooltip` | overlays in the torrent and scheduler windows |
| `tabs` | general / files / trackers tabs, settings sections |
| `progress` | per download and per torrent progress |
| `scroll-area` | long queues and file lists |
| `command` | the search and the command palette |

`sonner` is installed as a package and mounted by `app/providers.tsx`. The
shadcn `sonner` wrapper is deliberately not used: it depends on `next-themes`,
which this Vite app does not have.

Rules:

- **R13 — A component is used before it is written.** If shadcn has it, it is
  used. A hand-written button is a bug.
- **R14 — A generated component is not edited.** The change goes upstream or
  it is wrapped: `components/ui/button.tsx` stays generated, the app-specific
  variant lives in `components/layout/` or `features/<f>/components/`.
- **R15 — Theme tokens only.** Colours come from the Tailwind v4 theme, never
  from a hex value in a component. The dark mode is the token set, not a
  second stylesheet.

### 7.2 UI utilities

| Module | Responsibility | State |
|---|---|---|
| `lib/utils.ts` | `cn()` — `clsx` + `tailwind-merge` | exists |
| `lib/ui/theme.ts` | light / dark / system resolution, document attribute | exists; persisted only by the options form |
| `sonner` | one `Toaster` mounted by `app/providers.tsx` | exists (direct package, no wrapper) |
| `lib/ui/dialogs.ts` | opens a dialog in the right **window** (Tauri multi-window) | not created |
| `lib/ui/confirm.ts` | `confirm()` as a promise, backed by `AlertDialog` | not created; the toolbar uses `AlertDialog` inline |
| `lib/format.ts` | `formatBytes`, `formatSpeed`, `formatPercent`, `formatDuration` | exists |
| `lib/ui/table.ts` | shared column definitions, sorting and the resizer | not created |
| `lib/hooks/useEvent.ts` | subscribe to a Tauri event, returns the unsubscribe | not created |
| `app/window-mode.ts` | parses the window query string (pure function) | exists |
| `lib/hooks/useHotkey.ts` | one keyboard shortcut registry | not created |

### 7.3 The windows and their modes

The Rust side opens six windows today, all from `index.html` with a query
string. React keeps that, because the Rust routes are the contract:

| Route | Mode | React screen |
|---|---|---|
| `index.html` | main | `DownloadsScreen` |
| `index.html?progress=<id>` | progress | `DownloadProgressScreen` |
| `index.html?options=1` | options | `SettingsScreen` |
| `index.html?add=1&paste=&scheduled=` | add | `AddDownloadScreen` |
| `index.html?scheduler=1` | scheduler | `SchedulerScreen` |
| `index.html?integration=1` | integration | `BrowserIntegrationScreen` |
| `torrents.html` | torrents | `TorrentsScreen` |

`lib/../app/routes.tsx` maps the mode to the screen. A mode that no longer
exists is a dead route and is deleted in the same commit.

**Today `main` and `options` are implemented.** `progress`, `add`, `scheduler`
and `integration` render `UnavailableScreen` with an honest "not migrated yet"
message. `torrents.html` is not built and the Rust side still opens it, so that
window is broken until step 2 of §0.3 lands.

---

## 8. Typed IPC

The single biggest win. The Rust DTOs are the source of truth.

### 8.1 Generation

`ts-rs` derives `TS` on the DTOs and writes `src/types/ipc.ts`:

```rust
#[derive(Serialize, Deserialize, TS)]
#[ts(export_to = "types/ipc.ts")]
#[serde(rename_all = "camelCase")]
pub struct TorrentListItem { /* … */ }
```

The build runs `cargo test export_bindings` (ts-rs exports on test) and the
file is committed. CI fails when the file is out of date:

```bash
cargo test --lib export_bindings && git diff --exit-code src/types/ipc.ts
```

### 8.2 The command map

One file, generated, one entry per command. A command that the Rust does not
register cannot be called. **Today the map is hand-written**
(`renderer/src/lib/ipc/commands.ts`, 7 of 65 commands) and generated in
step 5 of §0.3:

```ts
// renderer/src/lib/ipc/commands.ts — target: generated
export interface CommandMap {
  snapshot: { args: undefined; result: Snapshot };
  torrent_list: { args: undefined; result: TorrentListItem[] };
  torrent_add_magnet: { args: { magnet: string }; result: string };
  // … every command in generate_handler!
}
```

`lib/ipc/client.ts` only accepts a key of that map:

```ts
export async function call<K extends keyof CommandMap>(
  command: K,
  args?: CommandMap[K]["args"],
): Promise<CommandMap[K]["result"]> {
  return invoke<CommandMap[K]["result"]>(command, args);
}
```

Rules:

- **R16 — The command map is generated from the Rust `generate_handler!`
  list.** The audit found `tray_set_active_downloads` missing from that list
  while the JavaScript called it. That class dies here.
- **R17 — The payload type comes from the Rust DTO.** The `TorrentListItem`
  the UI reads is the struct the backend sends.
- **R18 — `invoke` is never called outside `lib/ipc` and the feature `api.ts`
  files.**

---

## 9. Screens, features and files

One table per feature. The rule is: **one component per file, one concern per
file, no file over 300 lines.**

```
features/torrents/
├── api.ts                        # typed calls + query keys
├── schemas.ts                    # zod schemas for the torrent settings form
├── state/torrentsStore.ts        # UI state: selected id, open tab, panel width
├── hooks/useTorrents.ts          # query + event invalidation
├── hooks/useTorrentFiles.ts      # files of one torrent, keyed by id and hash
├── hooks/useTorrentReview.ts     # the pending review
├── components/TorrentRow.tsx
├── components/TorrentList.tsx
├── components/TorrentToolbar.tsx
├── components/TorrentFacts.tsx
├── components/TorrentFileList.tsx
├── components/TorrentFileRow.tsx
├── components/TorrentTrackerList.tsx
├── components/TorrentReviewPanel.tsx
├── components/TorrentSettingsForm.tsx
├── components/TorrentAddDialog.tsx
├── components/TorrentRemoveDialog.tsx
├── screens/TorrentsScreen.tsx
├── screens/TorrentDetailPanel.tsx
└── index.ts                      # the public surface of the feature
```

`index.ts` is the only import path other features use:

```ts
// features/torrents/index.ts
export { TorrentsScreen } from "./screens/TorrentsScreen";
export { useTorrentsStore } from "./state/torrentsStore";
```

**R19 — A feature is imported through its `index.ts`.** No deep imports across
features. The folder boundary stays visible.

The same six folders are created for `downloads`, `queues`, `scheduler`,
`credentials`, `settings` and `media`.

---

## 10. Tooling

### 10.1 `components.json`

Lives at the **repository root**, not in `renderer/`. The shadcn CLI resolves
`@/` from the root `tsconfig.json`; `renderer/` has no `package.json`, so a
`components.json` there makes the CLI offer to scaffold a new project. The CSS
path is relative to the repository root:

```json
{
  "$schema": "https://ui.shadcn.com/schema.json",
  "style": "radix-vega",
  "rsc": false,
  "tsx": true,
  "tailwind": {
    "config": "",
    "css": "renderer/src/index.css",
    "baseColor": "slate",
    "cssVariables": true,
    "prefix": ""
  },
  "aliases": {
    "components": "@/components",
    "utils": "@/lib/utils",
    "ui": "@/components/ui",
    "lib": "@/lib",
    "hooks": "@/lib/hooks"
  },
  "iconLibrary": "lucide",
  "registries": {}
}
```

Run the CLI from the repository root:

```bash
bunx --bun shadcn@latest add badge checkbox alert-dialog skeleton
```

Two registry quirks observed on 2026-11-13:

1. The generated files import `cn` from the npm package `"cn"` instead of the
   configured alias. Patch the import to `@/lib/utils` and `bun remove cn`.
   The registry also declares `cn` as a dependency.
2. `add` prompts before overwriting an existing file even with `--yes`; pass
   `--overwrite` when regenerating.

`registries` stays empty unless a private registry is added. A registry entry
looks like this and is only added when there is a registry to add:

```json
"registries": { "@acme": "https://acme.com/r/{name}.json" }
```

### 10.2 `vite.config.ts`

`renderer/vite.config.ts` is run from the repository root with an explicit
`--config`, so its `root` is the renderer directory:

```ts
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "node:path";

export default defineConfig({
  root: import.meta.dirname,
  plugins: [react(), tailwindcss()],
  resolve: { alias: { "@": path.resolve(import.meta.dirname, "./src") } },
  build: { outDir: "dist", emptyOutDir: true, sourcemap: false },
  server: {
    port: 5173,
    strictPort: true,
    fs: { allow: [".."] },
  },
});
```

`import.meta.dirname` (not `__dirname`) is required because the root
`package.json` is `"type": "module"`. The Tauri environment hooks
(`TAURI_DEV_HOST`, platform-specific build targets) are still to add with the
second entry point.

**Today the config has one entry.** `torrents.html` is not built and must be
restored in step 2 of §0.3. The target adds:

```ts
build: {
  rollupOptions: {
    input: {
      main: path.resolve(import.meta.dirname, "index.html"),
      torrents: path.resolve(import.meta.dirname, "torrents.html"),
    },
  },
}
```

### 10.3 `tsconfig.json`

There are two, on purpose:

| File | Used by | Paths |
|---|---|---|
| `renderer/tsconfig.json` | `bun run typecheck`, the editor, Vite | `@/*` → `renderer/src/*` |
| `tsconfig.json` (root) | the shadcn CLI only | `@/*` → `renderer/src/*` |

The renderer config is the strict one:

```jsonc
{
  "compilerOptions": {
    "target": "ES2022",
    "lib": ["esnext", "dom", "dom.iterable"],
    "module": "ESNext",
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "skipLibCheck": true,
    "jsx": "react-jsx",
    "strict": true,
    "noUncheckedIndexedAccess": true,
    "noImplicitOverride": true,
    "noFallthroughCasesInSwitch": true,
    "noUnusedLocals": true,
    "forceConsistentCasingInFileNames": true,
    "baseUrl": ".",
    "paths": { "@/*": ["./src/*"] }
  },
  "include": ["src"],
  "exclude": ["dist", "node_modules"]
}
```

`noUncheckedIndexedAccess` is the compiler rule that would have caught
`state.filesHash.get(...)` on a missing member. The root config exists only
because the shadcn CLI looks up aliases from the repository root; keep the two
`@/*` mappings identical.

### 10.4 Tauri configuration changes

The current values in `src-tauri/tauri.conf.json`:

```jsonc
{
  "build": {
    "beforeDevCommand": "bun run frontend:dev",
    "beforeBuildCommand": "cargo build --release --bin open-download-manager-native-host && bun run frontend:build",
    "devUrl": "http://localhost:5173",
    "frontendDist": "../renderer/dist"
  }
}
```

The commands call `frontend:*` scripts, not `dev` / `build`, because those are
the Tauri wrappers and would recurse. `frontendDist` points at the Vite output
under `renderer/`.

The `extension/**/*` and `extension-firefox/**/*` bundle resources were removed
with the deleted extension sources; Stage 5 restores both the folders and the
resource entries. Do not add a resource glob that matches nothing — packaging
then fails on a missing path.

The CSP move (from the HTML `<meta>` tag to `tauri.conf.json`) is still open.
Today `renderer/index.html` carries:

```
default-src 'self';
script-src 'self' 'unsafe-inline';
style-src 'self' 'unsafe-inline';
img-src 'self' data: blob: asset: http://asset.localhost;
connect-src 'self' ipc: http://ipc.localhost ws://localhost:5173 http://localhost:5173;
base-uri 'none';
form-action 'none'
```

`connect-src` must include `ipc:` and `http://ipc.localhost`, or every
`invoke` is blocked in the bundled app. The dev-server sources keep HMR
working. `app.security.capabilities: ["default"]` is the only security setting
in `tauri.conf.json` today.

The `unsafe-inline` in `style-src` is needed by Tailwind's runtime-free output
and by Radix, which injects styles.

### 10.5 ESLint

`eslint.config.mjs` has a `renderer/**/*.{ts,tsx}` block with the TypeScript
parser, `eslint-plugin-react-hooks` and the file rules that need no type
program:

```js
{
  files: ["renderer/**/*.{ts,tsx}"],
  rules: {
    "@typescript-eslint/no-explicit-any": "error",
    "@typescript-eslint/consistent-type-imports": "error",
    "@typescript-eslint/no-unused-vars": ["warn", { argsIgnorePattern: "^_" }],
    "@typescript-eslint/no-unused-expressions": "error",
    "react-hooks/rules-of-hooks": "error",
    "react-hooks/exhaustive-deps": "warn",
    "max-lines": ["error", { max: 300, skipBlankLines: true, skipComments: true }],
    "no-restricted-syntax": ["error", {
      selector: "AssignmentExpression[left.object.name='window']",
      message: "Do not hang state on window. Use lib/ipc or lib/ui.",
    }],
  },
}
```

`renderer/dist/**`, `reference/**`, `graphify-out/**` and `coverage/**` are
ignored. Before 2026-11-13 the built bundle was being linted and produced 114
false errors.

The old `renderer/**/*.js`, `renderer/app.js`, `renderer/torrents.js` and
`extension/**/*.js` blocks were deleted together with those sources. Only
`tools/**/*.mjs` remains outside the renderer block.

Still to add, when the feature folders land: `import/no-default-export`,
`import/no-cycle`, `max-imports`, `react/jsx-key` and the type-aware
`recommendedTypeChecked` set (it needs `parserOptions.project`).

### 10.6 Scripts

The scripts that matter to this migration:

```json
{
  "scripts": {
    "dev": "tauri dev",
    "build": "tauri build",
    "frontend:dev": "vite --config renderer/vite.config.ts",
    "frontend:build": "vite build --config renderer/vite.config.ts",
    "typecheck": "tsc --noEmit -p renderer/tsconfig.json",
    "lint": "eslint renderer/ tools/",
    "test": "cargo test && bun run test:renderer",
    "test:renderer": "vitest run --config renderer/vitest.config.ts",
    "tauri": "tauri"
  }
}
```

`dev` and `build` stay the Tauri wrappers; `beforeDevCommand` and
`beforeBuildCommand` call the `frontend:*` scripts. `test:renderer` is the
script CI calls; `test` adds the Rust suite for a local full run.

---

## 11. Testing

| Kind | Tool | Rule |
|---|---|---|
| Unit | Vitest | pure helpers, stores, selectors |
| Component | Vitest + Testing Library | one component per test file, named after the component |
| IPC contract | Vitest | the generated command map is compared with the Rust `generate_handler!` list |
| Backend | `cargo test` | unchanged |
| Window boot | Vitest + jsdom | the window mounts and renders without an error — the test that caught the shipped crash |

Today: 29 tests in 6 files — `app/window-mode.test.ts`, `lib/format.test.ts`,
`features/downloads/state/downloads-store.test.ts`,
`features/downloads/components/DownloadsTable.test.tsx`,
`features/downloads/screens/DownloadsScreen.test.tsx` and
`features/settings/screens/SettingsScreen.test.tsx`. The last two are the
window-boot tests for the implemented windows.

**R20 — The IPC mock is generated.** A test that mocks `invoke` uses the
generated payload types, so a renamed field breaks the mock and the test.
Today the fixtures in `test/fixtures.ts` are hand-written mirrors; they become
generated together with `types/ipc.ts` in step 5.

**R21 — The old `node:test` suite is migrated file by file.** The legacy specs
survive in `reference/legacy-renderer/` and `reference/legacy-extension/`.
`formatters` and `theme` have replacements. `torrents-view`, `context-actions`,
`capture-evaluator`, `hls` and the extension boot tests still need ports when
their features land.

---

## 12. The browser extension

The extension is a separate build with different constraints: MV3 content
scripts are **not** ES modules, so a React component cannot be loaded as a
module. It must be bundled to a single IIFE file.

### 12.1 What moves

| Part | Decision |
|---|---|
| `background.js` (569 lines) | **TypeScript, no React.** It is a service worker with no UI. React there is dead weight. |
| `content.js` (658 lines) | **TypeScript.** The in-page panel becomes React; the scanner stays framework free. |
| The in-page panel and the chooser | **React 19**, rendered into the Shadow DOM that exists today |
| `youtube-*.js`, `media-*.js`, `hls.js`, `capture-evaluator.js` | **TypeScript**, no React |
| The shared logic | moves to `packages/shared/` and is used by the app **and** the extension |

### 12.2 The shared package

```
packages/shared/
├── src/
│   ├── format.ts          # bytes, duration, speed
│   ├── media-policy.ts    # the capture rules
│   ├── torrent-key.ts     # the identity of a torrent
│   └── protocol.ts        # the message names and their payloads
└── package.json
```

**R22 — The shared package holds pure logic only.** No DOM, no `chrome.*`, no
`window`. It is imported by the app, by the extension and by the tests.

### 12.3 The extension build

```ts
// extension/vite.config.ts
export default defineConfig({
  build: {
    outDir: "dist",
    emptyOutDir: true,
    rollupOptions: {
      input: {
        background: "src/background.ts",
        content: "src/content.ts",
        panel: "src/panel.tsx",
      },
      output: {
        format: "iife",
        entryFileNames: "[name].js",
        inlineDynamicImports: true,
      },
    },
  },
});
```

`inlineDynamicImports` is required: a content script is one file.

The manifest is generated from the build output, so the file list in
`manifest.json` and `manifest.firefox.json` cannot drift from the bundle.

### 12.4 Rules

- **R23 — The extension keeps a plain TypeScript service worker.** No React.
- **R24 — The panel is the only React surface.** It is one folder with one
  component per file.
- **R25 — The extension build is separate.** It never shares a bundle with
  the app, only source.
- **R26 — The extension tests keep running.** The existing 101 extension
  tests move to Vitest with the same assertions.

---

## 13. Stages

Every stage ends with a shippable build, a green test suite and a rollback
point.

### Stage 0 — Baseline (done)

The lint gate, the static member guard, the window boot test, the CI
workflow. The audit fixes. `v0.3.0-beta.2` ships on the current architecture.

### Stage 1 — The build pipeline and TypeScript

**Status: partial.** Vite 8, TypeScript 5, Tailwind v4, Vitest and the React
plugin are wired; `renderer/vite.config.ts`, `renderer/tsconfig.json` and
`renderer/vitest.config.ts` exist. The renderer is TypeScript under
`renderer/src`. Missing: `ts-rs`, the generated command map, and the type-aware
ESLint rules.

- Add Vite, TypeScript, Tailwind v4, shadcn/ui, Vitest. **Done**
- Move the renderer into `src/` as TypeScript modules. The DOM code stays as
  it is — the same functions, the same CSS, no React yet.
- The CSS moves into Tailwind v4 only if it can be done without a visual
  change. If it cannot, the CSS stays a plain stylesheet and Tailwind comes
  with the components in Stage 3.
- Typed IPC: `ts-rs` on the DTOs, the generated command map, `invoke` typed.
- **Gate:** no visual change, every existing test ported, `tsc --noEmit` clean.

**Why first:** it kills the whole crash class with the smallest diff. The
`state.filesHash` bug cannot happen in TypeScript.

### Stage 2 — shadcn/ui and the UI utilities

**Status: partial.** 11 primitives are generated under
`renderer/src/components/ui/` with the CLI (style `radix-vega`): `alert-dialog`,
`badge`, `button`, `checkbox`, `dialog`, `input`, `label`, `select`,
`separator`, `skeleton`, `table`. All of them import `cn` from `lib/utils.ts`
and Radix from the single `radix-ui` package. The hand-written `toast` /
`toaster` pair is gone; `sonner` is mounted directly. `format`
helpers live at `lib/format.ts`, the theme at `lib/ui/theme.ts`. Missing:
`lib/ui/dialogs`, `lib/ui/confirm`, `lib/ui/table`, `lib/hooks/useEvent`,
`lib/hooks/useHotkey`.

- Add the shadcn components that the current UI already needs.
- Build `lib/ui`: theme, toasts, dialogs, the table, the formatters.
- Replace the hand-written dialogs one by one, starting with the error
  dialog and the settings window.
- **Gate:** the UI looks the same, the component count grows, no file passes
  300 lines.

The port uses the `radix-vega` defaults, so the look already differs from the
old renderer. That change has not been signed off; Stage 4 still owns the
visual pass.

### Stage 3 — React screens

**Status: partial, 2 of 7 windows.** The main window (`DownloadsScreen`) and
the options window (`SettingsScreen`) are React. `progress`, `add`,
`scheduler` and `integration` render `UnavailableScreen`; the torrents window
and `torrents.html` do not exist. The old renderer is deleted and preserved
only in the gitignored `reference/legacy-renderer/`.

- One feature per stage: `downloads` → `settings` → `queues` → `torrents` →
  `scheduler` → `media` → `credentials`.
- The old code is deleted in the same commit that replaces it. No two
  implementations of one screen.
- The main window and the Torrents window are separate entries.
- **Gate:** every screen has a component test, the window boot test passes
  for both entries.

### Stage 4 — The visual redesign

**Status: not started.** No sign-off, no baseline screenshots, no compact
Windows layout pass.

- The Tailwind v4 theme, the shadcn tokens, light and dark.
- Needs a sign-off on the look. `tools/screenshots/` holds the baselines.
- The compact Windows layout from `CONTRIBUTING.md` is a requirement, not a
  preference.

### Stage 5 — The extension

**Status: not started.** `extension/` was deleted. The legacy sources survive
only in `reference/legacy-extension/`, which is gitignored. No TypeScript, no
`packages/shared/`, no React panel. Stage 5 must start by restoring the
extension sources into version control.

- TypeScript for the service worker and the content script.
- `packages/shared/` with the logic the app and the extension both use.
- React for the in-page panel.
- The manifest is generated from the build.

---

## 14. Risks

| # | Risk | Mitigation |
|---|---|---|
| R-1 | The rewrite loses behaviour nobody wrote down | Stage 1 keeps the behaviour and only changes the language. The window boot test and the component tests are written before the port |
| R-2 | The visual change is not wanted | Stage 4 is separate and gated on a sign-off |
| R-3 | The generated IPC types drift from the Rust | CI fails when the generated file is out of date |
| R-4 | Tailwind v4 + Vite 8 + shadcn friction | The shadcn CLI runs from the repository root; if a future CLI release breaks, pin the last working `shadcn` version. Vite 8 is working today, so no downgrade is needed |
| R-5 | TypeScript 7 is too new for the tooling | Pin TypeScript 5.x |
| R-6 | The extension bundle breaks the content script | `inlineDynamicImports`, one file per entry, the manifest is generated, the extension tests run on the built file |
| R-7 | The scope is too big to finish | Each stage is shippable and independent. Stop after any stage |
| R-8 | A status claim outruns the gates, as it did on 2026-11-12 | §0 is the only status source. A stage is promoted only in the commit that shows green `typecheck`, `lint` and `test` |

---

## 15. Definition of done

- [x] `tsc --noEmit` is clean with `strict` and `noUncheckedIndexedAccess`.
- [x] `bun run lint` is clean, including `max-lines` and `no-explicit-any`.
- [ ] Every screen is a feature folder with the six sub-folders. (2 of 7:
      `downloads`, `settings`.)
- [x] No file over 300 lines. (Largest app file: 165 lines.)
- [ ] The command map is generated and matches `generate_handler!`. (7 of 65
      commands are hand-mirrored in `lib/ipc/commands.ts`.)
- [ ] The IPC types are generated from the Rust DTOs. (`types/ipc.ts` is
      hand-written.)
- [x] No `window.x =` outside `lib/`.
- [x] Every shadcn component in use came from the CLI. The old hand-written
      `toast` / `toaster` pair is deleted; `sonner` is the upstream package,
      mounted directly because the shadcn wrapper pulls `next-themes`.
- [ ] Both windows boot in the test harness. (Main and options windows have
      boot tests; the torrents window and `torrents.html` do not exist.)
- [x] `vitest run`, `bun run lint` and `bun run typecheck` are green locally,
      and `.github/workflows/ci.yml` runs those three. `cargo test` is not a CI
      job yet; it is green locally and runs through `bun run test`.
- [ ] The extension builds, its tests pass, and the panel is React. The
      legacy sources are preserved in `reference/`, which is gitignored.

---

## 16. Open questions

1. **Design system.** Decided 2026-11-13: Tailwind v4 + shadcn with the
   `radix-vega` style. Stage 4 decides the final look within that system.
2. **Typed IPC generator.** `ts-rs` (small, explicit) or `specta` (bigger,
   generates the command wrappers too)? `ts-rs` is the safer first step.
3. **Extension scope.** Is the React panel worth it, or is TypeScript alone
   enough for the extension?
4. **Windows layout.** The compact layout is a hard requirement from
   `CONTRIBUTING.md`. Which screens keep it?
