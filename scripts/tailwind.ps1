param(
  [switch]$Minify
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

$projectRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$tailwindVersion = "4.3.0"
$tailwindSha256 = "E4303D06A337899D426583508F55C143CE89A585F90ED080F482D02FD83093C1"
$toolsDirectory = Join-Path $projectRoot ".tools"
$tailwindExecutable = Join-Path $toolsDirectory "tailwindcss-v$tailwindVersion-windows-x64.exe"
$downloadUri = "https://github.com/tailwindlabs/tailwindcss/releases/download/v$tailwindVersion/tailwindcss-windows-x64.exe"

function Get-Sha256([string]$Path) {
  $stream = [System.IO.File]::OpenRead($Path)
  try {
    $sha256 = [System.Security.Cryptography.SHA256]::Create()
    try {
      return [BitConverter]::ToString($sha256.ComputeHash($stream)).Replace("-", "")
    } finally {
      $sha256.Dispose()
    }
  } finally {
    $stream.Dispose()
  }
}

function Test-TailwindChecksum {
  if (-not (Test-Path -LiteralPath $tailwindExecutable -PathType Leaf)) {
    return $false
  }

  return (Get-Sha256 $tailwindExecutable) -eq $tailwindSha256
}

if (-not (Test-TailwindChecksum)) {
  New-Item -ItemType Directory -Path $toolsDirectory -Force | Out-Null
  $temporaryExecutable = Join-Path $toolsDirectory "tailwindcss.download"

  try {
    Write-Host "Downloading Tailwind CSS CLI v$tailwindVersion..."
    Invoke-WebRequest -Uri $downloadUri -OutFile $temporaryExecutable

    $downloadedHash = Get-Sha256 $temporaryExecutable
    if ($downloadedHash -ne $tailwindSha256) {
      throw "Tailwind CSS CLI checksum mismatch. Expected $tailwindSha256, got $downloadedHash."
    }

    Move-Item -LiteralPath $temporaryExecutable -Destination $tailwindExecutable -Force
  } finally {
    Remove-Item -LiteralPath $temporaryExecutable -Force -ErrorAction SilentlyContinue
  }
}

$arguments = @(
  "--input", (Join-Path $projectRoot "assets\tailwind.input.css"),
  "--output", (Join-Path $projectRoot "assets\tailwind.generated.css")
)
if ($Minify) {
  $arguments += "--minify"
}

& $tailwindExecutable @arguments
if ($LASTEXITCODE -ne 0) {
  throw "Tailwind CSS CLI failed with exit code $LASTEXITCODE."
}
