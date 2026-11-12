# Runtime Crash Audit — Undefined Members In The Renderer

Status: fixed. Written 2026-11-12.

`v0.3.0-beta.1` shipped with a crash in the Torrents window:

```
undefined is not an object evaluating state.filehash.set
```

JavaScriptCore reports the failing expression, not the missing value. The
expression was `state.filesHash.set(id, torrent.infoHash)` in the file list
cache. The map itself was never created, so every read of the file list of a
torrent threw. The release was withdrawn.

The same class of bug — a script using a name that does not exist — was
present in five more places. This document records the audit, the fixes and
the guard that keeps the class from coming back.

---

## 1. The torrent file list cache

### What happened

The Torrents window opened, the torrent list appeared, and the first refresh
of the file list threw. The window showed no files at all.

### Root cause

`renderer/torrents.js` keeps three maps per torrent id: the file list, the
loading promise and the time the list was fetched. A fourth map was needed:
the info hash each cached file list belongs to, because the engine hands the
same id to the next torrent after a deletion.

The map was used in three places and declared in none:

```js
if (state.filesHash.get(torrent.id) !== torrent.infoHash) {   // read
state.filesHash.delete(id);                                     // write
state.filesHash.set(id, torrent.infoHash);                     // write
```

### Fix

`filesHash: new Map()` is declared next to the other cache maps, and the
static test below fails if a member of an object literal is read but never
declared.

---

## 2. The selected download was read from the wrong place

`renderer/app.js` keeps the selected download in a module level variable
called `selectedId`. Four places read `state.selectedId` instead, which is
always `undefined`:

- the media tool dialog opened without a download,
- the error dialog could not be opened at all, because it looked for a
  download with the id `undefined` and gave up,
- the "install ffmpeg" button opened the dialog for nothing,
- the download that should be resumed after the tool was installed was lost.

### Fix

All four now read `selectedId`.

---

## 3. `hideDialog` was never written

The download error dialog has six buttons that close the dialog before they
act: "Retry", "Install ffmpeg & Assemble", "Install yt-dlp & Resume",
"Resume Transfer" and "Check Proxy Settings". Every one of them called
`hideDialog("#error-dialog")`, and no such function existed anywhere in the
renderer.

The dialog therefore never closed and the action never ran. The user clicked
and nothing happened.

### Fix

`hideDialog` sits next to `showDialog` now, and both ignore a dialog that is
not on the page.

---

## 4. The category folders were written to an undeclared variable

`renderer/app.js` is a strict mode script. The options dialog assigned the
per category download folders to `optionsCategoryDirectories`, and the
variable was never declared:

```js
optionsCategoryDirectories = { ...(settings.categoryDirectories || {}) };
```

In strict mode that throws `ReferenceError: optionsCategoryDirectories is not
defined`, so opening the options window failed on the first line of work.

### Fix

The map is declared next to `optionsConnectionOverrides`, which is the same
kind of editor state.

---

## 5. The clipboard handler was declared in the wrong scope

The handler for a detected clipboard link was declared inside the callback of
`start().then(async () => { ... })`, but it is called from an event listener
that is registered while `start()` is still running. The handler was not in
scope at the call site, so every clipboard candidate threw
`handleClipboardCandidate is not defined`.

### Fix

The handler, the prompt buttons and the current candidate live at module
scope now.

---

## The guard

Three things keep this class of bug out:

1. **`bun run lint`** — ESLint with `no-undef` over the renderer and the
   extension. It catches a name that is used but never declared
   (`hideDialog`, `optionsCategoryDirectories`,
   `handleClipboardCandidate`).
2. **`renderer/state-members.test.cjs`** — a static test that reads every
   object literal in the renderer, collects the keys it declares and fails
   when a script reads a member that is not among them
   (`state.filesHash`, `state.selectedId`). ESLint cannot see this, because
   a member access is not an undefined identifier.
3. **`.github/workflows/ci.yml`** — the lint step and the test suite run on
   every push and pull request, so a release cannot be built from a tree that
   fails either.
