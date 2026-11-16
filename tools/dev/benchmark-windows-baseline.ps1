<#
.SYNOPSIS
    Captures performance baseline metrics for Open Download Manager on Windows.

.DESCRIPTION
    Measures:
    1. Cold-start / warm-start latency: time from process spawn until the main window is created and visible.
       Iteration 1 measures a Cold Start; iterations 2+ measure Warm Starts.
    2. Total resource footprint: sums CPU usage and memory (Working Set) across both the host
       executable (open-download-manager.exe) and all its WebView2 child processes (msedgewebview2.exe).

    NOTE ON SIDE EFFECTS & REGISTRY:
    The benchmark script itself makes NO registry changes and creates NO persistent files outside
    the specified CsvOutput. However, running the application executable (open-download-manager.exe)
    performs the application's standard per-user native messaging manifest sync in HKCU on startup.

    RUNNING INSTRUCTIONS:
    - Run in a normal (non-administrator) PowerShell console.
    - Run against the installed Release build (e.g. from GitHub release / Inno Setup), not dev build.
    - Close background resource-heavy applications before running.
    - Run 3 consecutive iterations to capture cold start and warm start behavior.

.PARAMETER AppPath
    Path to open-download-manager.exe. Defaults to LocalAppData installed location.

.PARAMETER Iterations
    Number of benchmark iterations to run (default: 3).

.PARAMETER WarmupSeconds
    Seconds to wait after window visibility before sampling steady-state CPU/memory (default: 5).

.PARAMETER CsvOutput
    Path to save benchmark CSV results (default: "windows-baseline.csv").
#>

[CmdletBinding()]
param(
    [string]$AppPath = "$env:LOCALAPPDATA\Programs\Open Download Manager\open-download-manager.exe",
    [int]$Iterations = 3,
    [int]$WarmupSeconds = 5,
    [string]$CsvOutput = "windows-baseline.csv"
)

# P/Invoke signature for checking true window visibility
$user32Code = @"
using System;
using System.Runtime.InteropServices;
public class Win32Window {
    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool IsWindowVisible(IntPtr hWnd);
}
"@

if (-not ([System.Management.Automation.PSTypeName]'Win32Window').Type) {
    Add-Type -TypeDefinition $user32Code
}

# Resolve target binary
if (-not (Test-Path $AppPath)) {
    # Check alternate debug/target path
    $fallback = ".\src-tauri\target\release\open-download-manager.exe"
    if (Test-Path $fallback) {
        $AppPath = (Resolve-Path $fallback).Path
    } else {
        Write-Error "Could not find open-download-manager.exe at: $AppPath"
        Write-Host "Please specify -AppPath pointing to your installed or built binary."
        exit 1
    }
}

Write-Host "=========================================================="
Write-Host " Open Download Manager - Windows Baseline Benchmark"
Write-Host "=========================================================="
Write-Host "Target: $AppPath"
Write-Host "Iterations: $Iterations"
Write-Host "Warmup time per iteration: ${WarmupSeconds}s"
Write-Host "CSV Output: $CsvOutput"
Write-Host "Safety Note: This script makes no registry changes, but the app writes HKCU native-messaging entries on launch."
Write-Host ""

# Pre-flight check: ensure clean initial state before benchmark starts
$existing = Get-Process -Name "open-download-manager" -ErrorAction SilentlyContinue
if ($existing) {
    Write-Warning "Existing open-download-manager process detected (PIDs: $($existing.Id -join ', ')). Stopping before benchmark..."
    $existing | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 1
}

$results = @()
$previousProcesses = @()

