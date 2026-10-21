<#
.SYNOPSIS
    Installs the official CPU-only ONNX Runtime that the speech recognition
    engine links against.

.DESCRIPTION
    The prebuilt distribution that the `ort` build script downloads on Windows
    is compiled with the DirectML execution provider, so it drags the DirectX
    12 libraries into the import table of every binary that links it. The
    library `DirectML.dll` needs the entry point
    `D3D12SerializeVersionedRootSignature` from `d3d12.dll`, and a Windows
    installation whose `d3d12.dll` does not export it refuses to start the
    program with STATUS_ENTRYPOINT_NOT_FOUND, even though the speech
    recognition is a pure CPU workload.

    This script installs the official Microsoft CPU-only distribution instead.
    That distribution has no DirectX dependency at all. The archive is
    verified against the digest that Microsoft publishes with the release
    before anything is written to the disk.

    The script prints the two environment variables that make the build use
    that distribution. The caller has to set them before the crate is built,
    because the build script of `ort-sys` reads them.

.PARAMETER Destination
    The directory the runtime is installed into.

.PARAMETER Version
    The version of the ONNX Runtime to install.

.PARAMETER Force
    Reinstall the runtime even when it is already present.

.EXAMPLE
    ./scripts/install-onnx-runtime.ps1
#>
[CmdletBinding()]
param(
    [string]$Destination,
    [string]$Version = '1.23.2',
    [switch]$Force
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# The official Microsoft release of the ONNX Runtime. The digest is the one
# published by Microsoft for that release, and the script refuses to install
# anything that does not match it.
$releases = @{
    '1.23.2' = @{
        Url          = 'https://github.com/microsoft/onnxruntime/releases/download/v1.23.2/onnxruntime-win-x64-1.23.2.zip'
        Sha256       = '0b38df9af21834e41e73d602d90db5cb06dbd1ca618948b8f1d66d607ac9f3cd'
        LibrarySha256 = 'dec964ab1ee36cc9b0ae247d13b376627992fc57dec0454354017ab8fd84f1ea'
        FileName     = 'onnxruntime-win-x64-1.23.2.zip'
        Directory    = 'onnxruntime-win-x64-1.23.2'
    }
}

if (-not $releases.ContainsKey($Version)) {
    $known = ($releases.Keys | Sort-Object) -join ', '
    throw "The ONNX Runtime version '$Version' is not pinned in this script. The known versions are: $known."
}

$release = $releases[$Version]

if (-not $Destination) {
    $Destination = Join-Path $env:RUNNER_TEMP "onnxruntime-$Version"
}

$runtimeDirectory = Join-Path $Destination $release.Directory
$libraryDirectory = Join-Path $runtimeDirectory 'lib'
$library = Join-Path $libraryDirectory 'onnxruntime.dll'

if ((Test-Path -LiteralPath $library) -and -not $Force) {
    Write-Host "The ONNX Runtime $Version is already installed at '$runtimeDirectory'."
} else {
    $work = Join-Path $Destination 'download'
    if (Test-Path -LiteralPath $work) {
        Remove-Item -LiteralPath $work -Recurse -Force
    }
    New-Item -ItemType Directory -Force -Path $work | Out-Null

    $archive = Join-Path $work $release.FileName
    Write-Host "Downloading the ONNX Runtime $Version from $($release.Url)"
    Invoke-WebRequest -Uri $release.Url -OutFile $archive

    $digest = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($digest -ne $release.Sha256) {
        Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
        throw "The downloaded ONNX Runtime does not match the expected digest. Expected '$($release.Sha256)' but got '$digest'. Nothing was installed."
    }
    Write-Host 'The digest of the archive matches the value published with the release.'

    $extracted = Join-Path $work 'extracted'
    Expand-Archive -LiteralPath $archive -DestinationPath $extracted -Force

    $source = Join-Path $extracted $release.Directory
    $extractedLibrary = Join-Path $source 'lib\onnxruntime.dll'
    if (-not (Test-Path -LiteralPath $extractedLibrary)) {
        Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
        throw 'The extracted ONNX Runtime does not contain the expected library. Nothing was installed.'
    }

    # The engine verifies the digest of the library before it loads it, so the
    # library that is installed has to be the one that the engine expects.
    $libraryDigest = (Get-FileHash -LiteralPath $extractedLibrary -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($libraryDigest -ne $release.LibrarySha256) {
        Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
        throw "The extracted ONNX Runtime library does not match the expected digest. Expected '$($release.LibrarySha256)' but got '$libraryDigest'. Nothing was installed."
    }
    Write-Host 'The digest of the library matches the value the engine verifies.'

    if (Test-Path -LiteralPath $runtimeDirectory) {
        Remove-Item -LiteralPath $runtimeDirectory -Recurse -Force
    }
    New-Item -ItemType Directory -Force -Path $Destination | Out-Null
    Copy-Item -LiteralPath $source -Destination $runtimeDirectory -Recurse -Force

    # The symbols file is large and is not needed to run the engine.
    foreach ($name in @('onnxruntime.pdb', 'onnxruntime_providers_shared.pdb')) {
        $path = Join-Path $libraryDirectory $name
        if (Test-Path -LiteralPath $path) {
            Remove-Item -LiteralPath $path -Force
        }
    }

    Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
    Write-Host "The ONNX Runtime $Version is installed at '$runtimeDirectory'."
}

foreach ($name in @('onnxruntime.dll', 'onnxruntime_providers_shared.dll')) {
    $path = Join-Path $libraryDirectory $name
    if (Test-Path -LiteralPath $path) {
        Write-Host ("  {0}  ({1} bytes)" -f $name, (Get-Item -LiteralPath $path).Length)
    }
}

Write-Host ''
Write-Host 'Set the following environment variable before the crate runs:'
Write-Host "  ORT_DYLIB_PATH=$library"
