param(
  [switch]$Production
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true
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

if ($Production) {
  & "$PSScriptRoot\check.ps1"
  $env:CARGO_PROFILE_RELEASE_LTO = "thin"
  $env:CARGO_PROFILE_RELEASE_CODEGEN_UNITS = "1"
  $env:CARGO_PROFILE_RELEASE_INCREMENTAL = "false"
} else {
  $env:CARGO_PROFILE_RELEASE_LTO = "false"
  $env:CARGO_PROFILE_RELEASE_CODEGEN_UNITS = "16"
  $env:CARGO_PROFILE_RELEASE_INCREMENTAL = "true"
}

cargo tauri build
if ($LASTEXITCODE -ne 0) {
  throw "Tauri build failed with exit code $LASTEXITCODE."
}

Write-Host "Bundles are available in target\release\bundle." -ForegroundColor Green
