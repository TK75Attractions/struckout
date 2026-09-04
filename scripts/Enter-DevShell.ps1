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
$protocRoot = Join-Path $toolsRoot "protoc-35.1"
$gnuToolchainBin = Join-Path $rustupHome "toolchains\stable-x86_64-pc-windows-gnu\lib\rustlib\x86_64-pc-windows-gnu\bin\self-contained"
$mingwBinutils = "C:\msys64\mingw64\bin"
$localJdk21 = Get-ChildItem -LiteralPath (Join-Path $toolsRoot "jdk-21") -Directory -ErrorAction SilentlyContinue |
    Where-Object { Test-Path -LiteralPath (Join-Path $_.FullName "bin\java.exe") } |
    Select-Object -First 1 -ExpandProperty FullName

$env:CARGO_HOME = $cargoHome
$env:CARGO_TARGET_DIR = Join-Path $rustToolsRoot "target"
$env:RUSTUP_HOME = $rustupHome
$env:DOTNET_ROOT = $dotnetRoot
$env:GRADLE_USER_HOME = Join-Path $toolsRoot "gradle"
$env:PROTOC = Join-Path $protocRoot "bin\protoc.exe"
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
    $dotnetRoot,
    (Join-Path $protocRoot "bin")
)

$javaCandidates = @(@(
    $env:STRUCKOUT_JAVA_HOME,
    $localJdk21,
    "C:\Program Files\Unity\Hub\Editor\6000.5.2f1\Editor\Data\PlaybackEngines\AndroidPlayer\OpenJDK",
    "C:\Program Files\Unity\Hub\Editor\6000.3.9f1\Editor\Data\PlaybackEngines\AndroidPlayer\OpenJDK",
    "C:\Program Files\Android\Android Studio\jbr"
) | Where-Object { $_ -and (Test-Path -LiteralPath (Join-Path $_ "bin\java.exe")) })

if ($javaCandidates.Count -gt 0) {
    $env:JAVA_HOME = $javaCandidates[0]
    $pathEntries += Join-Path $env:JAVA_HOME "bin"
}

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
Write-Host "  Cargo      : $cargoHome"
Write-Host "  Rust target: $env:CARGO_TARGET_DIR"
Write-Host "  .NET       : $dotnetRoot"
Write-Host "  protoc     : $env:PROTOC"
if ($env:JAVA_HOME) { Write-Host "  Java       : $env:JAVA_HOME" }
if ($env:ANDROID_SDK_ROOT) { Write-Host "  Android SDK: $env:ANDROID_SDK_ROOT" }
if ($env:DATABASE_URL) { Write-Host "  Database   : $env:DATABASE_URL" }
