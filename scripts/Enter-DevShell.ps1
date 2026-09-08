[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"

$physicalRepoRoot = Split-Path -Parent $PSScriptRoot
$asciiRepoRoot = Join-Path $env:USERPROFILE "src\struckout"
$repoRoot = if (Test-Path -LiteralPath $asciiRepoRoot) { $asciiRepoRoot } else { $physicalRepoRoot }
$toolsRoot = Join-Path $repoRoot ".tools"

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

$env:GRADLE_USER_HOME = Join-Path $toolsRoot "gradle"

# mise.toml points DATABASE_URL at the physical (non-ASCII) path. Prefer the
# ASCII junction for the same reason as the Rust directories above.
$devDb = Join-Path $toolsRoot "dev.db"
if (Test-Path -LiteralPath $devDb) {
    $env:DATABASE_URL = "sqlite://$($devDb.Replace('\', '/'))"
}

$pathEntries = @(
    $mingwBinutils,
    $gnuToolchainBin,
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
Write-Host "  Cargo      : $cargoHome"
Write-Host "  Rust target: $env:CARGO_TARGET_DIR"
if ($env:JAVA_HOME) { Write-Host "  Java       : $env:JAVA_HOME" }
if ($env:ANDROID_SDK_ROOT) { Write-Host "  Android SDK: $env:ANDROID_SDK_ROOT" }
if ($env:DATABASE_URL) { Write-Host "  Database   : $env:DATABASE_URL" }
