param(
  [switch]$SkipChecks
)

$ErrorActionPreference = "Stop"
$projectRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $projectRoot

if (-not (Test-Path "src-tauri\Everything3_x64.dll")) {
  & "$PSScriptRoot\install-everything-sdk.ps1"
}
if (-not (Test-Path "src-tauri\engine\Everything.exe")) {
  & "$PSScriptRoot\install-everything-runtime.ps1"
}
if (-not (Test-Path "Cargo.lock")) {
  cargo generate-lockfile
}

if (-not $SkipChecks) {
  & "$PSScriptRoot\check.ps1" -FixFormatting
}

cargo tauri build
Write-Host "Bundles are available in target\release\bundle." -ForegroundColor Green
