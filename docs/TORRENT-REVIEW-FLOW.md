# Torrent Review Flow — Problems, Root Causes, Fixes

Status: fixed, verified by tests. Written 2026-11-12.

This document records the investigation of the symptoms that showed up while
the Torrents window was being tested:

1. The "Start download" panel came back after the user had confirmed it.
2. The torrent was reported as running, but not a single byte arrived.
3. The confirmation failed with the engine wording `torrent is already live`.
4. After deleting a torrent, the next one showed the deleted torrent's files.
5. A magnet with 1200+ seeders showed zero seeds and ran slow.

The short version: **the engine was never the problem.** The service path
(`paused add → set files → resume`) transfers the payload, and that is now
covered by tests. The trackers answer with the right numbers. What was broken
sat around that path — in the delivery of the review to the window, in a race
with the engine's setup, in the identity the window used for a torrent, and
in the peer settings the engine ran with.

---

## 1. The review panel came back after confirming

### What happened

The user confirmed the download. The toast said the download started. The
"Start download" panel then reappeared, and the files the user had ticked
were gone. A second click produced `torrent is already live`.

### Root cause

The review reached the window over **two** channels at the same time:

- the `torrent://review` event, and
- the one second poll that asks the backend for a pending review.

The backend stored the pending copy *after* sending the event:

```rust
let _ = app.emit("torrent://review", &review);
torrents.set_pending_review(Some(review));
```

The window received the event, asked for the pending copy, and — depending on
which of the two calls landed first — either got the review or got nothing.
In the second case the backend stored the copy a moment later, the poll found
it, and the same review was handed out a second time.

The second delivery was not harmless. `beginReview()` reset the file
selection:

```js
state.fileSelection = null;
```

so the second delivery threw away the files the user had just ticked, and
`reviewPlan()` then found an empty selection.

### Fix

- The backend stores the pending copy **before** it emits the event, so the
  window always finds it and always clears it.
- `beginReview()` is now idempotent: a review that is already on screen is
  only re-rendered, never reset.
- A review the user already answered is remembered in `state.answered` and
  never shown again.

---

## 2. "Running, but nothing is downloading"

### What happened

After confirming, the engine reported:

```
state=live progress=0/1174243328 peers=4 seeds=0 leechers=4 down=0
```

The window drew that faithfully — a running torrent with no progress. The
peers it found were all leechers, so there was nothing to download from.

### Root cause

Two separate things, and neither of them was the engine:

1. **The test torrent had no seeds.** The magnet used for the test was a
   DHT-only magnet for Ubuntu 21.04, an end-of-life release. Its DHT swarm
   no longer holds a single copy of the payload, so every peer that answered
   was a leecher. The engine was doing the right thing with a dead swarm.

2. **The engine could not be reached at all in some runs.** The engine keeps
   its DHT routing table in its own configuration folder
   (`~/Library/Caches/com.rtbit.dht/dht.json`), which every copy of the
   engine shares. A second instance reads the same file, tries the same UDP
   port and fails with `Address already in use` — and the whole torrent
   engine then fails to start.

### Proof that the engine is fine

`src-tauri/src/application/torrent_review_flow_tests.rs` runs a real seeder on
localhost and drives the exact calls the window makes:

| Test | What it proves |
| --- | --- |
| `a_review_torrent_does_not_download_before_the_confirmation` | Nothing moves before the user confirms. |
| `a_confirmed_review_downloads_the_payload` | The confirmed torrent receives the payload, byte for byte. |
| `a_torrent_added_without_the_review_downloads_the_payload` | The plain path behaves the same, so the review path is not the odd one out. |
| `a_file_selection_waits_for_the_engine_to_finish_the_setup` | The selection survives a torrent that is still initializing. |
| `a_confirmed_review_downloads_from_the_real_swarm` (ignored, needs internet) | The same flow downloads from the public Ubuntu 24.04 swarm. |

### Fixes

- The engine now keeps its DHT state in the app's own data folder
  (`~/.open-download-manager/dht.json`), so a second instance no longer
  steals the port and no longer writes into another application's folder.
- The window now says **"Waiting for peers"** instead of "Downloading" when a
  running torrent has received nothing and cannot see a peer with the data.
  That is the honest description of a torrent whose swarm is empty.

---

## 3. "torrent is already live"

### What happened

Confirming a torrent that was already running failed with the engine's own
wording. Two separate problems sat behind it.

### Root cause

1. **The file selection was applied too early.** A magnet resolves its
   metadata in the background. The engine refuses to change the selection
   while it is still setting a torrent up:

   ```
   can't update initializing torrent
   ```

   The window asked for the selection as soon as the user clicked, which can
   happen before the engine finished the setup.

2. **A resume on a running torrent is an error in the engine.** The engine
   refuses to start a torrent that already runs. The service now checks the
   state first and treats a resume of a running torrent as a no-op.

### Fixes

- `TorrentService::set_files` waits for the engine to finish setting the
  torrent up instead of failing.
- `TorrentService::action(Resume)` stays idempotent.
- The window maps engine wording to a sentence a person can act on
  (`friendlyError`).

---

## 4. What was *not* the problem

