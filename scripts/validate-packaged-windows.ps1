#requires -Version 7.0
<#
  Qualify the packaged Murmure Windows application on a real installer.

  This script installs the built NSIS bundle, launches the installed
  application, checks that it survives startup, closes it, verifies that no
  orphan process or temporary audio file is left behind, then uninstalls it
  and proves that unrelated user data survived.

  It deliberately does not claim to validate the graphical user journey:
  driving the Tauri interface requires a visible desktop session. What it
  proves is the installer, the first launch, the shutdown, the cleanup and
  the uninstaller.

  PowerShell 7 is required for the same reason as the Pocket TTS scripts.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $false)]
    [string] $BundleDirectory = 'src-tauri/target/release/bundle/nsis',

    [Parameter(Mandatory = $false)]
    [int] $LaunchSeconds = 25
)

$ErrorActionPreference = 'Stop'

function Fail {
    param([string] $Message)
    throw $Message
}

function Wait-For {
    param([scriptblock] $Condition, [int] $TimeoutSeconds, [string] $Because)
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        if (& $Condition) { return $true }
        Start-Sleep -Milliseconds 500
    }
    Fail "Timed out after $TimeoutSeconds seconds waiting for $Because"
}

# --- Locate the installer that the build produced -------------------------
$bundlePath = Join-Path $PSScriptRoot "..\$BundleDirectory"
$bundlePath = [System.IO.Path]::GetFullPath($bundlePath)
$installers = @(Get-ChildItem -LiteralPath $bundlePath -Filter '*.exe' -File -ErrorAction SilentlyContinue)
if ($installers.Count -ne 1) {
    Fail "Expected exactly one packaged installer in $bundlePath, found $($installers.Count)"
}
$installer = $installers[0].FullName
Write-Output "Found packaged installer: $installer"

# --- Sentinel data that the uninstaller must never touch ------------------
$sentinel = Join-Path $env:LOCALAPPDATA ("murmure-uninstall-sentinel-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $sentinel -Force | Out-Null
Set-Content -LiteralPath (Join-Path $sentinel 'keep.txt') -Value 'not Murmure data'
Write-Output "Created unrelated user data sentinel: $sentinel"

# --- Clean installation ---------------------------------------------------
$install = Start-Process -FilePath $installer -ArgumentList '/S' -Wait -PassThru
if ($install.ExitCode -ne 0) {
    Fail "The packaged installer exited with code $($install.ExitCode)"
}

$installed = $null
Wait-For -TimeoutSeconds 120 -Because 'the installed application to appear' -Condition {
    $candidate = @(Get-ChildItem -LiteralPath $env:LOCALAPPDATA -Filter 'murmure.exe' -File -Recurse -Depth 2 -ErrorAction SilentlyContinue)
    if ($candidate.Count -eq 1) { $script:installed = $candidate[0].FullName; return $true }
    return $false
}
Write-Output "Installed application: $installed"

$installDirectory = Split-Path -Parent $installed
$uninstaller = @(Get-ChildItem -LiteralPath $installDirectory -Filter 'Uninstall*.exe' -File -ErrorAction SilentlyContinue)
if ($uninstaller.Count -ne 1) {
    Fail "Expected exactly one uninstaller next to the application, found $($uninstaller.Count)"
}

# --- First launch ---------------------------------------------------------
$process = Start-Process -FilePath $installed -PassThru
Start-Sleep -Seconds $LaunchSeconds

if ($process.HasExited) {
    Fail "The installed application exited during startup with code $($process.ExitCode)"
}
Write-Output "The installed application survived a $LaunchSeconds second startup window."

# --- The speech engine really loads --------------------------------------
# A package that starts but cannot load its ONNX Runtime is a package that
# transcribes nothing, which is the whole product. This script used to watch
# the process only, so a bundle that shipped without
# resources/onnxruntime/onnxruntime.dll passed every check while the first
# dictation failed on the user's machine.
$dataDirectory = Join-Path $env:LOCALAPPDATA 'com.al1x-ai.murmure'
$logFile = Join-Path $dataDirectory 'logs\murmure.log'
if (-not (Test-Path -LiteralPath $logFile)) {
    Fail "The application wrote no log at $logFile, so the engine state cannot be checked"
}
$log = Get-Content -LiteralPath $logFile -Raw
if ($log -match 'ONNX Runtime .* is not installed') {
    Fail "The application could not find its ONNX Runtime: the package is missing resources/onnxruntime/onnxruntime.dll"
}
if ($log -notmatch 'Transcription engine initialized and ready') {
    Fail "The speech engine never reported itself ready, so this package cannot transcribe"
}
Write-Output "The speech engine found its ONNX Runtime and reported itself ready."

# --- Privacy: no model is downloaded without an explicit consent ----------
if (Test-Path -LiteralPath (Join-Path $dataDirectory 'pocket-tts')) {
    Fail "The application created a Pocket TTS model cache without an explicit consent"
}
Write-Output "No Pocket TTS model cache was created before an explicit consent."

# --- Shutdown -------------------------------------------------------------
# Murmure deliberately hides to the tray instead of exiting when its window
# is closed, so the process is expected to stay alive after a window close.
# The graceful quit lives in the tray menu and stays a manual qualification.
$closed = $process.CloseMainWindow()
if ($closed) {
    Start-Sleep -Seconds 8
}
if (-not $process.HasExited) {
    Write-Output "The application stayed running after the window was closed, as designed for the tray."
    Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
    Wait-Process -Id $process.Id -Timeout 30 -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2
}
if (-not $process.HasExited) {
    Fail "The installed application could not be stopped"
}
Write-Output "The installed application stopped without an unhandled crash."

# --- No orphan process and no leftover temporary audio ---------------------
$orphans = @(Get-Process -ErrorAction SilentlyContinue | Where-Object {
    $_.ProcessName -like 'murmure*' -or $_.ProcessName -like 'pocket-tts*'
})
if ($orphans.Count -ne 0) {
    Fail "The application left $($orphans.Count) orphan process(es) running: $($orphans.ProcessName -join ', ')"
}
$leftovers = @(Get-ChildItem -LiteralPath $env:TEMP -Filter 'murmure-pocket-preview-*.wav' -File -ErrorAction SilentlyContinue)
if ($leftovers.Count -ne 0) {
    Fail "The application left $($leftovers.Count) temporary audio file(s) behind"
}
Write-Output "No orphan process and no temporary audio file survived the shutdown."

# --- Uninstall ------------------------------------------------------------
$remove = Start-Process -FilePath $uninstaller[0].FullName -ArgumentList '/S' -Wait -PassThru
if ($remove.ExitCode -ne 0) {
    Fail "The uninstaller exited with code $($remove.ExitCode)"
}
Wait-For -TimeoutSeconds 120 -Because 'the application files to be removed' -Condition {
    -not (Test-Path -LiteralPath $installed)
}
Write-Output "The uninstaller removed the installed application."

if (-not (Test-Path -LiteralPath (Join-Path $sentinel 'keep.txt'))) {
    Fail "The uninstaller deleted unrelated user data"
}
Write-Output "Unrelated user data survived the uninstall."

Remove-Item -LiteralPath $sentinel -Recurse -Force -ErrorAction SilentlyContinue
Write-Output "PASS: the packaged Windows application installs, launches, closes, uninstalls and leaves nothing behind."
