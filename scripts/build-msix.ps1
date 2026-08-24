[CmdletBinding()]
param(
  [string]$IdentityName = $env:MSIX_IDENTITY_NAME,
  [string]$Publisher = $env:MSIX_PUBLISHER,
  [string]$PublisherDisplayName = $env:MSIX_PUBLISHER_DISPLAY_NAME,
  [string]$Version,
  [string]$OutputDirectory = (Join-Path $PSScriptRoot '..\artifacts\msix'),
  [switch]$SkipBuild,
  [switch]$WithoutService
)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
$projectRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $projectRoot

function Require-Value([string]$Name, [string]$Value) {
  if ([string]::IsNullOrWhiteSpace($Value)) {
    throw "$Name is required. Set it as a parameter or repository variable."
  }
  if ($Value.Contains('"') -or $Value.Contains('<') -or $Value.Contains('>')) {
    throw "$Name contains characters that cannot be used in the MSIX manifest."
  }
}

Require-Value 'IdentityName' $IdentityName
Require-Value 'Publisher' $Publisher
Require-Value 'PublisherDisplayName' $PublisherDisplayName

$tauriConfig = Get-Content 'src-tauri\tauri.conf.json' -Raw | ConvertFrom-Json
$manifestVersion = [string]$tauriConfig.version
if ([string]::IsNullOrWhiteSpace($Version)) {
  $Version = $manifestVersion
} elseif ($Version -ne $manifestVersion) {
  throw "Requested MSIX version '$Version' does not match tauri.conf.json version '$manifestVersion'."
}

if ($Version -notmatch '^\d+\.\d+\.\d+$') {
  throw "Version '$Version' must contain exactly three numeric components, such as 0.9.1."
}
$packageVersion = "$Version.0"

if (-not $SkipBuild) {
  # Store packages use the manifest startup task and Microsoft Store updates.
  # Do not compile the registry autostart path or private updater signing material into this build.
  $savedStoreBuild = $env:TAURI_STORE_BUILD
  $savedUpdaterKey = $env:TAURI_UPDATER_PUBLIC_KEY
  $savedSigningKey = $env:TAURI_SIGNING_PRIVATE_KEY
  try {
    $env:TAURI_STORE_BUILD = '1'
    Remove-Item Env:TAURI_UPDATER_PUBLIC_KEY -ErrorAction SilentlyContinue
    Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY -ErrorAction SilentlyContinue

    & cargo tauri build --no-bundle --config src-tauri/tauri.release.conf.json
    if ($LASTEXITCODE -ne 0) {
      throw "Tauri build failed with exit code $LASTEXITCODE."
    }

    & cargo build --release -p everything-next --bin everything-next-autostart
    if ($LASTEXITCODE -ne 0) {
      throw "Autostart launcher build failed with exit code $LASTEXITCODE."
    }
  } finally {
    if ($null -eq $savedStoreBuild) {
      Remove-Item Env:TAURI_STORE_BUILD -ErrorAction SilentlyContinue
    } else {
      $env:TAURI_STORE_BUILD = $savedStoreBuild
    }
    if ($null -eq $savedUpdaterKey) {
      Remove-Item Env:TAURI_UPDATER_PUBLIC_KEY -ErrorAction SilentlyContinue
    } else {
      $env:TAURI_UPDATER_PUBLIC_KEY = $savedUpdaterKey
    }
    if ($null -eq $savedSigningKey) {
      Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY -ErrorAction SilentlyContinue
    } else {
      $env:TAURI_SIGNING_PRIVATE_KEY = $savedSigningKey
    }
  }
}

