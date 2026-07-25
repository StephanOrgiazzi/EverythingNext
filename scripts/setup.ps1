param(
  [switch]$SkipSdk,
  [switch]$SkipRuntime,
  [switch]$SkipFormat
)

$ErrorActionPreference = "Stop"
$projectRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $projectRoot

if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) {
  throw "Rustup is missing. Install Rust from https://rustup.rs and run this script again."
}

rustup target add wasm32-unknown-unknown

if (-not (Get-Command trunk -ErrorAction SilentlyContinue)) {
  cargo install trunk --version "0.21.14" --locked
}

if (-not (Get-Command cargo-tauri -ErrorAction SilentlyContinue)) {
  cargo install tauri-cli --version "2.11.0" --locked
}

if (-not (Test-Path "Cargo.lock")) {
  cargo generate-lockfile
}

if (-not $SkipFormat) {
  cargo fmt --all
}

if (-not $SkipSdk -and -not (Test-Path "src-tauri\Everything3_x64.dll")) {
  & "$PSScriptRoot\install-everything-sdk.ps1"
}
if (-not $SkipRuntime -and -not (Test-Path "src-tauri\engine\Everything.exe")) {
  & "$PSScriptRoot\install-everything-runtime.ps1"
}

Write-Host "Toolchain, lockfile, SDK3 and Everything 1.5 runtime are ready." -ForegroundColor Green
Write-Host "Run .\scripts\check.ps1 then .\scripts\dev.ps1."
