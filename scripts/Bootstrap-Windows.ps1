[CmdletBinding()]
param(
    [switch]$SkipSlint,
    [switch]$SkipAndroid,
    [switch]$SkipUnity
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

$repoRoot = Split-Path -Parent $PSScriptRoot
$asciiWorkspaceRoot = Join-Path $env:USERPROFILE "src"
$asciiRepoRoot = Join-Path $asciiWorkspaceRoot "struckout"
$asciiSlintRoot = Join-Path $asciiWorkspaceRoot "slint"
$toolsRoot = Join-Path $repoRoot ".tools"
$cacheRoot = Join-Path $toolsRoot "cache"
$rustToolsRoot = Join-Path $asciiWorkspaceRoot ".struckout-tools"
$cargoHome = Join-Path $rustToolsRoot "cargo"
$rustupHome = Join-Path $rustToolsRoot "rustup"
$dotnetRoot = Join-Path $toolsRoot "dotnet"
$protocRoot = Join-Path $toolsRoot "protoc-35.1"
$jdk21Root = Join-Path $toolsRoot "jdk-21"
$androidSdkRoot = Join-Path $toolsRoot "android-sdk"
$unityRoot = Join-Path $asciiWorkspaceRoot "Unity\6000.5.2f1"
$rustToolchain = "stable-x86_64-pc-windows-gnu"

New-Item -ItemType Directory -Force -Path $cacheRoot | Out-Null
New-Item -ItemType Directory -Force -Path $rustToolsRoot | Out-Null

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

$env:CARGO_HOME = $cargoHome
$env:RUSTUP_HOME = $rustupHome
$rustupInit = Join-Path $cacheRoot "rustup-init.exe"
Get-ToolFile -Uri "https://win.rustup.rs/x86_64" -Destination $rustupInit
if (-not (Test-Path -LiteralPath (Join-Path $cargoHome "bin\cargo.exe"))) {
    & $rustupInit -y --no-modify-path --profile minimal --default-toolchain $rustToolchain
}
$rustup = Join-Path $cargoHome "bin\rustup.exe"
& $rustup toolchain install $rustToolchain --profile minimal
& $rustup default $rustToolchain
& $rustup component add rustfmt clippy --toolchain $rustToolchain

$protocZip = Join-Path $cacheRoot "protoc-35.1-win64.zip"
Get-ToolFile -Uri "https://github.com/protocolbuffers/protobuf/releases/download/v35.1/protoc-35.1-win64.zip" -Destination $protocZip
if (-not (Test-Path -LiteralPath (Join-Path $protocRoot "bin\protoc.exe"))) {
    Expand-Archive -LiteralPath $protocZip -DestinationPath $protocRoot -Force
}

$jdk21Zip = Join-Path $cacheRoot "OpenJDK21.zip"
Get-ToolFile -Uri "https://api.adoptium.net/v3/binary/latest/21/ga/windows/x64/jdk/hotspot/normal/eclipse" -Destination $jdk21Zip
$jdk21 = Get-ChildItem -LiteralPath $jdk21Root -Directory -ErrorAction SilentlyContinue |
    Where-Object { Test-Path -LiteralPath (Join-Path $_.FullName "bin\java.exe") } |
    Select-Object -First 1 -ExpandProperty FullName
if (-not $jdk21) {
    Expand-Archive -LiteralPath $jdk21Zip -DestinationPath $jdk21Root -Force
}

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

    $jdk21 = Get-ChildItem -LiteralPath $jdk21Root -Directory |
        Where-Object { Test-Path -LiteralPath (Join-Path $_.FullName "bin\java.exe") } |
        Select-Object -First 1 -ExpandProperty FullName
    $env:JAVA_HOME = $jdk21
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

if (-not $SkipSlint) {
    $slintRoot = Join-Path (Split-Path -Parent $repoRoot) "slint"
    if (-not (Test-Path -LiteralPath (Join-Path $slintRoot "api\rs\build\Cargo.toml"))) {
        Write-Host "Cloning the custom Slint dependency to $slintRoot"
        & git clone --branch feat/add-compiler-config-for-attribute --single-branch https://github.com/taichi765/slint $slintRoot
    }
    if (-not (Test-Path -LiteralPath $asciiSlintRoot)) {
        New-Item -ItemType Junction -Path $asciiSlintRoot -Target $slintRoot | Out-Null
    }
}

. (Join-Path $PSScriptRoot "Enter-DevShell.ps1")

Write-Host ""
Write-Host "Installed versions:"
& cargo --version
& rustc --version
& dotnet --version
& $env:PROTOC --version
if ($env:JAVA_HOME) { & (Join-Path $env:JAVA_HOME "bin\java.exe") -version }
