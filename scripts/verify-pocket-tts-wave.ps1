#requires -Version 7.0
<#
  Shared structural validation for a Pocket TTS generated WAV file.

  The parser walks the real RIFF chunk list instead of assuming a fixed
  44-byte header, so extra metadata chunks and word padding are tolerated
  while truncated, malformed or silent audio is rejected.

  The chunk walk uses explicit byte offsets over the whole file. Mixing a
  buffered BinaryReader with raw stream positions desynchronizes the walk,
  because BinaryReader may refill its internal buffer and move the stream
  cursor further than the bytes it actually consumed.

  Dot-source this file, then call:
      $info = Test-PocketTtsWave -Path <wav> [-MinimumSeconds <seconds>]
#>

function Test-PocketTtsWave {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string] $Path,

        [Parameter(Mandatory = $false)]
        [double] $MinimumSeconds = 0.5
    )

    $ErrorActionPreference = 'Stop'
    if (-not (Test-Path -LiteralPath $Path)) {
        throw "Pocket TTS reported success but produced no WAV file at $Path"
    }

    $bytes = [System.IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -lt 48) { throw 'Generated WAV is too small to be valid audio' }

    $ascii = [System.Text.Encoding]::ASCII
    if ($ascii.GetString($bytes, 0, 4) -ne 'RIFF') { throw 'Generated file is not a RIFF container' }
    if ($ascii.GetString($bytes, 8, 4) -ne 'WAVE') { throw 'Generated file is not a WAV stream' }
    $riffSize = [long][System.BitConverter]::ToUInt32($bytes, 4)
    if ($riffSize + 8 -gt $bytes.Length) {
        throw 'RIFF container declares more audio than the file actually contains'
    }

    $format = 0; $channels = 0; $rate = 0; $bits = 0; $dataSize = 0; $peak = 0
    $offset = 12
    while ($offset + 8 -le $bytes.Length) {
        $chunk = $ascii.GetString($bytes, $offset, 4)
        $size = [long][System.BitConverter]::ToUInt32($bytes, $offset + 4)
        $start = $offset + 8
        if ($start + $size -gt $bytes.Length) { throw 'WAV chunk exceeds file boundaries' }
        if ($chunk -eq 'fmt ') {
            if ($size -lt 16) { throw 'Invalid WAV format chunk' }
            $format = [System.BitConverter]::ToUInt16($bytes, $start)
            $channels = [System.BitConverter]::ToUInt16($bytes, $start + 2)
            $rate = [System.BitConverter]::ToUInt32($bytes, $start + 4)
            $bits = [System.BitConverter]::ToUInt16($bytes, $start + 14)
        }
        if ($chunk -eq 'data') {
            $dataSize = $size
            # 16-bit PCM samples are word aligned inside a RIFF chunk.
            $limit = [int][Math]::Min(65536, $size)
            for ($index = 0; $index + 1 -lt $limit; $index += 2) {
                $magnitude = [Math]::Abs([int][System.BitConverter]::ToInt16($bytes, $start + $index))
                if ($magnitude -gt $peak) { $peak = $magnitude }
            }
        }
        $offset = $start + $size + ($size % 2)
    }

    if ($format -ne 1) { throw "Generated WAV is not uncompressed PCM (format=$format)" }
    if ($channels -ne 1) { throw "Unexpected WAV channel count ($channels)" }
    if ($rate -ne 24000) { throw "Unexpected WAV sample rate ($rate)" }
    if ($bits -ne 16) { throw "Unexpected WAV bit depth ($bits)" }
    if ($dataSize -le 0) { throw 'Generated WAV carries no audio samples' }

    $seconds = [double] $dataSize / ($rate * $channels * ($bits / 8))
    if ($peak -le 0) { throw 'Generated WAV is silent' }
    if ($seconds -lt $MinimumSeconds) {
        $rendered = [string]::Format(
            [System.Globalization.CultureInfo]::InvariantCulture, '{0:F2}', $seconds)
        $bound = [string]::Format(
            [System.Globalization.CultureInfo]::InvariantCulture, '{0:F2}', $MinimumSeconds)
        throw ("Generated WAV is shorter than {0} seconds ({1}s)" -f $bound, $rendered)
    }

    return [PSCustomObject]@{
        Path            = $Path
        DurationSeconds = $seconds
        SampleRate      = $rate
        Channels        = $channels
        BitsPerSample   = $bits
        DataBytes       = $dataSize
        PeakAmplitude   = $peak
    }
}
