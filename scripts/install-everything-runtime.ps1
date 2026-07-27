$ErrorActionPreference = "Stop"
$projectRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$version = "1.5.0.1418b"
$expectedArchiveSha256 = "2240f7055d772983da5ad3a433dbb9250c501ccb3e835451f76d29fe121c1571"
$expectedExecutableSha256 = "be20a73fe5f9269baaf7bf15cfc033c17c11debcd1d46fffa5ac91d44fb7348f"
$expectedSignerThumbprint = "C6B9AE08C3B83981FB1931CDA4A501FCE5F4F92E"
$temp = Join-Path $env:TEMP "everything-next-runtime-$([guid]::NewGuid().ToString('N'))"
$archiveName = "Everything-$version.x64.zip"
$archive = Join-Path $temp $archiveName
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
  $actualHash = (Get-FileHash $archive -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($actualHash -ne $expectedArchiveSha256) {
    throw "Unexpected SHA-256 fingerprint for ${archiveName}: $actualHash"
  }

  Expand-Archive $archive -DestinationPath $temp -Force
  $executable = Get-ChildItem $temp -Recurse -Filter "Everything.exe" | Select-Object -First 1
  if (-not $executable) {
    throw "Everything.exe was not found in the official archive."
  }

  Assert-X64Pe $executable.FullName
  $executableHash = (Get-FileHash $executable.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($executableHash -ne $expectedExecutableSha256) {
    throw "Unexpected SHA-256 fingerprint for Everything.exe: $executableHash"
  }

  $signature = Get-AuthenticodeSignature $executable.FullName
  if ($signature.Status -ne "Valid" -or -not $signature.SignerCertificate) {
    throw "The Authenticode signature for Everything.exe is invalid."
  }
  if ($signature.SignerCertificate.Thumbprint -ne $expectedSignerThumbprint) {
    throw "Everything.exe is signed by an unexpected certificate."
  }

  Copy-Item $executable.FullName $destination -Force
  Write-Host "Everything $version installed in src-tauri\engine\Everything.exe" -ForegroundColor Green
  Write-Host "Archive SHA-256: $actualHash"
  Write-Host "Executable SHA-256: $executableHash"
  Write-Host "Signer: $($signature.SignerCertificate.Subject)"
} finally {
  Remove-Item $temp -Recurse -Force -ErrorAction SilentlyContinue
}
