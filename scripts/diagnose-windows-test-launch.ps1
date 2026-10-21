<#
.SYNOPSIS
    Reports why a compiled Windows test binary refuses to start, and verifies
    that every dynamic library it imports can satisfy its imports.

.DESCRIPTION
    A test binary that was linked but cannot be launched fails with the
    Windows status code STATUS_ENTRYPOINT_NOT_FOUND (0xc0000139) or
    STATUS_DLL_NOT_FOUND (0xc0000135). Cargo only reports the numeric
    status, so the name of the dynamic library and of the missing entry
    point stay invisible in the workflow log.

    This script runs the binary directly, reports the status code, lists the
    dynamic libraries it imports, and then checks, for every imported
    function, that the library the loader would find really exports it. The
    last check is the one that names the culprit: a library that exists but
    does not export a required function is exactly what makes the loader
    refuse the process.

.PARAMETER TestBinary
    The test binary to launch. When omitted, the most recently compiled test
    binary of the crate is used.

.PARAMETER WorkingDirectory
    The directory the binary is launched from.

.EXAMPLE
    ./scripts/diagnose-windows-test-launch.ps1
#>
[CmdletBinding()]
param(
    [string]$TestBinary,
    [string]$WorkingDirectory
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repositoryRoot = Split-Path -Parent $PSScriptRoot
if (-not $WorkingDirectory) {
    $WorkingDirectory = Join-Path $repositoryRoot 'src-tauri'
}

if (-not $TestBinary) {
    $dependencies = Join-Path $WorkingDirectory 'target\debug\deps'
    if (-not (Test-Path -LiteralPath $dependencies)) {
        throw "No compiled test binary directory was found at '$dependencies'."
    }
    $candidate = Get-ChildItem -LiteralPath $dependencies -Filter 'murmure_lib-*.exe' -File |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1
    if (-not $candidate) {
        throw "No compiled test binary was found in '$dependencies'."
    }
    $TestBinary = $candidate.FullName
}

Write-Host "Test binary: $TestBinary"
Write-Host "Working directory: $WorkingDirectory"
Write-Host "Operating system: $([System.Environment]::OSVersion.VersionString)"
Write-Host ''

$dumpbin = Get-Command 'dumpbin.exe' -ErrorAction SilentlyContinue
if (-not $dumpbin) {
    $dumpbin = Get-ChildItem -Path 'C:\Program Files*\Microsoft Visual Studio' -Recurse -Filter 'dumpbin.exe' -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -like '*Hostx64\x64*' } |
        Sort-Object FullName -Descending |
        Select-Object -First 1
}
if (-not $dumpbin) {
    throw 'dumpbin is not available, the import table cannot be read.'
}

# The loader resolves the imports of the executable, then the imports of
# every library it pulls in. A function that is missing from a library that
# the loader does find is what produces STATUS_ENTRYPOINT_NOT_FOUND.
$imports = & $dumpbin.FullName /nologo /imports $TestBinary 2>&1 | Out-String

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;

public static class LoaderProbe
{
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern IntPtr LoadLibraryW(string path);

    [DllImport("kernel32.dll", CharSet = CharSet.Ansi, SetLastError = true)]
    private static extern IntPtr GetProcAddress(IntPtr module, string name);

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern uint GetSystemDirectoryW(System.Text.StringBuilder buffer, uint size);

    public static string SystemDirectory()
    {
        var buffer = new System.Text.StringBuilder(260);
        GetSystemDirectoryW(buffer, (uint)buffer.Capacity);
        return buffer.ToString();
    }

    public static bool Exports(string library, string function)
    {
        IntPtr module = LoadLibraryW(library);
        if (module == IntPtr.Zero)
        {
            return false;
        }
        return GetProcAddress(module, function) != IntPtr.Zero;
    }

    // The loader is asked whether the library can be loaded at all, which
    // resolves the API set contracts to the library that hosts them.
    public static bool CanLoad(string library)
    {
        IntPtr module = LoadLibraryW(library);
        return module != IntPtr.Zero;
    }
}
'@

$systemDirectory = [LoaderProbe]::SystemDirectory()

