# Windows migration baseline runbook

Use this procedure on a Windows x64 machine before replacing the Electron
desktop shell. Keep the results with the tested build; do not commit machine-
specific results or user data. Repeat the same procedure and use the same test
inputs for the Tauri candidate.

## Prepare the test machine

- Use a Windows x64 machine with stable power and close unrelated applications.
  Record the Windows edition, version/build, CPU, RAM, and Bun version.
- Use a clean user account or VM snapshot. Do not include an existing
  `%USERPROFILE%\.awesome-downloader` directory in installed-size measurements.
- Check out the exact source revision to be measured. Record `git rev-parse HEAD`
  and whether the working tree is clean.
- Build and test on Windows:

  ```powershell
  bun install --frozen-lockfile
  bun run test
  bun run build
  ```

  Installer packaging is disabled while the Tauri migration is in progress.
  Record the build output and do not treat a successful app build as an installer
  or runtime validation.
- Install the application from that installer. Record the install directory and
  the path to `Awesome Downloader.exe`. Do not launch it until the package hash
  and installed size have been captured.

## Capture package, startup, and idle memory

Run the helper from the repository root in PowerShell. Provide the actual
installer, install directory, executable, and a CSV path outside the install
directory:

```powershell
.\tools\capture-windows-baseline.ps1 `
  -InstallerPath .\release\Awesome-Downloader-Setup.exe `
  -InstallDirectory "$env:LOCALAPPDATA\Programs\Awesome Downloader" `
  -ExecutablePath "$env:LOCALAPPDATA\Programs\Awesome Downloader\Awesome Downloader.exe" `
  -OutputPath "$env:USERPROFILE\Documents\awesome-downloader-baseline.csv"
```

The helper records the installer SHA-256 and byte size, installed file byte
size, source revision/tree state, machine/runtime details, time from launch to
the app's top-level window handle, and aggregate working-set/private-memory
size after 60 idle seconds. Pass `-RebootedBeforeTrial` when the machine was
rebooted since the preceding trial.
The startup number is a process-cold, launch-to-window-handle measure, not a
first-paint measure. Keep that definition unchanged when measuring Tauri.

Run five trials. Between trials, quit the application from its tray menu and
confirm its processes have exited before starting the helper again. Keep the
machine on the same power state; record whether each trial followed a reboot.
The CSV is appended to, so use one file for all trials. Report the median and
range for startup and idle memory alongside each raw trial.

The helper leaves the app running. Quit it normally after each capture. It
refuses to start a trial while any process with the app's executable name is
already running, avoiding accidental single-instance launches.

## Benchmark download behavior

Use the same Windows machine, build, server, payloads, and procedure for the
Electron and Tauri measurements. Prefer a local, deterministic HTTP fixture to
avoid Internet variation. For every run, record the payload size and SHA-256,
elapsed transfer time, resulting file SHA-256, and any errors. Do not use
authenticated URLs, browser cookies, signed media links, or private content.

Record separate results for:

| Scenario | Required observations |
| --- | --- |
| Regular HTTP download | Payload size/hash, elapsed time, output hash |
| Segmented HTTP download | Same observations; include configured segment count and confirm range support |
| Pause and resume | Pause point and duration, resumed elapsed time, final output hash |
| Video/audio assembly | Input fixture hashes, FFmpeg version, elapsed time, output hash |

Use at least five successful transfer trials per non-interrupted scenario and
three pause/resume trials. Verify output hashes before including timing data.
Keep fixture generation and server instructions with the results so the
measurements can be repeated; do not substitute unrelated public test URLs
between implementations.

## Results record

Keep the raw CSV and a companion record with the following fields:

| Field | Value |
| --- | --- |
| Source revision / working-tree state | |
| Windows edition and version/build | |
| Machine model / CPU / RAM | |
| Bun version | |
| Installer path / bytes / SHA-256 | |
| Install directory / installed bytes | |
| Startup trials / median / range | |
| Idle-memory trials / median / range | |
| Download fixture and server revision | |
| HTTP / segmented / pause-resume / assembly results | |
| Test date and operator | |

Set size and performance targets only after these results have been collected.
Do not treat the baseline phase as complete until results and target thresholds
are recorded.
