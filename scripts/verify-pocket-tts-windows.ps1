#requires -Version 5.1
<#
  Verify the official Kyutai Pocket TTS Windows CPU executable.
  This is a CI/diagnostic smoke test, NOT a model installer.
  No speech text or weights are downloaded or stored.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)]
    [string] $Destination
)

$ErrorActionPreference = 'Stop'
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

# CI checks the CLI contract only. Generating audio requires a separately
# approved installation of French model weights and is not tested here.
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