# The import table is grouped by library. The header of a group is the line
# that carries the library name.
$missing = New-Object System.Collections.Generic.List[string]
$library = ''
$functions = New-Object System.Collections.Generic.List[string]
$checked = 0

function Test-Imports {
    param([string]$Name, [System.Collections.Generic.List[string]]$Entries)

    if (-not $Name -or $Entries.Count -eq 0) {
        return
    }

    # The loader resolves the API set contracts to the library that hosts
    # them, so the loader is the reference and not the file system.
    if (-not [LoaderProbe]::CanLoad($Name)) {
        Write-Host ("  {0}: the loader cannot load this library" -f $Name)
        $missing.Add($Name)
        return
    }

    $absent = New-Object System.Collections.Generic.List[string]
    foreach ($entry in $Entries) {
        $symbol = $entry.Trim()
        if (-not $symbol -or $symbol -like '*Ordinal*' -or $symbol -like 'Hint*') {
            continue
        }
        if (-not [LoaderProbe]::Exports($Name, $symbol)) {
            $absent.Add($symbol)
        }
    }
    if ($absent.Count -eq 0) {
        Write-Host ("  {0}: every imported function is exported" -f $Name)
    } else {
        Write-Host ("  {0}: {1} imported function(s) are NOT exported" -f $Name, $absent.Count)
        foreach ($symbol in $absent) {
            Write-Host ("      $symbol")
        }
        $missing.Add($Name)
    }
}

Write-Host 'Imported dynamic libraries and the functions they must export:'
$current = ''
foreach ($line in ($imports -split "`r?`n")) {
    $trimmed = $line.Trim()
    if ($trimmed -match '^\S+\.dll$') {
        # The header of a library closes the group of the previous one.
        if ($current -and $functions.Count -gt 0) {
            Test-Imports -Name $current -Entries $functions
        }
        $current = $trimmed
        $functions = New-Object System.Collections.Generic.List[string]
        continue
    }
    # Every imported function is written as its ordinal and the name of the
    # symbol, separated by spaces.
    if ($trimmed -match '^[0-9A-F]+ \S+$') {
        $parts = $trimmed -split '\s+'
        $functions.Add($parts[1])
        $checked++
    }
}
if ($current -and $functions.Count -gt 0) {
    Test-Imports -Name $current -Entries $functions
}

Write-Host ''
Write-Host "Imported functions checked: $checked"

$binaryDirectory = Split-Path -Parent $TestBinary
Write-Host ''
Write-Host 'Dynamic libraries next to the binary:'
Get-ChildItem -LiteralPath $binaryDirectory -Filter '*.dll' -File -ErrorAction SilentlyContinue |
    Sort-Object Name |
    ForEach-Object {
        Write-Host ("  {0}  ({1} bytes)" -f $_.Name, $_.Length)
    }

Write-Host ''
# The binary is launched with `--list`, which makes the harness print the
# names of its tests without running any of them. The point of this script is
# to observe the loader, which happens before the tests start, so running the
# whole suite here would execute unrelated tests with no filter and report
# failures that have nothing to do with the launch.
$output = & $TestBinary --list 2>&1
$exitCode = $LASTEXITCODE
Write-Host "Launch exit code: $exitCode"
if ($output) {
    Write-Host 'Loader and runtime output:'
    $output | ForEach-Object { Write-Host "  $_" }
}

$loaderFailure = switch ($exitCode) {
    -1073741511 { 'STATUS_ENTRYPOINT_NOT_FOUND: a dynamic library was found but does not export the required entry point.' }
    -1073741515 { 'STATUS_DLL_NOT_FOUND: a required dynamic library is missing.' }
    -1073741701 { 'STATUS_INVALID_IMAGE_FORMAT: the binary or one of its libraries does not match the architecture.' }
    default { '' }
}
if ($loaderFailure) {
    Write-Host ''
    Write-Host "Diagnosis: $loaderFailure"
}

if ($missing.Count -gt 0) {
    Write-Host ''
    Write-Host 'Libraries that do not export every function the binary imports:'
    foreach ($name in $missing) {
        Write-Host "  $name"
    }
}