- **The renderer is not the bottleneck.** It draws what the backend reports.
  The backend reported 0 bytes because 0 bytes had arrived.
- **The engine is not the bottleneck.** Proven by the tests above, and by the
  tracker probe: the trackers answer with the right numbers and the payload
  arrives.
- The file list was read from the engine on every progress tick, once per
  second, which was wasteful. It is now read at most every two seconds and
  only while the Files tab is open.

---

## 5. Only a few peers, so the download is slow

### What happened

A magnet that a magnet site lists with 1200+ seeders showed **zero** seeds in
the app, and the transfer ran far below the line speed. The same magnet
downloads fast in another client.

### Root cause

The engine does not have a protocol problem. A probe against the same magnet
shows the trackers answering with their real numbers:

```
tracker udp://tracker.opentrackr.org:1337      -> Working seeders=Some(1476)
tracker udp://open.stealth.si:80/announce       -> Working seeders=Some(1228)
tracker udp://tracker.torrent.eu.org:451/announce-> Working seeders=Some(882)
```

and the engine collects over 2000 peer addresses from them. Two things then
throttle it:

1. **The peer limit was 50.** The engine's own default is 200 and
   qBittorrent's is 200 as well. Only the connected peers can send pieces,
   so a torrent that is connected to ten peers runs at the speed of ten
   peers. The peer statistics show the whole picture:

   ```
   seen=2012 queued=1776 connecting=182 live=18
   ```

   The trackers hand out more peers than the limit allows, so the rest wait
   in the queue while the slots are taken.

2. **The peer list goes stale.** A tracker hands out a list of peers and
   then asks to be left alone for the next half hour. The engine only
   announces again when fewer than **five** peers are left, so a torrent
   that is connected to a handful of leechers never asks for the seeds. When
   those few peers drop, the transfer stops until the next announce.

### Fixes

- The peer limit moved from 50 to 200, and a settings file written by the
  older version is migrated.
- The service now asks the trackers for new peers when a torrent stops
  receiving bytes for 45 seconds or when fewer than ten peers are connected,
  with a one minute cooldown per torrent.

A probe with the fix shows the difference. Without a re-announce the transfer
died after the first peers went away and stayed at zero for three minutes:

```
t= 20s progress= 28172373 down=0B/s     <- last byte
t=120s progress= 28172373 down=0B/s     <- still nothing
t=200s progress= 28172373 down=0B/s
```

With the re-announce the torrent asks the trackers again and picks up where
it left off:

```
t= 60s  -> re-announce #1 (thin swarm)
t=120s  -> re-announce #2 (stalled)
t=190s  -> re-announce #3 (stalled)
t=280s progress= 88481877 down=4069958B/s
t=300s progress=146278485 down=4127412B/s
```

The window also shows what the trackers say about the swarm next to the
peers that are connected, so "0 peers" can be read as "the swarm has 1476
seeders, none of them reached yet" instead of "nothing is there".

### What is still on the engine

The engine sorts its peer queue by arrival and does not prefer peers that
answer. With thousands of unreachable addresses in the queue, the useful
peers are found slowly. That lives in the engine
(`vendor/rustTorrent`), which is compiled from the upstream git dependency —
a change there needs an upstream commit and a new pinned revision.

---

## 6. Repository problems found on the way

- The rustTorrent submodule was half moved: `.gitmodules` pointed at
  `vendor/rustTorrent` while the index still held `reference/rustTorrent`,
  so `git submodule status` failed with
  `no submodule mapping found in .gitmodules`. The submodule is now
  registered at `vendor/rustTorrent` with the right revision.
- The engine is compiled from the **git dependency**, not from the vendored
  folder. A change in `vendor/rustTorrent` has no effect on the build until
  it is pushed upstream and the pinned revision moves. This is now written
  down in `src-tauri/Cargo.toml`.

---

## 7. The engine reuses torrent ids

The engine hands out `max(existing ids) + 1`, and `0` when the list is empty.
Deleting the only torrent and adding another one therefore gives the new
torrent the same id as the deleted one.

The window used to cache the file list, the file selection and the answered
reviews by that id, so a new torrent inherited the file list of the deleted
one — the Files tab showed the files of another download, and the review of
the new torrent was silenced because the id was already in the answered set.

Everything that identifies a download now uses the info hash:
`view.torrentKey`, `view.staleTorrentIds`, the cache in `state.files`, and
the backend check that drops a pending review whose torrent is gone.

A review is also skipped when the torrent it belongs to is already running:
handing the same magnet over twice keeps the torrent the engine already has,
and there is nothing to confirm about a download that is under way.

---

## 8. How to check it again

```bash
# The flow works, without a network and without a window.
cd src-tauri && cargo test --features torrent --lib torrent_review_flow_tests

# The same flow against the public swarm (needs internet, ~20 s).
cd src-tauri && cargo test --features torrent --lib -- \
  --ignored a_confirmed_review_downloads_from_the_real_swarm
```

For a manual check, open a torrent that is actually seeded — for example
`https://releases.ubuntu.com/24.04/ubuntu-24.04.4-desktop-amd64.iso.torrent`
— and watch the progress bar move.
