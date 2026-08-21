[CmdletBinding()]
param(
  [Parameter(Mandatory)]
  [ValidatePattern('^\d+\.\d+\.\d+$')]
  [string]$Version,

  [string]$InstallerPath,

  [string]$OutputDirectory = (Join-Path $PSScriptRoot '..\artifacts\winget')
)

$ErrorActionPreference = 'Stop'

$packageIdentifier = 'StephanOrgiazzi.EverythingNext'
$installerName = "Everything Next_${Version}_x64-setup.exe"
$escapedInstallerName = [System.Uri]::EscapeDataString($installerName)
$installerUrl = "https://github.com/StephanOrgiazzi/EverythingNext/releases/download/v$Version/$escapedInstallerName"
$temporaryInstaller = $null

try {
  if ($InstallerPath) {
    $resolvedInstaller = (Resolve-Path -LiteralPath $InstallerPath).Path
  } else {
    $temporaryInstaller = Join-Path ([System.IO.Path]::GetTempPath()) $installerName
    Invoke-WebRequest -Uri $installerUrl -OutFile $temporaryInstaller
    $resolvedInstaller = $temporaryInstaller
  }

  $installerSha256 = (Get-FileHash -LiteralPath $resolvedInstaller -Algorithm SHA256).Hash
  $manifestDirectory = Join-Path $OutputDirectory "manifests\s\StephanOrgiazzi\EverythingNext\$Version"
  New-Item -ItemType Directory -Path $manifestDirectory -Force | Out-Null

  $versionManifest = @"
# yaml-language-server: `$schema=https://aka.ms/winget-manifest.version.1.12.0.schema.json

PackageIdentifier: $packageIdentifier
PackageVersion: $Version
DefaultLocale: en-US
ManifestType: version
ManifestVersion: 1.12.0
"@

  $installerManifest = @"
# yaml-language-server: `$schema=https://aka.ms/winget-manifest.installer.1.12.0.schema.json

PackageIdentifier: $packageIdentifier
PackageVersion: $Version
InstallerType: nullsoft
Scope: machine
UpgradeBehavior: install
RequireExplicitUpgrade: true
ElevationRequirement: elevatesSelf
Installers:
  - Architecture: x64
    InstallerUrl: $installerUrl
    InstallerSha256: $installerSha256
ManifestType: installer
ManifestVersion: 1.12.0
"@

  $localeManifest = @"
# yaml-language-server: `$schema=https://aka.ms/winget-manifest.defaultLocale.1.12.0.schema.json

PackageIdentifier: $packageIdentifier
PackageVersion: $Version
PackageLocale: en-US
Publisher: Stephan Orgiazzi
PublisherUrl: https://github.com/StephanOrgiazzi
PublisherSupportUrl: https://github.com/StephanOrgiazzi/EverythingNext/issues
PackageName: Everything Next
PackageUrl: https://github.com/StephanOrgiazzi/EverythingNext
License: MIT
LicenseUrl: https://github.com/StephanOrgiazzi/EverythingNext/blob/master/LICENSE
ShortDescription: Fast, native-style file search for Windows 11
Description: Everything Next is an open-source desktop client for Everything 1.5, with native query syntax, instant indexing, file actions, and a Windows 11 native-style interface.
Moniker: everything-next
Tags:
  - everything
  - file-search
  - search
  - windows
ManifestType: defaultLocale
ManifestVersion: 1.12.0
"@

  Set-Content -LiteralPath (Join-Path $manifestDirectory "$packageIdentifier.yaml") -Value $versionManifest -Encoding utf8NoBOM
  Set-Content -LiteralPath (Join-Path $manifestDirectory "$packageIdentifier.installer.yaml") -Value $installerManifest -Encoding utf8NoBOM
  Set-Content -LiteralPath (Join-Path $manifestDirectory "$packageIdentifier.locale.en-US.yaml") -Value $localeManifest -Encoding utf8NoBOM

  Write-Host "WinGet manifests written to $manifestDirectory"
  Write-Output $manifestDirectory
} finally {
  if ($temporaryInstaller -and (Test-Path -LiteralPath $temporaryInstaller)) {
    Remove-Item -LiteralPath $temporaryInstaller -Force
  }
}
