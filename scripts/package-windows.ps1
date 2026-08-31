param(
    [switch]$SkipInstaller
)

$ErrorActionPreference = "Stop"
$RepoDir = Split-Path -Parent $PSScriptRoot
$DistDir = Join-Path $RepoDir "dist"
$Exe = Join-Path $RepoDir "target\x86_64-pc-windows-msvc\release\formiga.exe"
$Version = if ($env:FORMIGA_VERSION) { $env:FORMIGA_VERSION } else { "0.38.1" }
$Version = $Version.TrimStart("v")
$env:FORMIGA_BUILD_VERSION = $Version
$Portable = Join-Path $DistDir "Formiga-$Version-windows-x64.zip"
$Installer = Join-Path $DistDir "Formiga-$Version-windows-x64.msi"
$PortableDir = Join-Path $DistDir "Formiga-portable"
$Icon = Join-Path $RepoDir "packaging\shared\Formiga.ico"

function Write-Checksum([string]$Path) {
    $Hash = (Get-FileHash -Algorithm SHA256 $Path).Hash.ToLowerInvariant()
    $Name = Split-Path -Leaf $Path
    Set-Content -NoNewline -Path "$Path.sha256" -Value "$Hash  $Name`n"
}

Push-Location $RepoDir
try {
    rustup target add x86_64-pc-windows-msvc
    cargo build --release -p formiga-desktop --target x86_64-pc-windows-msvc
    if ($env:FORMIGA_SIGNTOOL_CERT_SHA1) {
        signtool sign /sha1 $env:FORMIGA_SIGNTOOL_CERT_SHA1 /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 $Exe
    }
    New-Item -ItemType Directory -Force $DistDir | Out-Null
    if (Test-Path $PortableDir) { Remove-Item -Recurse -Force $PortableDir }
    New-Item -ItemType Directory -Force $PortableDir | Out-Null
    Copy-Item $Exe (Join-Path $PortableDir "Formiga.exe")
    Copy-Item (Join-Path $RepoDir "packaging\windows\README.txt") (Join-Path $PortableDir "Read Me.txt")
    Compress-Archive -Force -Path (Join-Path $PortableDir "*") -DestinationPath $Portable
    Write-Checksum $Portable

    if (-not $SkipInstaller) {
        if (-not (Get-Command wix -ErrorAction SilentlyContinue)) {
            throw "WiX 4 CLI is required for the MSI. Install it with: dotnet tool install --global wix --version 4.0.5"
        }
        wix build `
            -d "FormigaExe=$Exe" `
            -d "FormigaIcon=$Icon" `
            -d "FormigaVersion=$Version" `
            -arch x64 `
            -o $Installer `
            (Join-Path $RepoDir "packaging\windows\Formiga.wxs")
        if ($env:FORMIGA_SIGNTOOL_CERT_SHA1) {
            signtool sign /sha1 $env:FORMIGA_SIGNTOOL_CERT_SHA1 /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 $Installer
        }
        Write-Checksum $Installer
    }
} finally {
    Pop-Location
}
