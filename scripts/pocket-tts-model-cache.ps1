#requires -Version 7.0
<#
  Isolate the Pocket TTS model cache from the user Hugging Face state.

  The Murmure application already points HF_HOME at its own cache and drops
  inherited Hugging Face credentials before starting the Pocket TTS binary.
  The verification scripts must offer the same guarantee, otherwise a
  developer machine silently reuses a personal token and pollutes the
  shared Hugging Face cache.

  Dot-source this file, then call:
      $cache = Initialize-PocketTtsModelCache -Destination <folder>
#>

function Initialize-PocketTtsModelCache {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string] $Destination
    )

    $ErrorActionPreference = 'Stop'
    $cache = Join-Path $Destination 'model-cache'
    if (-not (Test-Path -LiteralPath $cache)) {
        New-Item -ItemType Directory -Path $cache -Force | Out-Null
    }

    # Keep the previous values so a caller can restore them afterwards.
    $script:PocketTtsPreviousHome = $env:HF_HOME
    $script:PocketTtsPreviousToken = $env:HF_TOKEN
    $script:PocketTtsPreviousTokenPath = $env:HF_TOKEN_PATH
    $script:PocketTtsPreviousHubToken = $env:HUGGING_FACE_HUB_TOKEN
    $script:PocketTtsModelCacheInitialized = $true

    $env:HF_HOME = $cache
    # A personal login must never be attached to an automated download.
    Remove-Item Env:\HF_TOKEN -ErrorAction SilentlyContinue
    Remove-Item Env:\HF_TOKEN_PATH -ErrorAction SilentlyContinue
    Remove-Item Env:\HUGGING_FACE_HUB_TOKEN -ErrorAction SilentlyContinue
    Remove-Item Env:\HUGGINGFACE_HUB_CACHE -ErrorAction SilentlyContinue
    Remove-Item Env:\HF_HUB_CACHE -ErrorAction SilentlyContinue

    return $cache
}

function Restore-PocketTtsModelCache {
    [CmdletBinding()]
    param()

    # Never touch the environment when initialization never ran, otherwise a
    # failure before the cache was prepared would clear a user value.
    if (-not $script:PocketTtsModelCacheInitialized) { return }
    $script:PocketTtsModelCacheInitialized = $false

    if ($null -eq $script:PocketTtsPreviousHome) {
        Remove-Item Env:\HF_HOME -ErrorAction SilentlyContinue
    } else {
        $env:HF_HOME = $script:PocketTtsPreviousHome
    }
    if ($null -eq $script:PocketTtsPreviousToken) {
        Remove-Item Env:\HF_TOKEN -ErrorAction SilentlyContinue
    } else {
        $env:HF_TOKEN = $script:PocketTtsPreviousToken
    }
    if ($null -eq $script:PocketTtsPreviousTokenPath) {
        Remove-Item Env:\HF_TOKEN_PATH -ErrorAction SilentlyContinue
    } else {
        $env:HF_TOKEN_PATH = $script:PocketTtsPreviousTokenPath
    }
    if ($null -eq $script:PocketTtsPreviousHubToken) {
        Remove-Item Env:\HUGGING_FACE_HUB_TOKEN -ErrorAction SilentlyContinue
    } else {
        $env:HUGGING_FACE_HUB_TOKEN = $script:PocketTtsPreviousHubToken
    }
}
