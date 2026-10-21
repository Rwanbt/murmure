#requires -Version 7.0
<#
  Verify the official Kyutai Pocket TTS Windows CPU executable.
  By default this only validates the CLI. With -GenerateFrench, runs a
  fixed test phrase against the official ungated French model fallback.
  This is a disposable CI diagnostic, NOT a model installer.

  PowerShell 7 is required because the generation step drives the child
  process through ProcessStartInfo.ArgumentList, which only exists on
  .NET Core. On Windows PowerShell 5.1 that property is null and the run
  would abort with an opaque "method call on a null-valued expression".
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)]
    [string] $Destination,
    [switch] $GenerateFrench
)

$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/verify-pocket-tts-wave.ps1"
. "$PSScriptRoot/pocket-tts-model-cache.ps1"
$release = 'v4.0.0'
$archive = 'pocket-tts-3.3.0-x86_64-pc-windows-msvc.zip'
$expectedSha256 = 'b3b2faa74703c505e22a7f428c984f69577548c85440c4c9eee0d336797f3cef'
$uri = "https://github.com/kyutai-labs/pocket-tts/releases/download/$release/$archive"

# Never overwrite existing sidecars or unrelated user files.
if (Test-Path -LiteralPath $Destination) {
    if (@(Get-ChildItem -LiteralPath $Destination -Force).Count -gt 0) {
        throw "Destination must be an empty staging folder"
    }
} else {
    New-Item -ItemType Directory -Path $Destination -Force | Out-Null
}

$zip = Join-Path $Destination 'pocket-tts.zip'
$expanded = Join-Path $Destination 'unpacked'
Write-Output "Retrieving pinned Pocket TTS release $release"
Invoke-WebRequest -Uri $uri -OutFile $zip -UseBasicParsing
$actual = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant()
if ($actual -ne $expectedSha256) {
    Remove-Item -LiteralPath $zip -Force
    throw "Pocket TTS archive digest mismatch; refusing to execute"
}

Expand-Archive -LiteralPath $zip -DestinationPath $expanded
$binaries = @(Get-ChildItem -Path $expanded -Recurse -File -Filter 'pocket-tts.exe')
if ($binaries.Count -ne 1) { throw "Expected one Pocket TTS executable, found $($binaries.Count)" }
$binary = $binaries[0].FullName
$binarySha256 = (Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash.ToLowerInvariant()
$expectedBinarySha256 = "9b52ad9d8721869c0adcb98e62a5e89e8fbfef0abdc397b4189db411456c6023"
if ($binarySha256 -ne $expectedBinarySha256) { throw "The extracted Pocket TTS executable does not match the pinned Murmure runtime" }
Write-Output "PASS: Extracted Pocket TTS executable SHA256: $binarySha256"

# Always validate the command line without using speech/model assets.
# Actual synthesis runs only when -GenerateFrench is explicitly passed.
$globalHelp = (& $binary --help | Out-String)
if ($LASTEXITCODE -ne 0 -or $globalHelp -notmatch 'generate') {
    throw "Pocket TTS executable does not support generate"
}
$generateHelp = (& $binary generate --help | Out-String)
if ($LASTEXITCODE -ne 0) { throw "Pocket TTS generate --help failed" }
foreach ($flag in @('--language','--voice','--text','--device','--output-path')) {
    if (-not $generateHelp.Contains($flag)) { throw "Missing required CLI argument: $flag" }
}
Write-Output "PASS: Pinned Pocket TTS Windows binary digest and generation interface verified."

if ($GenerateFrench) {
    # The released CLI first checks the gated voice-cloning model, then
    # falls back to the public without-voice-cloning French model.
    # No Hugging Face login or user-supplied credential is required.
    # Fixed test text; never pass personal content through CI.
    $wav = Join-Path $Destination 'french-smoke.wav'
    # Keep the model download inside this staging folder and never let a
    # personal Hugging Face login leak into an automated run.
    $null = Initialize-PocketTtsModelCache -Destination $Destination
    $psi = [System.Diagnostics.ProcessStartInfo]::new($binary)
    $psi.UseShellExecute = $false
    $psi.CreateNoWindow = $true
    $psi.RedirectStandardInput = $true
    foreach ($arg in @('generate', '--language', 'french', '--voice', 'estelle', '--device', 'cpu', '--text', '-', '--output-path', $wav, '--quiet')) {
        [void] $psi.ArgumentList.Add($arg)
    }
    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $psi
    Write-Output 'Running a fixed French synthesis on the Windows CPU (no voice cloning).'
    try {
        if (-not $process.Start()) { throw 'Failed to launch the official Pocket TTS binary' }
        $process.StandardInput.WriteLine('Bonjour. Ceci est un essai de synthese vocale locale.')
        $process.StandardInput.Close()
        if (-not $process.WaitForExit(900000)) {
            $process.Kill($true)
            [void] $process.WaitForExit(10000)
            throw 'French audio generation exceeded the 15 minute bound'
        }
        if ($process.ExitCode -ne 0) {
            throw "French audio generation failed with exit code $($process.ExitCode)"
        }
    } finally {
        $process.Dispose()
        Restore-PocketTtsModelCache
    }

    if (-not (Test-Path -LiteralPath $wav)) {
        throw 'Pocket TTS returned success without creating a WAV file'
    }
    # Parse real RIFF chunks, not a fixed 44-byte assumption: extra metadata
    # and chunk padding are permitted, but invalid/empty audio is not.
    try {
        $wave = Test-PocketTtsWave -Path $wav -MinimumSeconds 0.5
    } finally {
        Remove-Item -LiteralPath $wav -Force -ErrorAction SilentlyContinue
    }
    $duration = [string]::Format(
        [System.Globalization.CultureInfo]::InvariantCulture, '{0:F2}', $wave.DurationSeconds)
    Write-Output (
        "PASS: French CPU model generated non-silent PCM WAV, duration={0}s, Hz={1}, peak={2}" -f
            $duration, $wave.SampleRate, $wave.PeakAmplitude)
}

