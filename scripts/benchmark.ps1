param(
  [string]$Query = "*.rs",
  [int]$Iterations = 40
)

$ErrorActionPreference = "Stop"
$projectRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $projectRoot

if (-not (Test-Path "src-tauri\Everything64.dll")) {
  throw "Everything64.dll absent. Exécutez .\scripts\install-everything-sdk.ps1."
}

$env:EVERYTHING_BENCH_QUERY = $Query
$env:EVERYTHING_BENCH_ITERATIONS = [Math]::Max(5, $Iterations).ToString()
cargo run -p everything-core --example benchmark --release