for ($i = 1; $i -le $Iterations; $i++) {
    $runType = if ($i -eq 1) { "Cold Start" } else { "Warm Start" }
    Write-Host "--- Iteration $i of $Iterations ($runType) ---"

    # Assert no leftover host processes before starting iteration
    $leftoverHost = Get-Process -Name "open-download-manager" -ErrorAction SilentlyContinue
    if ($leftoverHost) {
        throw "Assertion failed: Leftover open-download-manager process detected before iteration $i start (PIDs: $($leftoverHost.Id -join ', '))."
    }

    # Assert all child processes from previous iteration have fully exited
    foreach ($prevProc in $previousProcesses) {
        try {
            $prevProc.Refresh()
            if (-not $prevProc.HasExited) {
                throw "Assertion failed: Leftover child process (PID: $($prevProc.Id), Name: $($prevProc.ProcessName)) still alive before iteration $i start."
            }
        } catch [System.InvalidOperationException] {
            # Process handle invalid / already terminated - expected
        }
    }

    # Spawn process and measure time to visible main window
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $proc = Start-Process -FilePath $AppPath -PassThru

    $windowVisible = $false
    $timeoutMs = 15000

    while ($sw.ElapsedMilliseconds -lt $timeoutMs) {
        $proc.Refresh()
        if ($proc.HasExited) {
            Write-Warning "Process exited unexpectedly during startup benchmark."
            break
        }

        $hwnd = $proc.MainWindowHandle
        if ($hwnd -ne [IntPtr]::Zero) {
            if ([Win32Window]::IsWindowVisible($hwnd)) {
                $windowVisible = $true
                break
            }
        }
        Start-Sleep -Milliseconds 15
    }

    $sw.Stop()
    $startLatencyMs = $sw.ElapsedMilliseconds

    if (-not $windowVisible) {
        Write-Warning "Main window was not detected as visible within ${timeoutMs}ms."
    } else {
        Write-Host "  Main Window Visible ($runType): ${startLatencyMs} ms"
    }

    # Allow steady-state warmup to observe polling and idle load
    Write-Host "  Waiting ${WarmupSeconds}s for steady-state / polling observation..."
    Start-Sleep -Seconds $WarmupSeconds

    # Find host process + all msedgewebview2 child processes belonging to this tree
    $parentPid = $proc.Id
    $allProcesses = @($proc)

    # Query WMI / CIM for child processes of parent PID
    try {
        $children = Get-CimInstance Win32_Process -Filter "ParentProcessId = $parentPid" | ForEach-Object {
            Get-Process -Id $_.ProcessId -ErrorAction SilentlyContinue
        }
        if ($children) {
            $allProcesses += $children
        }
    } catch {
        # Fallback to name match if CIM query fails
        $wvChildren = Get-Process -Name "msedgewebview2" -ErrorAction SilentlyContinue
        if ($wvChildren) {
            $allProcesses += $wvChildren
        }
    }

    # Sample CPU across 1 second interval
    $cpuSamples1 = @{}
    foreach ($p in $allProcesses) {
        try {
            $p.Refresh()
            $cpuSamples1[$p.Id] = $p.TotalProcessorTime.TotalMilliseconds
        } catch {}
    }

    Start-Sleep -Seconds 1

    $totalCpuMs = 0.0
    $totalMemBytes = 0.0

    foreach ($p in $allProcesses) {
        try {
            $p.Refresh()
            if ($cpuSamples1.ContainsKey($p.Id)) {
                $delta = $p.TotalProcessorTime.TotalMilliseconds - $cpuSamples1[$p.Id]
                if ($delta -gt 0) {
                    $totalCpuMs += $delta
                }
            }
            $totalMemBytes += $p.WorkingSet64
        } catch {}
    }

    # CPU Percentage across all cores (1000 ms elapsed)
    $cpuCores = [Environment]::ProcessorCount
    $totalCpuPercent = [Math]::Round(($totalCpuMs / (1000.0 * $cpuCores)) * 100.0, 2)
    $totalMemMB = [Math]::Round($totalMemBytes / 1MB, 2)

    Write-Host "  Processes Measured: $($allProcesses.Count) (Host + WebView2 children)"
    Write-Host "  Total Steady-State CPU: $totalCpuPercent % (normalized over $cpuCores cores)"
    Write-Host "  Total Memory (Working Set): $totalMemMB MB"

    $timestamp = (Get-Date).ToString("yyyy-MM-ddTHH:mm:ssZ")
    $record = [PSCustomObject]@{
        Iteration        = $i
        RunType          = $runType
        Timestamp        = $timestamp
        StartupLatencyMs = $startLatencyMs
        TotalCpuPercent  = $totalCpuPercent
        TotalMemMB       = $totalMemMB
    }
    $results += $record

    # Gracefully close window and wait up to 10s for the host process to exit
    Write-Host "  Closing main window and waiting up to 10s for process exit..."
    try {
        $proc.CloseMainWindow() | Out-Null
        $proc.WaitForExit(10000) | Out-Null
    } catch {}

    # If host process is still running after 10s, force stop it and its WebView2 children
    if (-not $proc.HasExited) {
        Write-Warning "  Host process did not exit within 10s; forcibly terminating host and child processes..."
        foreach ($child in $allProcesses) {
            if ($child.Id -ne $proc.Id) {
                try {
                    Stop-Process -Id $child.Id -Force -ErrorAction SilentlyContinue
                } catch {}
            }
        }
        try {
            Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
            $proc.WaitForExit(2000) | Out-Null
        } catch {}
    } else {
        # Host exited, ensure all child webview processes from this run are terminated
        foreach ($child in $allProcesses) {
            if ($child.Id -ne $proc.Id) {
                try {
                    $child.Refresh()
                    if (-not $child.HasExited) {
                        Stop-Process -Id $child.Id -Force -ErrorAction SilentlyContinue
                    }
                } catch {}
            }
        }
    }

    $previousProcesses = $allProcesses
    Start-Sleep -Seconds 2
}

# Output Table Summary
Write-Host ""
Write-Host "================ BENCHMARK SUMMARY ================"
$results | Format-Table -AutoSize

# Export CSV
$results | Export-Csv -Path $CsvOutput -NoTypeInformation -Encoding utf8
Write-Host "Results exported to: $CsvOutput"
