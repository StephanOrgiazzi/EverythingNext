$ErrorActionPreference = "Stop"
$projectRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$sdkVersion = "3.0.0.9"
$expectedArchiveSha256 = "124685d35a5f49f3c1e9898853e166215748c893782c6a251f5dde58dacad4fa"
$expectedDllSha256 = "be25b01c73bbf359b50ddf30255133225f93b4bc40a8d208173319373bcdaa5c"
$temp = Join-Path $env:TEMP "everything-modern-sdk3"
$archive = Join-Path $temp "Everything-SDK-$sdkVersion.zip"
$destination = Join-Path $projectRoot "src-tauri\Everything3_x64.dll"

try {
  Remove-Item $temp -Recurse -Force -ErrorAction SilentlyContinue
  New-Item -ItemType Directory -Path $temp | Out-Null
  Invoke-WebRequest "https://www.voidtools.com/Everything-SDK-$sdkVersion.zip" -OutFile $archive

  $archiveHash = (Get-FileHash $archive -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($archiveHash -ne $expectedArchiveSha256) {
    throw "SHA-256 inattendu pour Everything-SDK-$sdkVersion.zip : $archiveHash"
  }

  Expand-Archive $archive -DestinationPath $temp -Force

  $dll = Get-ChildItem $temp -Recurse -Filter "Everything3_x64.dll" | Select-Object -First 1
  if (-not $dll) {
    throw "Everything3_x64.dll introuvable dans l’archive officielle SDK3."
  }

  $dllHash = (Get-FileHash $dll.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($dllHash -ne $expectedDllSha256) {
    throw "SHA-256 inattendu pour Everything3_x64.dll : $dllHash"
  }

  $bytes = [System.IO.File]::ReadAllBytes($dll.FullName)
  if ($bytes.Length -lt 256 -or $bytes[0] -ne 0x4D -or $bytes[1] -ne 0x5A) {
    throw "Le fichier téléchargé n’est pas une DLL PE valide."
  }
  $peOffset = [BitConverter]::ToInt32($bytes, 0x3C)
  if ($peOffset -lt 0 -or $peOffset + 6 -gt $bytes.Length) {
    throw "En-tête PE invalide dans Everything3_x64.dll."
  }
  $machine = [BitConverter]::ToUInt16($bytes, $peOffset + 4)
  if ($machine -ne 0x8664) {
    throw ("Architecture inattendue pour Everything3_x64.dll : 0x{0:X4}" -f $machine)
  }

  Copy-Item $dll.FullName $destination -Force
  Write-Host "Everything SDK3 $sdkVersion x64 vérifié et installé dans src-tauri\Everything3_x64.dll" -ForegroundColor Green
  Write-Host "SHA-256 archive : $archiveHash"
  Write-Host "SHA-256 DLL : $dllHash"
} finally {
  Remove-Item $temp -Recurse -Force -ErrorAction SilentlyContinue
}
