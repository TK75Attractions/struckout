[CmdletBinding()]
param(
    [switch]$SkipProto,
    [switch]$SkipRust,
    [switch]$SkipDotNet,
    [switch]$SkipAndroid,
    [switch]$SkipUnity
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot "Enter-DevShell.ps1")

# Runs first: if the checked-in C# protobuf code no longer matches api/proto,
# every check after this one is testing the wrong protocol.
if (-not $SkipProto) {
    & (Join-Path $PSScriptRoot "Generate-Proto.ps1") -Check
    if ($LASTEXITCODE -ne 0) { throw "Generated protobuf code is out of sync with api/proto" }
}

if (-not $SkipRust) {
    Push-Location $repoRoot
    try {
        cargo test --workspace
        if ($LASTEXITCODE -ne 0) { throw "Rust tests failed with exit code $LASTEXITCODE" }
    }
    finally {
        Pop-Location
    }
}

if (-not $SkipDotNet) {
    & dotnet build (Join-Path $repoRoot "testTcpCLI\testTcpCLI.csproj")
    if ($LASTEXITCODE -ne 0) { throw ".NET build failed with exit code $LASTEXITCODE" }
}

if (-not $SkipAndroid) {
    Push-Location (Join-Path $repoRoot "struckoutCameraApp")
    try {
        & .\gradlew.bat test --no-daemon --max-workers=1 --console=plain
        if ($LASTEXITCODE -ne 0) { throw "Android unit tests failed with exit code $LASTEXITCODE" }
    }
    finally {
        Pop-Location
    }
}

if (-not $SkipUnity) {
    $unity = @(
        "C:\Users\$env:USERNAME\src\Unity\6000.5.2f1\Editor\Unity.exe",
        "C:\Program Files\Unity\Hub\Editor\6000.5.2f1\Editor\Unity.exe"
    ) | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
    if (-not $unity) {
        throw "Unity 6000.5.2f1 is not installed. Run scripts\Bootstrap-Windows.ps1 first."
    }

    $logPath = Join-Path $repoRoot ".tools\logs\unity-import.log"
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $logPath) | Out-Null
    $unityArgs = @(
        "-batchmode",
        "-nographics",
        "-quit",
        "-projectPath", (Join-Path $repoRoot "projector"),
        "-logFile", $logPath
    )
    $unityProcess = Start-Process -FilePath $unity -ArgumentList $unityArgs -Wait -PassThru -WindowStyle Hidden
    if ($unityProcess.ExitCode -ne 0) { throw "Unity compile/import failed. See $logPath" }
}

Write-Host "All selected dynamic checks completed."
