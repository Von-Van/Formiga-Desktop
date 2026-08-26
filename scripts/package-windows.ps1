param(
    [switch]$SkipInstaller
)

$ErrorActionPreference = "Stop"
$RepoDir = Split-Path -Parent $PSScriptRoot
$DistDir = Join-Path $RepoDir "dist"
$Exe = Join-Path $RepoDir "target\x86_64-pc-windows-msvc\release\formiga.exe"

Push-Location $RepoDir
try {
    rustup target add x86_64-pc-windows-msvc
    cargo build --release -p formiga-desktop --target x86_64-pc-windows-msvc
    New-Item -ItemType Directory -Force $DistDir | Out-Null
    Compress-Archive -Force -Path $Exe -DestinationPath (Join-Path $DistDir "Formiga-0.1.0-windows-x64.zip")

    if (-not $SkipInstaller) {
        if (-not (Get-Command wix -ErrorAction SilentlyContinue)) {
            throw "WiX 4 CLI is required for the MSI. Install it with: dotnet tool install --global wix --version 4.0.5"
        }
        wix build `
            -d "FormigaExe=$Exe" `
            -arch x64 `
            -o (Join-Path $DistDir "Formiga-0.1.0-windows-x64.msi") `
            (Join-Path $RepoDir "packaging\windows\Formiga.wxs")
    }
} finally {
    Pop-Location
}
