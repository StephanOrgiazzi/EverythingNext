$ErrorActionPreference = "Stop"
$projectRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $projectRoot

if (-not (Test-Path "src-tauri\Everything3_x64.dll")) {
  & "$PSScriptRoot\install-everything-sdk.ps1"
}
if (-not (Test-Path "Cargo.lock")) {
  cargo generate-lockfile
}

cargo tauri dev
