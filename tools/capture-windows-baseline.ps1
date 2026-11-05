[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)]
  [string]$InstallerPath,

  [Parameter(Mandatory = $true)]
  [string]$InstallDirectory,

  [Parameter(Mandatory = $true)]
  [string]$ExecutablePath,

  [Parameter(Mandatory = $true)]
  [string]$OutputPath,

  [switch]$RebootedBeforeTrial,

  [ValidateRange(1, 600)]
  [int]$IdleSeconds = 60,

  [ValidateRange(1, 300)]
  [int]$StartupTimeoutSeconds = 30
)

$ErrorActionPreference = 'Stop'

$installer = (Resolve-Path -LiteralPath $InstallerPath).Path
$installDirectory = (Resolve-Path -LiteralPath $InstallDirectory).Path
$executable = (Resolve-Path -LiteralPath $ExecutablePath).Path

if (-not (Test-Path -LiteralPath $executable -PathType Leaf)) {
  throw "Application executable not found: $executable"
}

$installPrefix = $installDirectory.TrimEnd([IO.Path]::DirectorySeparatorChar) +
  [IO.Path]::DirectorySeparatorChar
if (-not $executable.StartsWith($installPrefix, [StringComparison]::OrdinalIgnoreCase)) {
  throw "Application executable must be inside the install directory: $installDirectory"
}

$processName = [IO.Path]::GetFileNameWithoutExtension($executable)
$existingProcesses = @(Get-Process -Name $processName -ErrorAction SilentlyContinue)
if ($existingProcesses.Count -gt 0) {
  throw "Close Awesome Downloader and wait for its processes to exit before capturing a trial."
}

$installerInfo = Get-Item -LiteralPath $installer
$installerHash = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash
$installedBytes = (Get-ChildItem -LiteralPath $installDirectory -File -Recurse -Force |
  Measure-Object -Property Length -Sum).Sum
if ($null -eq $installedBytes) {
  $installedBytes = [int64]0
}

$operatingSystem = Get-CimInstance -ClassName Win32_OperatingSystem
$computerSystem = Get-CimInstance -ClassName Win32_ComputerSystem
$processor = Get-CimInstance -ClassName Win32_Processor |
  Select-Object -First 1
$sourceRevision = git rev-parse HEAD
if ($LASTEXITCODE -ne 0) {
  throw 'Could not read the source revision. Run this helper from the repository.'
}
$sourceTreeStatus = git status --porcelain
if ($LASTEXITCODE -ne 0) {
  throw 'Could not read the source working-tree status.'
}
$sourceTreeState = if ([string]::IsNullOrWhiteSpace(($sourceTreeStatus -join ''))) {
  'clean'
} else {
  'dirty'
}
$nodeVersion = node --version
if ($LASTEXITCODE -ne 0) {
  throw 'Could not read the Node.js version.'
}
$npmVersion = npm --version
if ($LASTEXITCODE -ne 0) {
  throw 'Could not read the npm version.'
}

$startupTimer = [Diagnostics.Stopwatch]::StartNew()
$launchedProcess = Start-Process -FilePath $executable -PassThru
$windowProcess = $null
$timeout = [TimeSpan]::FromSeconds($StartupTimeoutSeconds)

while ($startupTimer.Elapsed -lt $timeout) {
  $launchedProcess.Refresh()
  if ($launchedProcess.HasExited) {
    throw "Application exited before its main window appeared (exit code $($launchedProcess.ExitCode))."
  }

  $windowProcess = Get-Process -Name $processName -ErrorAction SilentlyContinue |
    Where-Object { $_.MainWindowHandle -ne [IntPtr]::Zero -and $_.MainWindowTitle -eq 'Awesome Downloader' } |
    Select-Object -First 1
  if ($null -ne $windowProcess) {
    break
  }

  Start-Sleep -Milliseconds 100
}

if ($null -eq $windowProcess) {
  throw "Timed out after $StartupTimeoutSeconds seconds waiting for the Awesome Downloader window."
}

$startupTimer.Stop()
Start-Sleep -Seconds $IdleSeconds

$applicationProcesses = @(Get-Process -Name $processName -ErrorAction SilentlyContinue)
if ($applicationProcesses.Count -eq 0) {
  throw "Application exited during the $IdleSeconds-second idle measurement."
}

$workingSetBytes = [int64](($applicationProcesses |
  Measure-Object -Property WorkingSet64 -Sum).Sum)
$privateBytes = [int64](($applicationProcesses |
  Measure-Object -Property PrivateMemorySize64 -Sum).Sum)

$result = [PSCustomObject]@{
  CapturedAtUtc          = [DateTime]::UtcNow.ToString('o')
  SourceRevision         = ($sourceRevision -join '').Trim()
  SourceTreeState        = $sourceTreeState
  RebootedBeforeTrial    = [bool]$RebootedBeforeTrial
  WindowsCaption         = $operatingSystem.Caption
  WindowsVersion         = $operatingSystem.Version
  WindowsBuild           = $operatingSystem.BuildNumber
  WindowsArchitecture    = $operatingSystem.OSArchitecture
  ComputerManufacturer   = $computerSystem.Manufacturer
  ComputerModel          = $computerSystem.Model
  Processor              = $processor.Name
  InstalledRamGiB        = [Math]::Round($computerSystem.TotalPhysicalMemory / 1GB, 2)
  InstallerPath          = $installer
  InstallerBytes         = $installerInfo.Length
  InstallerSha256        = $installerHash
  InstallDirectory       = $installDirectory
  InstalledBytes         = $installedBytes
  ExecutablePath         = $executable
  ProductVersion         = (Get-Item -LiteralPath $executable).VersionInfo.ProductVersion
  StartupMilliseconds    = [Math]::Round($startupTimer.Elapsed.TotalMilliseconds, 2)
  IdleSeconds            = $IdleSeconds
  ApplicationProcessCount = $applicationProcesses.Count
  IdleWorkingSetBytes    = $workingSetBytes
  IdlePrivateBytes       = $privateBytes
  NodeVersion            = ($nodeVersion -join '').Trim()
  NpmVersion             = ($npmVersion -join '').Trim()
}

$outputFullPath = [IO.Path]::GetFullPath($OutputPath)
$outputDirectory = Split-Path -Parent $outputFullPath
if (-not (Test-Path -LiteralPath $outputDirectory -PathType Container)) {
  New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null
}

if (Test-Path -LiteralPath $outputFullPath -PathType Leaf) {
  $result | Export-Csv -LiteralPath $outputFullPath -NoTypeInformation -Append
} else {
  $result | Export-Csv -LiteralPath $outputFullPath -NoTypeInformation
}

$result
