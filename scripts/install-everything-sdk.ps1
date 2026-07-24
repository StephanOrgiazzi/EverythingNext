$ErrorActionPreference = "Stop"
$projectRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$temp = Join-Path $env:TEMP "everything-modern-sdk"
$archive = Join-Path $temp "Everything-SDK.zip"
$destination = Join-Path $projectRoot "src-tauri\Everything64.dll"

try {
  Remove-Item $temp -Recurse -Force -ErrorAction SilentlyContinue
  New-Item -ItemType Directory -Path $temp | Out-Null
  Invoke-WebRequest "https://www.voidtools.com/Everything-SDK.zip" -OutFile $archive
  Expand-Archive $archive -DestinationPath $temp -Force

  $dll = Get-ChildItem $temp -Recurse -Filter "Everything64.dll" | Select-Object -First 1
  if (-not $dll) {
    throw "Everything64.dll introuvable dans l’archive officielle."
  }

  $bytes = [System.IO.File]::ReadAllBytes($dll.FullName)
  if ($bytes.Length -lt 256 -or $bytes[0] -ne 0x4D -or $bytes[1] -ne 0x5A) {
    throw "Le fichier téléchargé n’est pas une DLL PE valide."
  }
  $peOffset = [BitConverter]::ToInt32($bytes, 0x3C)
  if ($peOffset -lt 0 -or $peOffset + 6 -gt $bytes.Length) {
    throw "En-tête PE invalide dans Everything64.dll."
  }
  $machine = [BitConverter]::ToUInt16($bytes, $peOffset + 4)
  if ($machine -ne 0x8664) {
    throw ("Architecture inattendue pour Everything64.dll : 0x{0:X4}" -f $machine)
  }

  Copy-Item $dll.FullName $destination -Force
  $hash = (Get-FileHash $destination -Algorithm SHA256).Hash.ToLowerInvariant()
  Write-Host "SDK x64 installé dans src-tauri\Everything64.dll" -ForegroundColor Green
  Write-Host "SHA-256 : $hash"
} finally {
  Remove-Item $temp -Recurse -Force -ErrorAction SilentlyContinue
}
