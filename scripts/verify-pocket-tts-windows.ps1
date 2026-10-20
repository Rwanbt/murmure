#requires -Version 5.1
<#
  Verify the official Kyutai Pocket TTS Windows CPU executable.
  By default this only validates the CLI. With -GenerateFrench, runs a
  fixed test phrase against the official ungated French model fallback.
  This is a disposable CI diagnostic, NOT a model installer.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)]
    [string] $Destination,
    [switch] $GenerateFrench
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
    }

    if (-not (Test-Path -LiteralPath $wav)) {
        throw 'Pocket TTS returned success without creating a WAV file'
    }
    # Parse real RIFF chunks, not a fixed 44-byte assumption: extra metadata
    # and chunk padding are permitted, but invalid/empty audio is not.
    $stream = [System.IO.File]::OpenRead($wav)
    $reader = [System.IO.BinaryReader]::new($stream)
    try {
        if ($stream.Length -lt 48) { throw 'Generated WAV is too small' }
        $ascii = [System.Text.Encoding]::ASCII
        if ($ascii.GetString($reader.ReadBytes(4)) -ne 'RIFF') { throw 'Not a RIFF file' }
        [void] $reader.ReadUInt32()
        if ($ascii.GetString($reader.ReadBytes(4)) -ne 'WAVE') { throw 'Not a WAV file' }
        $channels = 0; $rate = 0; $bits = 0; $format = 0; $dataSize = 0; $nonSilent = $false
        while ($stream.Position + 8 -le $stream.Length) {
            $chunk = $ascii.GetString($reader.ReadBytes(4))
            $size = [long] $reader.ReadUInt32()
            $start = $stream.Position
            if ($start + $size -gt $stream.Length) { throw 'WAV chunk exceeds file boundaries' }
            if ($chunk -eq 'fmt ') {
                if ($size -lt 16) { throw 'Invalid WAV format chunk' }
                $format = $reader.ReadUInt16()
                $channels = $reader.ReadUInt16()
                $rate = $reader.ReadUInt32()
                [void] $reader.ReadUInt32()
                [void] $reader.ReadUInt16()
                $bits = $reader.ReadUInt16()
            }
            if ($chunk -eq 'data') {
                $dataSize = $size
                $sample = $reader.ReadBytes([int][Math]::Min(32768, $dataSize))
                foreach ($b in $sample) {
                    if ($b -ne 0) { $nonSilent = $true; break }
                }
            }
            $stream.Position = $start + $size + ($size % 2)
        }
        if ($format -ne 1 -or $channels -ne 1 -or $rate -ne 24000 -or $bits -ne 16) {
            throw "Unexpected WAV format (PCM=$format, channels=$channels, Hz=$rate, bits=$bits)"
        }
        $seconds = [double] $dataSize / ($rate * $channels * ($bits / 8))
        if ($seconds -lt 0.5 -or -not $nonSilent) {
            throw 'WAV is silent or shorter than half a second'
        }
        Write-Output ("PASS: French CPU model generated non-silent PCM WAV, duration={0:N2}s, Hz={1}" -f $seconds, $rate)
    } finally {
        $reader.Dispose()
        $stream.Dispose()
        Remove-Item -LiteralPath $wav -Force -ErrorAction SilentlyContinue
    }
}