$makeAppxCommand = Get-Command makeappx.exe -ErrorAction SilentlyContinue | Select-Object -First 1
if ($makeAppxCommand) {
  $makeAppx = $makeAppxCommand.Source
} else {
  $makeAppx = Get-ChildItem `
    -Path (Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin') `
    -Filter makeappx.exe `
    -Recurse `
    -File `
    -ErrorAction SilentlyContinue |
    Where-Object { $_.Directory.Name -eq 'x64' } |
    Sort-Object FullName |
    Select-Object -Last 1 -ExpandProperty FullName
}
if (-not $makeAppx) {
  throw 'makeappx.exe was not found in the installed Windows SDK.'
}

$binary = Join-Path $projectRoot 'target\release\EverythingNext.exe'
$autostartLauncher = Join-Path $projectRoot 'target\release\everything-next-autostart.exe'
$sdk = Join-Path $projectRoot 'src-tauri\Everything3_x64.dll'
$engine = Join-Path $projectRoot 'src-tauri\engine\Everything.exe'
$license = Join-Path $projectRoot 'src-tauri\engine\THIRD-PARTY-LICENSES.txt'
$manifestTemplate = Join-Path $projectRoot 'packaging\msix\AppxManifest.xml.in'

foreach ($requiredFile in @($binary, $autostartLauncher, $sdk, $engine, $license, $manifestTemplate)) {
  if (-not (Test-Path -LiteralPath $requiredFile -PathType Leaf)) {
    throw "Required MSIX input is missing: $requiredFile"
  }
}

if ([System.IO.Path]::IsPathRooted($OutputDirectory)) {
  $outputDirectory = [System.IO.Path]::GetFullPath($OutputDirectory)
} else {
  $outputDirectory = [System.IO.Path]::GetFullPath((Join-Path $projectRoot $OutputDirectory))
}
New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null
$packagePath = Join-Path $outputDirectory "Everything.Next_$Version`_x64.msix"

$stagingRoot = Join-Path ([System.IO.Path]::GetTempPath()) "everything-next-msix-$([guid]::NewGuid().ToString('N'))"
$stagingEngine = Join-Path $stagingRoot 'engine'
$stagingAssets = Join-Path $stagingRoot 'Assets'

try {
  New-Item -ItemType Directory -Path $stagingEngine, $stagingAssets -Force | Out-Null

  Copy-Item -LiteralPath $binary -Destination (Join-Path $stagingRoot 'EverythingNext.exe')
  Copy-Item -LiteralPath $autostartLauncher -Destination (Join-Path $stagingRoot 'EverythingNextAutostart.exe')
  Copy-Item -LiteralPath $sdk -Destination (Join-Path $stagingRoot 'Everything3_x64.dll')
  Copy-Item -LiteralPath $engine -Destination (Join-Path $stagingEngine 'Everything.exe')
  Copy-Item -LiteralPath $license -Destination (Join-Path $stagingEngine 'THIRD-PARTY-LICENSES.txt')

  $icon = Join-Path $projectRoot 'assets\icon.png'
  if (-not (Test-Path -LiteralPath $icon -PathType Leaf)) {
    throw "The MSIX logo source is missing: $icon"
  }
  Add-Type -AssemblyName System.Drawing
  function Write-ScaledPng([string]$SourcePath, [string]$DestinationPath, [int]$Width, [int]$Height) {
    $sourceImage = [System.Drawing.Image]::FromFile($SourcePath)
    $bitmap = [System.Drawing.Bitmap]::new($Width, $Height)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
      $graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
      $graphics.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
      $graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
      $graphics.DrawImage($sourceImage, 0, 0, $Width, $Height)
      $bitmap.Save($DestinationPath, [System.Drawing.Imaging.ImageFormat]::Png)
    } finally {
      $graphics.Dispose()
      $bitmap.Dispose()
      $sourceImage.Dispose()
    }
  }

  Write-ScaledPng $icon (Join-Path $stagingAssets 'StoreLogo.png') 50 50
  Write-ScaledPng $icon (Join-Path $stagingAssets 'Square44x44Logo.png') 44 44
  Write-ScaledPng $icon (Join-Path $stagingAssets 'Square150x150Logo.png') 150 150

  $serviceExtension = ''
  $serviceCapabilities = ''
  if (-not $WithoutService) {
    $serviceExtension = @'
        <desktop6:Extension
          Category="windows.service"
          Executable="engine\Everything.exe"
          EntryPoint="Windows.FullTrustApplication">
          <desktop6:Service
            Name="Everything Service (EverythingNext)"
            StartupType="auto"
            StartAccount="localSystem"
            Arguments="-svc -instance EverythingNext -svc-pipe-name &quot;\\.\PIPE\Everything Service (EverythingNext)&quot;" />
        </desktop6:Extension>
'@
    $serviceCapabilities = @'
    <rescap:Capability Name="packagedServices" />
    <rescap:Capability Name="localSystemServices" />
'@
  }

  $manifest = Get-Content -LiteralPath $manifestTemplate -Raw
  $replacements = @{
    '__IDENTITY_NAME__' = [System.Security.SecurityElement]::Escape($IdentityName)
    '__PUBLISHER__' = [System.Security.SecurityElement]::Escape($Publisher)
    '__PUBLISHER_DISPLAY_NAME__' = [System.Security.SecurityElement]::Escape($PublisherDisplayName)
    '__VERSION__' = $packageVersion
    '__SERVICE_EXTENSION__' = $serviceExtension.TrimEnd()
    '__SERVICE_CAPABILITIES__' = $serviceCapabilities.TrimEnd()
  }
  foreach ($token in $replacements.Keys) {
    $manifest = $manifest.Replace($token, $replacements[$token])
  }
  Set-Content -LiteralPath (Join-Path $stagingRoot 'AppxManifest.xml') -Value $manifest -Encoding utf8NoBOM

  if (Test-Path -LiteralPath $packagePath) {
    Remove-Item -LiteralPath $packagePath -Force
  }
  & $makeAppx pack /d $stagingRoot /p $packagePath /o
  if ($LASTEXITCODE -ne 0) {
    throw "makeappx.exe failed with exit code $LASTEXITCODE."
  }

  Write-Host "Unsigned MSIX package written to $packagePath" -ForegroundColor Green
  Write-Host "Identity: $IdentityName"
  Write-Host "Publisher: $Publisher"
  if ($WithoutService) {
    Write-Host 'Packaged Everything service: disabled (the app starts its own engine process).' -ForegroundColor Yellow
  } else {
    Write-Host 'Packaged Everything service: enabled (the Store submission may require restricted capability approval).' -ForegroundColor Yellow
  }
} finally {
  Remove-Item -LiteralPath $stagingRoot -Recurse -Force -ErrorAction SilentlyContinue
}
