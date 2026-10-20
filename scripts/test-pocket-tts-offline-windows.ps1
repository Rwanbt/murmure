#requires -Version 5.1
<#
  Test that a cached public French Pocket TTS model supports synthesis
  after the executable's outbound connections are explicitly denied.
  No personal tokens, private text or audio files are retained.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)]
    [string] $Destination
)

$ErrorActionPreference = 'Stop'
& "$PSScriptRoot/verify-pocket-tts-windows.ps1" -Destination $Destination -GenerateFrench
if (-not $?) { throw 'The initial online French generation failed' }

$binaryFiles = @(Get-ChildItem -LiteralPath (Join-Path $Destination 'unpacked') -Recurse -File -Filter 'pocket-tts.exe')
if ($binaryFiles.Count -ne 1) { throw 'Expected exactly one verified executable' }
$binary = $binaryFiles[0].FullName

$profiles = @(Get-NetFirewallProfile -ErrorAction Stop | Where-Object { $_.Enabled })
if ($profiles.Count -eq 0) { throw 'Firewall is disabled: cannot prove offline mode' }
$ruleName = 'MurmurePocketTtsOffline-' + [guid]::NewGuid().ToString('N')
$offlineWav = Join-Path $Destination 'french-offline.wav'
$oldOffline = $env:HF_HUB_OFFLINE
$ruleCreated = $false
try {
    New-NetFirewallRule -Name $ruleName -DisplayName $ruleName -Direction Outbound -Action Block -Program $binary -Enabled True -Profile Any -ErrorAction Stop | Out-Null
    $ruleCreated = $true
    $rule = Get-NetFirewallRule -Name $ruleName -ErrorAction Stop
    if ($rule.Action -ne 'Block' -or $rule.Enabled -ne 'True') {
        throw 'The outbound blocking rule could not be verified'
    }

    # The firewall denies any outbound request from the CLI even if the
    # Rust hf-hub client does not honor the Python HF_HUB_OFFLINE flag.
    $env:HF_HUB_OFFLINE = '1'
    $psi = [System.Diagnostics.ProcessStartInfo]::new($binary)
    $psi.UseShellExecute = $false
    $psi.CreateNoWindow = $true
    $psi.RedirectStandardInput = $true
    foreach ($arg in @('generate','--language','french','--voice','estelle','--device','cpu','--text','-','--output-path',$offlineWav,'--quiet')) {
        [void] $psi.ArgumentList.Add($arg)
    }
    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $psi
    Write-Output 'Testing cached French synthesis with outbound network access denied to the Pocket TTS executable.'
    try {
        if (-not $process.Start()) { throw 'Unable to launch offline Pocket TTS' }
        $process.StandardInput.WriteLine('Bonjour. Ceci est un essai de synthese vocale locale.')
        $process.StandardInput.Close()
        if (-not $process.WaitForExit(360000)) {
            $process.Kill($true)
            [void] $process.WaitForExit(10000)
            throw 'Cached generation exceeded the 6 minute timeout'
        }
        if ($process.ExitCode -ne 0) { throw "Cached generation exited $($process.ExitCode)" }
    } finally {
        $process.Dispose()
    }
    if (-not (Test-Path -LiteralPath $offlineWav)) {
        throw 'Cached generation produced no WAV'
    }
    $file = [System.IO.File]::OpenRead($offlineWav)
    $reader = [System.IO.BinaryReader]::new($file)
    try {
        if ($file.Length -lt 2048) { throw 'Offline WAV too small' }
        $ascii = [System.Text.Encoding]::ASCII
        if ($ascii.GetString($reader.ReadBytes(4)) -ne 'RIFF') { throw 'Offline file is not RIFF' }
        [void] $reader.ReadUInt32()
        if ($ascii.GetString($reader.ReadBytes(4)) -ne 'WAVE') { throw 'Offline file is not WAVE' }
        Write-Output ("PASS: Cached French Pocket TTS generated a WAV without network access ({0} bytes)" -f $file.Length)
    } finally {
        $reader.Dispose()
        $file.Dispose()
    }
} finally {
    $env:HF_HUB_OFFLINE = $oldOffline
    if ($ruleCreated) { Remove-NetFirewallRule -Name $ruleName -ErrorAction Stop }
    Remove-Item -LiteralPath $offlineWav -Force -ErrorAction SilentlyContinue
}
