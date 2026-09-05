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

# The proto generator is a bash script so that CI and non-Windows machines can run
# the same code. On Windows it runs under the bash.exe that ships with Git.
function Resolve-Bash {
    $git = Get-Command git -ErrorAction SilentlyContinue
    if ($git) {
        $fromGit = Join-Path (Split-Path -Parent (Split-Path -Parent $git.Source)) "bin\bash.exe"
        if (Test-Path -LiteralPath $fromGit) { return $fromGit }
    }
    foreach ($candidate in @("C:\Program Files\Git\bin\bash.exe", "C:\Program Files (x86)\Git\bin\bash.exe")) {
        if (Test-Path -LiteralPath $candidate) { return $candidate }
    }
    throw "bash.exe not found. Install Git for Windows, which ships with Git Bash."
}

# Runs first: if the checked-in C# protobuf code no longer matches api/proto,
# every check after this one is testing the wrong protocol.
if (-not $SkipProto) {
    $bash = Resolve-Bash
    Push-Location -LiteralPath $repoRoot
    try {
        # Pass a relative path: bash cannot take the Windows backslash form.
        & $bash "scripts/generate-proto.sh" --check
        if ($LASTEXITCODE -ne 0) { throw "Generated protobuf code is out of sync with api/proto" }
    }
    finally {
        Pop-Location
    }
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
    & dotnet build (Join-Path $repoRoot "sandbox\testTcpCLI\testTcpCLI.csproj")
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
