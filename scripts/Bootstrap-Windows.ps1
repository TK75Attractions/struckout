[CmdletBinding()]
param(
    [switch]$SkipAndroid,
    [switch]$SkipUnity
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

$repoRoot = Split-Path -Parent $PSScriptRoot
$asciiWorkspaceRoot = Join-Path $env:USERPROFILE "src"
$asciiRepoRoot = Join-Path $asciiWorkspaceRoot "struckout"
$toolsRoot = Join-Path $repoRoot ".tools"
$cacheRoot = Join-Path $toolsRoot "cache"
$dotnetRoot = Join-Path $toolsRoot "dotnet"
$miseRoot = Join-Path $toolsRoot "mise"
$miseVersion = "2026.9.1"
$androidSdkRoot = Join-Path $toolsRoot "android-sdk"
$unityRoot = Join-Path $asciiWorkspaceRoot "Unity\6000.5.2f1"

New-Item -ItemType Directory -Force -Path $cacheRoot | Out-Null

# GNU binutils cannot reliably handle the Japanese workspace path on Windows.
# Keep the real files in place and expose ASCII-only junctions for compilation.
New-Item -ItemType Directory -Force -Path $asciiWorkspaceRoot | Out-Null
if (-not (Test-Path -LiteralPath $asciiRepoRoot)) {
    New-Item -ItemType Junction -Path $asciiRepoRoot -Target $repoRoot | Out-Null
}

function Get-ToolFile {
    param(
        [Parameter(Mandatory)] [string]$Uri,
        [Parameter(Mandatory)] [string]$Destination
    )

    if (Test-Path -LiteralPath $Destination) { return }
    Write-Host "Downloading $Uri"
    Invoke-WebRequest -UseBasicParsing -Uri $Uri -OutFile $Destination
}

$dotnetInstall = Join-Path $cacheRoot "dotnet-install.ps1"
Get-ToolFile -Uri "https://dot.net/v1/dotnet-install.ps1" -Destination $dotnetInstall
if (-not (Test-Path -LiteralPath (Join-Path $dotnetRoot "dotnet.exe"))) {
    & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $dotnetInstall -Channel "10.0" -InstallDir $dotnetRoot -NoPath
}

# protoc and the JDK are pinned in mise.toml so that macOS and Linux get the same
# versions from the same file. Install mise itself first, then let it install those.
$mise = (Get-Command mise -ErrorAction SilentlyContinue).Source
if (-not $mise) {
    $mise = Join-Path $miseRoot "bin\mise.exe"
    if (-not (Test-Path -LiteralPath $mise)) {
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $mise) | Out-Null
        Get-ToolFile -Uri "https://github.com/jdx/mise/releases/download/v$miseVersion/mise-v$miseVersion-windows-x64.exe" -Destination $mise
    }
}
& $mise trust $repoRoot
& $mise install
if ($LASTEXITCODE -ne 0) { throw "mise install failed with exit code $LASTEXITCODE" }

if (-not (Test-Path -LiteralPath "C:\msys64\mingw64\bin\gcc.exe")) {
    if (-not (Test-Path -LiteralPath "C:\msys64\usr\bin\bash.exe")) {
        & winget install --id MSYS2.MSYS2 --exact --silent --accept-package-agreements --accept-source-agreements --disable-interactivity
    }
    & "C:\msys64\usr\bin\bash.exe" -lc "pacman -Sy --noconfirm mingw-w64-x86_64-gcc"
}
if (-not (Test-Path -LiteralPath "C:\msys64\mingw64\bin\sqlite3.exe")) {
    & "C:\msys64\usr\bin\bash.exe" -lc "pacman -Sy --noconfirm mingw-w64-x86_64-sqlite3"
}

$devDb = Join-Path $toolsRoot "dev.db"
if (-not (Test-Path -LiteralPath $devDb)) {
    & "C:\msys64\mingw64\bin\sqlite3.exe" $devDb ".read $($repoRoot.Replace('\', '/'))/migrations/20260703054817_initial-migration.sql"
}

if (-not $SkipAndroid) {
    $commandLineToolsZip = Join-Path $cacheRoot "commandlinetools-win-11076708_latest.zip"
    Get-ToolFile -Uri "https://dl.google.com/android/repository/commandlinetools-win-11076708_latest.zip" -Destination $commandLineToolsZip
    $sdkManager = Join-Path $androidSdkRoot "cmdline-tools\latest\bin\sdkmanager.bat"
    if (-not (Test-Path -LiteralPath $sdkManager)) {
        $commandLineToolsExtract = Join-Path $toolsRoot "android-command-line-tools"
        Expand-Archive -LiteralPath $commandLineToolsZip -DestinationPath $commandLineToolsExtract -Force
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent (Split-Path -Parent $sdkManager)) | Out-Null
        Copy-Item -Path (Join-Path $commandLineToolsExtract "cmdline-tools\*") -Destination (Split-Path -Parent (Split-Path -Parent $sdkManager)) -Recurse -Force
    }

    # sdkmanager needs a JDK. Use the one mise.toml pins for the Gradle build.
    $env:JAVA_HOME = (& $mise where java)
    $env:ANDROID_SDK_ROOT = $androidSdkRoot
    $env:ANDROID_HOME = $androidSdkRoot
    $answers = 1..100 | ForEach-Object { "y" }
    $answers | & $sdkManager --sdk_root=$androidSdkRoot --licenses | Out-Null
    & $sdkManager --sdk_root=$androidSdkRoot `
        "platform-tools" `
        "platforms;android-34" `
        "platforms;android-37.0" `
        "build-tools;36.0.0" `
        "build-tools;37.0.0" `
        "ndk;28.2.13676358"
}

if (-not $SkipUnity) {
    $unity = Join-Path $unityRoot "Editor\Unity.exe"
    if (-not (Test-Path -LiteralPath $unity)) {
        $unityInstaller = Join-Path $cacheRoot "UnitySetup64-6000.5.2f1.exe"
        Get-ToolFile -Uri "https://download.unity3d.com/download_unity/eb73d3b415a1/Windows64EditorInstaller/UnitySetup64-6000.5.2f1.exe" -Destination $unityInstaller
        $unityInstall = Start-Process -FilePath $unityInstaller -ArgumentList @("/S", "/D=$unityRoot") -Wait -PassThru -WindowStyle Hidden
        if ($unityInstall.ExitCode -ne 0) { throw "Unity installer failed with exit code $($unityInstall.ExitCode)" }
    }
}


. (Join-Path $PSScriptRoot "Enter-DevShell.ps1")

Write-Host ""
Write-Host "Installed versions:"
& dotnet --version
& protoc --version
if ($env:JAVA_HOME) { & (Join-Path $env:JAVA_HOME "bin\java.exe") -version }
