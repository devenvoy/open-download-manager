# React + shadcn/ui Frontend Migration — Plan

**Status:** Draft / not started
**Created:** 2026-11-12
**Scope:** Replace the hand-written renderer with a React 19 + TypeScript + Vite
+ Tailwind CSS v4 + shadcn/ui frontend, split into modular features, with
typed IPC, disciplined state management and a reusable UI layer. The Rust
backend is not rewritten; the browser extension is migrated to TypeScript and
gets a React panel only.
**Constraint:** Every stage ships. The app must stay buildable, shippable and
free of the crash classes found in the 2026-11-12 audit.

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
src/                                  # Vite + React + TS application root
├── main.tsx                          # entry for index.html
├── torrents.tsx                      # entry for torrents.html
├── vite-env.d.ts
│
├── app/                              # app shell, wiring, nothing else
│   ├── App.tsx                       # router + providers
│   ├── providers.tsx                 # theme, query client, toaster, error boundary
│   ├── routes.tsx                    # the six window modes as routes
│   └── windows.ts                    # the route table for every window
│
├── components/                       # shared, feature-agnostic components
│   ├── ui/                           # shadcn/ui — generated, never hand-edited
│   ├── layout/                       # AppShell, TitleBar, Sidebar, DialogHost
│   ├── feedback/                     # EmptyState, ErrorState, ProgressBar
│   └── table/                        # DataTable, TableToolbar, ColumnResizer
│
├── features/                         # one vertical slice per domain
│   ├── downloads/
│   │   ├── api.ts                    # typed IPC calls for this feature
│   │   ├── hooks/                    # useDownloads, useDownloadEvents
│   │   ├── components/               # DownloadRow, DownloadList, DownloadFilters
│   │   ├── screens/                  # DownloadsScreen, DownloadDetailsPanel
│   │   ├── state/                    # downloadsStore (zustand, UI state only)
│   │   └── schemas.ts                # zod schemas for the forms
│   ├── torrents/                     # same six folders
│   ├── queues/
│   ├── scheduler/
│   ├── credentials/
│   ├── settings/
│   └── media/                        # playlist and format pickers
│
├── lib/                              # utilities, no business logic
│   ├── ipc/                          # invoke wrapper, event helpers, command map
│   ├── ui/                           # cn, theme tokens, dialog controller, toast
│   ├── hooks/                        # useTheme, useEvent, useWindowMode, useHotkey
│   ├── format/                       # bytes, duration, speed, eta, percent
│   └── validation/                   # shared zod helpers
│
├── stores/                           # one zustand store per cross-cutting concern
│   ├── windowStore.ts                # which window, which mode
│   └── preferencesStore.ts           # user preferences that survive a reload
│
├── types/                            # generated from Rust, never edited by hand
│   ├── ipc.ts                        # ts-rs output
│   └── commands.ts                   # the command map, generated
│
└── styles/
    └── globals.css                   # tailwind v4 theme tokens
