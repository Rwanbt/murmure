#requires -Version 7.0
<#
  Deterministic tests for the shared Pocket TTS WAV validator.

  These tests never launch the Pocket TTS runtime. They exercise only the
  structural validator, so they can run on every platform and on every
  pull request. Evidence that the official runtime really synthesizes
  French speech stays in verify-pocket-tts-windows.ps1, which drives the
  pinned Kyutai binary.
#>
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/verify-pocket-tts-wave.ps1"

$script:failures = 0
$script:checks = 0

function New-TestWave {
    param(
        [int] $SampleRate = 24000,
        [int] $Channels = 1,
        [int] $BitsPerSample = 16,
        [int] $Samples = 24000,
        [switch] $Silent,
        [switch] $Truncate,
        [switch] $NotRiff,
        [int] $DeclaredDataBytes = -1
    )

    $path = Join-Path ([System.IO.Path]::GetTempPath()) ("murmure-wave-{0}.wav" -f [guid]::NewGuid().ToString('N'))
    $bytes = [System.Collections.Generic.List[byte]]::new()

    if ($NotRiff) {
        $bytes.AddRange([System.Text.Encoding]::ASCII.GetBytes('NOTRIFF'))
        [System.IO.File]::WriteAllBytes($path, $bytes.ToArray())
        return $path
    }

    $dataSize = $Samples * $Channels * ($BitsPerSample / 8)
    # The data chunk header can claim more audio than the file really holds.
    $declared = if ($DeclaredDataBytes -ge 0) { $DeclaredDataBytes } else { $dataSize }
    $riffSize = 36 + $declared

    $bytes.AddRange([System.Text.Encoding]::ASCII.GetBytes('RIFF'))
    $bytes.AddRange([System.BitConverter]::GetBytes([int]$riffSize))
    $bytes.AddRange([System.Text.Encoding]::ASCII.GetBytes('WAVE'))
    $bytes.AddRange([System.Text.Encoding]::ASCII.GetBytes('fmt '))
    $bytes.AddRange([System.BitConverter]::GetBytes([int]16))
    # format, channels, blockAlign and bits are 16-bit fields in a WAV header.
    $bytes.AddRange([System.BitConverter]::GetBytes([int16]1))
    $bytes.AddRange([System.BitConverter]::GetBytes([int16]$Channels))
    $bytes.AddRange([System.BitConverter]::GetBytes([int]$SampleRate))
    $bytes.AddRange([System.BitConverter]::GetBytes([int]($SampleRate * $Channels * ($BitsPerSample / 8))))
    $bytes.AddRange([System.BitConverter]::GetBytes([int16]($Channels * ($BitsPerSample / 8))))
    $bytes.AddRange([System.BitConverter]::GetBytes([int16]$BitsPerSample))
    $bytes.AddRange([System.Text.Encoding]::ASCII.GetBytes('data'))
    $bytes.AddRange([System.BitConverter]::GetBytes([int]$declared))

    for ($index = 0; $index -lt ($Samples * $Channels); $index++) {
        if ($Silent) {
            $bytes.AddRange([System.BitConverter]::GetBytes([int16]0))
        } else {
            $value = [int16](12000 * [Math]::Sin(2 * [Math]::PI * 440 * $index / $SampleRate))
            $bytes.AddRange([System.BitConverter]::GetBytes($value))
        }
    }

    $payload = $bytes.ToArray()
    if ($Truncate) { $payload = $payload[0..199] }
    [System.IO.File]::WriteAllBytes($path, $payload)
    return $path
}

function Assert-Rejected {
    param([string] $Path, [string] $Because)
    $script:checks++
    try {
        $null = Test-PocketTtsWave -Path $Path -MinimumSeconds 0.5
        Write-Output "FAIL: the validator accepted an invalid WAV ($Because)"
        $script:failures++
    } catch {
        Write-Output ("PASS: rejected invalid WAV ({0}) -> {1}" -f $Because, $_.Exception.Message)
    } finally {
        Remove-Item -LiteralPath $Path -Force -ErrorAction SilentlyContinue
    }
}

function Assert-Accepted {
    param([string] $Path, [string] $Because)
    $script:checks++
    try {
        $wave = Test-PocketTtsWave -Path $Path -MinimumSeconds 0.5
        $duration = [string]::Format(
            [System.Globalization.CultureInfo]::InvariantCulture, '{0:F2}', $wave.DurationSeconds)
        Write-Output ("PASS: accepted a valid WAV ({0}) -> duration={1}s Hz={2} peak={3}" -f
            $Because, $duration, $wave.SampleRate, $wave.PeakAmplitude)
    } catch {
        Write-Output ("FAIL: the validator rejected a valid WAV ({0}) -> {1}" -f $Because, $_.Exception.Message)
        $script:failures++
    } finally {
        Remove-Item -LiteralPath $Path -Force -ErrorAction SilentlyContinue
    }
}

Assert-Accepted -Path (New-TestWave) -Because 'one second of 440 Hz audio'
Assert-Accepted -Path (New-TestWave -Samples 48000) -Because 'two seconds of 440 Hz audio'

Assert-Rejected -Path (New-TestWave -Silent) -Because 'silent PCM payload'
Assert-Rejected -Path (New-TestWave -Samples 1200) -Because 'audio shorter than half a second'
Assert-Rejected -Path (New-TestWave -Truncate) -Because 'truncated RIFF container'
Assert-Rejected -Path (New-TestWave -NotRiff) -Because 'file without a RIFF header'
Assert-Rejected -Path (New-TestWave -SampleRate 16000) -Because 'unexpected sample rate'
Assert-Rejected -Path (New-TestWave -Channels 2) -Because 'unexpected channel count'
Assert-Rejected -Path (New-TestWave -DeclaredDataBytes 480000) -Because 'data chunk longer than the file'
Assert-Rejected -Path (New-TestWave -Samples 1200 -DeclaredDataBytes 240000) -Because 'RIFF container longer than the file'
Assert-Rejected -Path (Join-Path ([System.IO.Path]::GetTempPath()) 'murmure-missing-wave-fixture.wav') -Because 'missing WAV file'

Write-Output ("Pocket TTS WAV validator: {0} checks, {1} failures" -f $script:checks, $script:failures)
if ($script:failures -ne 0) { exit 1 }
