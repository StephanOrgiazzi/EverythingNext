param(
  [string]$Version = "1.5.0.1418b"
)

$ErrorActionPreference = "Stop"
$projectRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$temp = Join-Path $env:TEMP "everything-modern-runtime"
$archiveName = "Everything-$Version.x64.zip"
$archive = Join-Path $temp $archiveName
$manifest = Join-Path $temp "Everything-$Version.sha256"
$destinationDirectory = Join-Path $projectRoot "src-tauri\engine"
$destination = Join-Path $destinationDirectory "Everything.exe"

function Assert-X64Pe([string]$Path) {
  $bytes = [System.IO.File]::ReadAllBytes($Path)
  if ($bytes.Length -lt 256 -or $bytes[0] -ne 0x4D -or $bytes[1] -ne 0x5A) {
    throw "$Path is not a valid PE executable."
  }
  $peOffset = [BitConverter]::ToInt32($bytes, 0x3C)
  if ($peOffset -lt 0 -or $peOffset + 6 -gt $bytes.Length) {
    throw "Invalid PE header in $Path."
  }
  $machine = [BitConverter]::ToUInt16($bytes, $peOffset + 4)
  if ($machine -ne 0x8664) {
    throw ("Unexpected architecture for {0}: 0x{1:X4}" -f $Path, $machine)
  }
}

try {
  Remove-Item $temp -Recurse -Force -ErrorAction SilentlyContinue
  New-Item -ItemType Directory -Path $temp | Out-Null
  New-Item -ItemType Directory -Path $destinationDirectory -Force | Out-Null

  Invoke-WebRequest "https://www.voidtools.com/$archiveName" -OutFile $archive
  Invoke-WebRequest "https://www.voidtools.com/Everything-$Version.sha256" -OutFile $manifest

  $manifestLine = Get-Content $manifest | Where-Object { $_ -match [regex]::Escape($archiveName) } | Select-Object -First 1
  if (-not $manifestLine -or $manifestLine -notmatch "([0-9A-Fa-f]{64})") {
    throw "The official fingerprint for $archiveName was not found."
  }
  $expectedHash = $Matches[1].ToLowerInvariant()
  $actualHash = (Get-FileHash $archive -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($actualHash -ne $expectedHash) {
    throw "Invalid SHA-256 fingerprint for $archiveName."
  }

  Expand-Archive $archive -DestinationPath $temp -Force
  $executable = Get-ChildItem $temp -Recurse -Filter "Everything.exe" | Select-Object -First 1
  if (-not $executable) {
    throw "Everything.exe was not found in the official archive."
  }

  Assert-X64Pe $executable.FullName
  $signature = Get-AuthenticodeSignature $executable.FullName
  if ($signature.Status -ne "Valid" -or -not $signature.SignerCertificate) {
    throw "The Authenticode signature for Everything.exe is invalid."
  }

  Copy-Item $executable.FullName $destination -Force
  Write-Host "Everything $Version installed in src-tauri\engine\Everything.exe" -ForegroundColor Green
  Write-Host "SHA-256: $actualHash"
  Write-Host "Signer: $($signature.SignerCertificate.Subject)"
} finally {
  Remove-Item $temp -Recurse -Force -ErrorAction SilentlyContinue
}
