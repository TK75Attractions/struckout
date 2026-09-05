[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"

$physicalRepoRoot = Split-Path -Parent $PSScriptRoot
$asciiRepoRoot = Join-Path $env:USERPROFILE "src\struckout"
$repoRoot = if (Test-Path -LiteralPath $asciiRepoRoot) { $asciiRepoRoot } else { $physicalRepoRoot }
$toolsRoot = Join-Path $repoRoot ".tools"
$rustToolsRoot = Join-Path $env:USERPROFILE "src\.struckout-tools"
$cargoHome = Join-Path $rustToolsRoot "cargo"
$rustupHome = Join-Path $rustToolsRoot "rustup"
$dotnetRoot = Join-Path $toolsRoot "dotnet"
$gnuToolchainBin = Join-Path $rustupHome "toolchains\stable-x86_64-pc-windows-gnu\lib\rustlib\x86_64-pc-windows-gnu\bin\self-contained"
$mingwBinutils = "C:\msys64\mingw64\bin"

# protoc and the JDK come from mise.toml, which also works on macOS and Linux.
# Everything below this point is a Windows-specific workaround that mise cannot
# express, which is why this script still exists.
function Resolve-Mise {
    $onPath = Get-Command mise -ErrorAction SilentlyContinue
    if ($onPath) { return $onPath.Source }

    $bundled = Join-Path $toolsRoot "mise\bin\mise.exe"
    if (Test-Path -LiteralPath $bundled) { return $bundled }

    throw "mise not found. Run scripts\Bootstrap-Windows.ps1 first."
}

$mise = Resolve-Mise
& $mise env -s powershell | Out-String | Invoke-Expression

# Rust lives outside the repository. GNU binutils cannot handle the
# non-ASCII path this repository sits under, so cargo, rustup and the
# build outputs all go to an ASCII-only location.
$env:CARGO_HOME = $cargoHome
$env:RUSTUP_HOME = $rustupHome
$env:CARGO_TARGET_DIR = Join-Path $rustToolsRoot "target"

# The .NET SDK is not in mise.toml: its Windows backend leaves a broken version
# symlink, so scripts\Bootstrap-Windows.ps1 installs it under .tools\dotnet.
$env:DOTNET_ROOT = $dotnetRoot
$env:GRADLE_USER_HOME = Join-Path $toolsRoot "gradle"

# mise.toml points DATABASE_URL at the physical (non-ASCII) path. Prefer the
# ASCII junction for the same reason as the Rust directories above.
$devDb = Join-Path $toolsRoot "dev.db"
if (Test-Path -LiteralPath $devDb) {
    $env:DATABASE_URL = "sqlite://$($devDb.Replace('\', '/'))"
}

if (Test-Path -LiteralPath (Join-Path $mingwBinutils "dlltool.exe")) {
    $env:CC = Join-Path $mingwBinutils "gcc.exe"
    $env:AR = Join-Path $mingwBinutils "ar.exe"
    $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = Join-Path $mingwBinutils "gcc.exe"
    $env:RUSTFLAGS = ((@($env:RUSTFLAGS, "-C", "dlltool=C:\msys64\mingw64\bin\dlltool.exe") |
        Where-Object { $_ }) -join " ").Trim()
}

$pathEntries = @(
    (Join-Path $cargoHome "bin"),
    $mingwBinutils,
    $gnuToolchainBin,
    $dotnetRoot
)

$androidSdkCandidates = @(@(
    $env:ANDROID_SDK_ROOT,
    $env:ANDROID_HOME,
    (Join-Path $toolsRoot "android-sdk"),
    "C:\Program Files (x86)\Android\android-sdk",
    "C:\Users\$env:USERNAME\AppData\Local\Android\Sdk"
) | Where-Object { $_ -and (Test-Path -LiteralPath $_) })

if ($androidSdkCandidates.Count -gt 0) {
    $env:ANDROID_SDK_ROOT = $androidSdkCandidates[0]
    $env:ANDROID_HOME = $androidSdkCandidates[0]
    $pathEntries += Join-Path $env:ANDROID_SDK_ROOT "platform-tools"
}

$existingPath = $env:Path -split ";"
$env:Path = (($pathEntries + $existingPath) | Where-Object { $_ } | Select-Object -Unique) -join ";"

Write-Host "Struckout development environment enabled."
Write-Host "  Repository : $repoRoot"
Write-Host "  mise       : $mise"
Write-Host "  Cargo      : $cargoHome"
Write-Host "  Rust target: $env:CARGO_TARGET_DIR"
Write-Host "  .NET       : $dotnetRoot"
if ($env:JAVA_HOME) { Write-Host "  Java       : $env:JAVA_HOME" }
if ($env:ANDROID_SDK_ROOT) { Write-Host "  Android SDK: $env:ANDROID_SDK_ROOT" }
if ($env:DATABASE_URL) { Write-Host "  Database   : $env:DATABASE_URL" }
