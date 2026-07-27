param(
  [switch]$InstallSdk,
  [switch]$InstallRuntime,
  [switch]$FixFormatting
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true
$projectRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $projectRoot

function Test-ExplicitFile([string]$VariableName) {
  $value = [Environment]::GetEnvironmentVariable($VariableName)
  return $value -and (Test-Path $value -PathType Leaf)
}

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
  throw "Rust/Cargo is missing. Run .\scripts\setup.ps1."
}
if (-not (Get-Command trunk -ErrorAction SilentlyContinue)) {
  throw "Trunk is missing. Run .\scripts\setup.ps1."
}

if (-not (Test-Path "src-tauri\Everything3_x64.dll") -and -not (Test-ExplicitFile "EVERYTHING_SDK3_DLL")) {
  if ($InstallSdk) {
    & "$PSScriptRoot\install-everything-sdk.ps1"
  } else {
    throw "Everything3_x64.dll is missing. Run .\scripts\setup.ps1 or use -InstallSdk."
  }
}

if (-not (Test-Path "src-tauri\engine\Everything.exe") -and -not (Test-ExplicitFile "EVERYTHING_ENGINE_EXE")) {
  if ($InstallRuntime -or $InstallSdk) {
    & "$PSScriptRoot\install-everything-runtime.ps1"
  } else {
    throw "The bundled Everything 1.5 runtime is missing. Run .\scripts\setup.ps1 or use -InstallRuntime."
  }
}

if (-not (Test-Path "src-tauri\engine\THIRD-PARTY-LICENSES.txt")) {
  throw "The bundled Everything runtime license notice is missing."
}

$legacyIdentifiers = @(
  ("Everything" + " Modern"),
  ("Everything" + "Modern"),
  ("everything" + "-modern"),
  ("everything" + "_modern"),
  ("everything" + "Modern"),
  ("everything" + "modern")
)
$PSNativeCommandUseErrorActionPreference = $false
foreach ($legacyIdentifier in $legacyIdentifiers) {
  $matches = & git.exe grep -n -F -- $legacyIdentifier 2>$null
  if ($LASTEXITCODE -eq 0) {
    throw "Legacy product identifier '$legacyIdentifier' remains:`n$($matches -join "`n")"
  }
  if ($LASTEXITCODE -ne 1) {
    throw "git grep failed while checking '$legacyIdentifier'."
  }
}
$PSNativeCommandUseErrorActionPreference = $true

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
cargo check -p everything-next-ui --target wasm32-unknown-unknown --locked
cargo check -p everything-next --locked
trunk build --release

Write-Host "Validation completed." -ForegroundColor Green
