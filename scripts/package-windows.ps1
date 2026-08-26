param(
    [switch]$SkipInstaller
)

$ErrorActionPreference = "Stop"
$RepoDir = Split-Path -Parent $PSScriptRoot
$DistDir = Join-Path $RepoDir "dist"
$Exe = Join-Path $RepoDir "target\x86_64-pc-windows-msvc\release\formiga.exe"
$Version = if ($env:FORMIGA_VERSION) { $env:FORMIGA_VERSION } else { "0.2.0" }
$Portable = Join-Path $DistDir "Formiga-$Version-windows-x64.zip"
$Installer = Join-Path $DistDir "Formiga-$Version-windows-x64.msi"

function Write-Checksum([string]$Path) {
    $Hash = (Get-FileHash -Algorithm SHA256 $Path).Hash.ToLowerInvariant()
    $Name = Split-Path -Leaf $Path
    Set-Content -NoNewline -Path "$Path.sha256" -Value "$Hash  $Name`n"
}

Push-Location $RepoDir
try {
    rustup target add x86_64-pc-windows-msvc
    cargo build --release -p formiga-desktop --target x86_64-pc-windows-msvc
    New-Item -ItemType Directory -Force $DistDir | Out-Null
    Compress-Archive -Force -Path $Exe -DestinationPath $Portable
    Write-Checksum $Portable

    if (-not $SkipInstaller) {
        if (-not (Get-Command wix -ErrorAction SilentlyContinue)) {
            throw "WiX 4 CLI is required for the MSI. Install it with: dotnet tool install --global wix --version 4.0.5"
        }
        wix build `
            -d "FormigaExe=$Exe" `
            -arch x64 `
            -o $Installer `
            (Join-Path $RepoDir "packaging\windows\Formiga.wxs")
        Write-Checksum $Installer
    }
} finally {
    Pop-Location
}