```

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

| Rule | Enforced by |
|---|---|
| One component per file, file name equals the component name | ESLint `react/filename-naming` |
| No file over 300 lines | ESLint `max-lines` |
| No file over 12 imports | ESLint `max-imports` |
| No `any` | ESLint `@typescript-eslint/no-explicit-any` (error) |
| No `export default` | ESLint `import/no-default-export` |
| No direct `window.x =` outside `lib/ipc` and `lib/ui` | ESLint `no-restricted-syntax` |
| No `useEffect` for derived data | ESLint `react-hooks/exhaustive-deps` + review rule R7 |
| Imports are ordered and aliased `@/` → `src/` | ESLint `import/order` |

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
| `window.initTheme` / `window.applyTheme` | `lib/ui/theme.tsx` — a provider, no globals |

---

## 7. UI layer

### 7.1 shadcn/ui

`components.json` (see §10.1) points the CLI at the standard registry and at
the `src/` folder. The CLI is the only way components are added:

```bash
bunx --bun shadcn@latest add button dialog dropdown-menu table tabs progress
bunx --bun shadcn@latest add checkbox select slider tooltip sonner
```

Components that are used and must exist:

| shadcn component | Used for |
|---|---|
| `button`, `input`, `label`, `textarea`, `select`, `checkbox`, `switch` | forms and toolbars |
| `dialog`, `sheet`, `popover`, `dropdown-menu`, `tooltip` | every overlay in the app |
| `tabs` | the general / files / trackers tabs, the settings sections |
| `table` | the download list and the torrent list |
| `progress` | per download and per torrent progress |
| `badge` | states and qualities |
| `separator`, `scroll-area`, `skeleton`, `sonner` | layout and feedback |
| `command` | the search and the command palette |
| `alert-dialog` | every destructive confirmation |

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

| Module | Responsibility |
|---|---|
| `lib/ui/cn.ts` | `cn()` — `clsx` + `tailwind-merge` |
| `lib/ui/theme.tsx` | light / dark / system, persisted through the Rust settings, one provider |
| `lib/ui/toast.tsx` | one `Toaster` mounted by the app shell, one API for every message |
| `lib/ui/dialogs.tsx` | opens a dialog in the right **window** (Tauri multi-window), not in the current one |
| `lib/ui/confirm.tsx` | `confirm()` as a promise, backed by `AlertDialog` |
| `lib/ui/format.ts` | `formatBytes`, `formatSpeed`, `formatEta`, `formatRatio`, `formatPercent` |
| `lib/ui/table.ts` | the shared column definitions, sorting and the resizer |
| `lib/hooks/useEvent.ts` | subscribe to a Tauri event, returns the unsubscribe, cleans up |
| `lib/hooks/useWindowMode.ts` | reads the query string of the window and exposes the mode |
| `lib/hooks/useHotkey.ts` | one keyboard shortcut registry, no `document.addEventListener` in a component |

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
register cannot be called:

```ts
// src/types/commands.ts — generated
export const commands = {
  snapshot: "snapshot",
  torrent_list: "torrent_list",
  torrent_add_magnet: "torrent_add_magnet",
  // … every command in generate_handler!
} as const;
```

`lib/ipc/invoke.ts` only accepts a key of that map:

```ts
export async function invoke<K extends keyof typeof commands>(
  command: K,
  args?: ArgsOf<K>,
): Promise<ResultOf<K>> {
  return tauriInvoke(commands[command], args);
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

Created in Stage 1 at the repository root:

```json
{
  "$schema": "https://ui.shadcn.com/schema.json",
  "style": "new-york",
  "rsc": false,
  "tsx": true,
  "tailwind": {
    "config": "",
    "css": "src/styles/globals.css",
    "baseColor": "slate",
    "cssVariables": true,
    "prefix": ""
  },
  "aliases": {
    "components": "@/components",
    "utils": "@/lib/ui/cn",
    "ui": "@/components/ui",
    "lib": "@/lib",
    "hooks": "@/lib/hooks"
  },
  "iconLibrary": "lucide",
  "registries": {}
}
```

`registries` is empty unless a private registry is added. A registry entry
looks like this and is only added when there is a registry to add:

```json
"registries": { "@acme": "https://acme.com/r/{name}.json" }
```

### 10.2 `vite.config.ts`

```ts
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "node:path";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: { alias: { "@": path.resolve(__dirname, "./src") } },
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 5174 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  build: {
    target: process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome105" : "safari13",
    minify: !process.env.TAURI_ENV_DEBUG,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
    rollupOptions: {
      input: {
        main: path.resolve(__dirname, "index.html"),
        torrents: path.resolve(__dirname, "torrents.html"),
      },
    },
  },
});
```

Two entry points, because Tauri opens `index.html` and `torrents.html` as
separate windows.

### 10.3 `tsconfig.json`

```jsonc
{
  "compilerOptions": {
    "target": "ES2022",
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "moduleResolution": "bundler",
    "jsx": "react-jsx",
    "strict": true,
    "noUncheckedIndexedAccess": true,   // the state.filesHash rule
    "noImplicitOverride": true,
    "noFallthroughCasesInSwitch": true,
    "verbatimModuleSyntax": true,
    "noEmit": true,
    "skipLibCheck": true,
    "types": ["vite/client"],
    "baseUrl": ".",
    "paths": { "@/*": ["./src/*"] }
  },
  "include": ["src", "vite.config.ts"]
}
```

`noUncheckedIndexedAccess` is the compiler rule that would have caught
`state.filesHash.get(...)` on a missing member.

### 10.4 Tauri configuration changes

```jsonc
{
  "build": {
    "beforeDevCommand": "bun run dev",
    "beforeBuildCommand": "bun run build && cargo build --release --bin open-download-manager-native-host",
    "devUrl": "http://localhost:5173",
    "frontendDist": "../dist"
  },
  "app": {
    "security": {
      "csp": {
        "default-src": "'self'",
        "connect-src": "ipc: http://ipc.localhost",
        "img-src": "'self' data: blob: asset: http://asset.localhost",
        "style-src": "'self' 'unsafe-inline'",
        "font-src": "'self' data:"
      },
      "devCsp": {
        "default-src": "'self'",
        "connect-src": "ipc: http://ipc.localhost ws://localhost:5173 http://localhost:5173",
        "style-src": "'self' 'unsafe-inline'",
        "script-src": "'self' 'unsafe-eval' 'unsafe-inline'"
      }
    }
  }
}
```

The CSP moves out of the HTML `<meta>` tag into `tauri.conf.json`, because
Tauri then hashes the bundled scripts and adds the IPC sources. The
`unsafe-inline` in `style-src` is needed by Tailwind's runtime-free output
and by Radix, which injects styles.

### 10.5 ESLint

The current config stays and grows:

```js
{
  files: ["src/**/*.{ts,tsx}"],
  rules: {
    ...tseslint.configs.recommendedTypeChecked.rules,
    ...reactPlugin.configs["recommended-type-checked"].rules,
    "max-lines": ["error", { max: 300, skipBlankLines: true, skipComments: true }],
    "max-imports": ["error", { max: 12 }],
    "import/no-default-export": "error",
    "@typescript-eslint/no-explicit-any": "error",
    "@typescript-eslint/consistent-type-imports": "error",
    "react/filename-naming": ["error", { regex: "^[A-Z][a-zA-Z0-9]*\\.tsx$" }],
    "react/no-array-index-key": "error",
    "no-restricted-syntax": ["error", {
      selector: "MemberExpression[object.object.name='window']",
      message: "Do not hang state on window. Use lib/ipc or lib/ui.",
    }],
  },
}
```

### 10.6 Scripts

```json
{
  "scripts": {
    "dev": "vite",
    "build": "tsc --noEmit && vite build",
    "preview": "vite preview",
    "lint": "eslint src/ renderer/ extension/ tools/",
    "typecheck": "tsc --noEmit",
    "test": "cargo test && bun run test:js",
    "test:js": "vitest run",
    "tauri": "tauri"
  }
}
```

---

## 11. Testing

| Kind | Tool | Rule |
|---|---|---|
| Unit | Vitest | pure helpers, stores, selectors |
| Component | Vitest + Testing Library | one component per test file, named after the component |
| IPC contract | Vitest | the generated command map is compared with the Rust `generate_handler!` list |
| Backend | `cargo test` | unchanged |
| Window boot | Vitest + jsdom | the window mounts and renders without an error — the test that caught the shipped crash |

**R20 — The IPC mock is generated.** A test that mocks `invoke` uses the
generated payload types, so a renamed field breaks the mock and the test.

**R21 — The old `node:test` suite is migrated file by file.** The pure
helpers in `torrents-view.js` and `formatters.js` become Vitest specs first;
they carry over almost unchanged.

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

- Add Vite, TypeScript, Tailwind v4, shadcn/ui, Vitest.
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

- Add the shadcn components that the current UI already needs.
- Build `lib/ui`: theme, toasts, dialogs, the table, the formatters.
- Replace the hand-written dialogs one by one, starting with the error
  dialog and the settings window.
- **Gate:** the UI looks the same, the component count grows, no file passes
  300 lines.

### Stage 3 — React screens

- One feature per stage: `downloads` → `settings` → `queues` → `torrents` →
  `scheduler` → `media` → `credentials`.
- The old code is deleted in the same commit that replaces it. No two
  implementations of one screen.
- The main window and the Torrents window are separate entries.
- **Gate:** every screen has a component test, the window boot test passes
  for both entries.

### Stage 4 — The visual redesign

- The Tailwind v4 theme, the shadcn tokens, light and dark.
- Needs a sign-off on the look. `tools/screenshots/` holds the baselines.
- The compact Windows layout from `CONTRIBUTING.md` is a requirement, not a
  preference.

### Stage 5 — The extension

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
| R-4 | Tailwind v4 + Vite 8 + shadcn friction | The shadcn CLI path is the supported one; if it fights, pin Vite 7 for the build |
| R-5 | TypeScript 7 is too new for the tooling | Pin TypeScript 5.x |
| R-6 | The extension bundle breaks the content script | `inlineDynamicImports`, one file per entry, the manifest is generated, the extension tests run on the built file |
| R-7 | The scope is too big to finish | Each stage is shippable and independent. Stop after any stage |

---

## 15. Definition of done

- [ ] `tsc --noEmit` is clean with `strict` and `noUncheckedIndexedAccess`.
- [ ] `bun run lint` is clean, including `max-lines` and `no-explicit-any`.
- [ ] Every screen is a feature folder with the six sub-folders.
- [ ] No file over 300 lines.
- [ ] The command map is generated and matches `generate_handler!`.
- [ ] The IPC types are generated from the Rust DTOs.
- [ ] No `window.x =` outside `lib/`.
- [ ] Every shadcn component in use came from the CLI.
- [ ] Both windows boot in the test harness.
- [ ] `cargo test`, `vitest run` and `bun run lint` are green in CI.
- [ ] The extension builds, its tests pass, and the panel is React.

---

## 16. Open questions

1. **Design system.** Tailwind v4 + shadcn (new look, own the code) or keep
   the current CSS and only use React? Stage 4 decides the look.
2. **Typed IPC generator.** `ts-rs` (small, explicit) or `specta` (bigger,
   generates the command wrappers too)? `ts-rs` is the safer first step.
3. **Extension scope.** Is the React panel worth it, or is TypeScript alone
   enough for the extension?
4. **Windows layout.** The compact layout is a hard requirement from
   `CONTRIBUTING.md`. Which screens keep it?
