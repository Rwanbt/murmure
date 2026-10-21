#requires -Version 7.0
<#
  Deterministic tests for the Pocket TTS model cache isolation.

  An automated Pocket TTS run must never reuse a personal Hugging Face
  login and must never write into the shared Hugging Face cache.
#>
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/pocket-tts-model-cache.ps1"

$script:failures = 0
$script:checks = 0

function Assert-True {
    param([bool] $Condition, [string] $Because)
    $script:checks++
    if ($Condition) {
        Write-Output "PASS: $Because"
    } else {
        Write-Output "FAIL: $Because"
        $script:failures++
    }
}

$staging = Join-Path ([System.IO.Path]::GetTempPath()) ("murmure-model-cache-{0}" -f [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $staging -Force | Out-Null

try {
    # Simulate a developer machine that already carries a personal login.
    $env:HF_HOME = Join-Path $staging 'personal-hf-home'
    $env:HF_TOKEN = 'hf_personal_token_must_not_leak'
    $env:HF_TOKEN_PATH = Join-Path $staging 'token.txt'
    $env:HUGGING_FACE_HUB_TOKEN = 'hf_personal_token_must_not_leak'

    $cache = Initialize-PocketTtsModelCache -Destination $staging

    Assert-True ($cache -eq (Join-Path $staging 'model-cache')) 'the model cache is isolated inside the staging folder'
    Assert-True (Test-Path -LiteralPath $cache) 'the isolated model cache is created'
    Assert-True ($env:HF_HOME -eq $cache) 'HF_HOME points at the isolated model cache'
    Assert-True ($null -eq $env:HF_TOKEN) 'HF_TOKEN is not inherited'
    Assert-True ($null -eq $env:HF_TOKEN_PATH) 'HF_TOKEN_PATH is not inherited'
    Assert-True ($null -eq $env:HUGGING_FACE_HUB_TOKEN) 'HUGGING_FACE_HUB_TOKEN is not inherited'

    Restore-PocketTtsModelCache

    Assert-True ($env:HF_HOME -eq (Join-Path $staging 'personal-hf-home')) 'the original HF_HOME is restored'
    Assert-True ($env:HF_TOKEN -eq 'hf_personal_token_must_not_leak') 'the original HF_TOKEN is restored'
    Assert-True ($env:HF_TOKEN_PATH -eq (Join-Path $staging 'token.txt')) 'the original HF_TOKEN_PATH is restored'
    Assert-True ($env:HUGGING_FACE_HUB_TOKEN -eq 'hf_personal_token_must_not_leak') 'the original HUGGING_FACE_HUB_TOKEN is restored'

    # Restoring twice, or without initializing, must never clear the
    # environment of a caller that never opted into the isolation.
    Remove-Item Env:\HF_HOME -ErrorAction SilentlyContinue
    Restore-PocketTtsModelCache
    Restore-PocketTtsModelCache
    Assert-True ($null -eq $env:HF_HOME) 'a redundant restore leaves the environment untouched'
} finally {
    Remove-Item Env:\HF_TOKEN -ErrorAction SilentlyContinue
    Remove-Item Env:\HF_TOKEN_PATH -ErrorAction SilentlyContinue
    Remove-Item Env:\HUGGING_FACE_HUB_TOKEN -ErrorAction SilentlyContinue
    Remove-Item Env:\HF_HOME -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $staging -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Output ("Pocket TTS model cache isolation: {0} checks, {1} failures" -f $script:checks, $script:failures)
if ($script:failures -ne 0) { exit 1 }
