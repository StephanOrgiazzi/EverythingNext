param(
  [switch]$SkipSdk,
  [switch]$SkipFormat
)

$ErrorActionPreference = "Stop"
$projectRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $projectRoot

if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) {
  throw "Rustup est absent. Installez Rust depuis https://rustup.rs puis relancez ce script."
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

if (-not $SkipSdk -and -not (Test-Path "src-tauri\Everything64.dll")) {
  & "$PSScriptRoot\install-everything-sdk.ps1"
}

Write-Host "Toolchain, lockfile et SDK prêts." -ForegroundColor Green
Write-Host "Lancez .\scripts\check.ps1 puis .\scripts\dev.ps1."
