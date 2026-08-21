$ErrorActionPreference = "Stop"
$projectRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $projectRoot

if ($env:NO_COLOR) {
  $env:NO_COLOR = "true"
}

function Test-ExplicitFile([string]$VariableName) {
  $value = [Environment]::GetEnvironmentVariable($VariableName)
  return $value -and (Test-Path $value -PathType Leaf)
}

if (-not (Test-Path "src-tauri\Everything3_x64.dll") -and -not (Test-ExplicitFile "EVERYTHING_SDK3_DLL")) {
  & "$PSScriptRoot\install-everything-sdk.ps1"
}
if (-not (Test-Path "src-tauri\engine\Everything.exe") -and -not (Test-ExplicitFile "EVERYTHING_ENGINE_EXE")) {
  & "$PSScriptRoot\install-everything-runtime.ps1"
}
if ([string]::IsNullOrWhiteSpace($env:EVERYTHING_INSTANCE)) {
  $env:EVERYTHING_INSTANCE = "EverythingNextDev"
}
& "$PSScriptRoot\ensure-everything-dev-service.ps1" -InstanceName $env:EVERYTHING_INSTANCE
if (-not (Test-Path "Cargo.lock")) {
  cargo generate-lockfile
}

cargo tauri dev
