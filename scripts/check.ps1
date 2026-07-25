param(
  [switch]$InstallSdk,
  [switch]$FixFormatting
)

$ErrorActionPreference = "Stop"
$projectRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $projectRoot

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
  throw "Rust/Cargo est absent. Exécutez .\scripts\setup.ps1."
}
if (-not (Get-Command trunk -ErrorAction SilentlyContinue)) {
  throw "Trunk est absent. Exécutez .\scripts\setup.ps1."
}

if (-not (Test-Path "src-tauri\Everything3_x64.dll")) {
  if ($InstallSdk) {
    & "$PSScriptRoot\install-everything-sdk.ps1"
  } else {
    throw "Everything3_x64.dll absent. Exécutez .\scripts\setup.ps1 ou relancez avec -InstallSdk."
  }
}

if (-not (Test-Path "Cargo.lock")) {
  cargo generate-lockfile
}

if ($FixFormatting) {
  cargo fmt --all
}

cargo fmt --all -- --check
cargo test -p everything-core --locked
cargo test -p windows-shell --locked
cargo check -p everything-core --locked
cargo check -p windows-shell --locked
cargo check -p everything-modern-ui --target wasm32-unknown-unknown --locked
cargo check -p everything-modern --locked
trunk build --release

Write-Host "Vérifications terminées."
