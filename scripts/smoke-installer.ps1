param(
  [Parameter(Mandatory)]
  [string]$InstallerPath
)

$ErrorActionPreference = "Stop"
$uninstallKey = "HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Everything Modern"
$pipeName = "Everything Service (EverythingModern)"
$engineProcess = $null

function Test-ServicePipe {
  $client = [System.IO.Pipes.NamedPipeClientStream]::new(
    ".",
    $pipeName,
    [System.IO.Pipes.PipeDirection]::InOut,
    [System.IO.Pipes.PipeOptions]::None
  )
  try {
    $client.Connect(100)
    return $client.IsConnected
  } catch {
    return $false
  } finally {
    $client.Dispose()
  }
}

function Wait-ServicePipe([bool]$Available) {
  $deadline = [DateTime]::UtcNow.AddSeconds(15)
  while ([DateTime]::UtcNow -lt $deadline) {
    if ((Test-ServicePipe) -eq $Available) {
      return
    }
    Start-Sleep -Milliseconds 100
  }
  throw "Timed out waiting for the Everything Modern service pipe availability to become $Available."
}

if (Test-Path -LiteralPath $uninstallKey) {
  throw "Everything Modern is already installed on this runner."
}

$installer = Get-Item -LiteralPath $InstallerPath -ErrorAction Stop
$uninstaller = $null

try {
  $install = Start-Process -FilePath $installer.FullName -ArgumentList "/S" -Wait -PassThru
  if ($install.ExitCode -ne 0) {
    throw "The installer failed with exit code $($install.ExitCode)."
  }

  $installed = Get-ItemProperty -LiteralPath $uninstallKey -ErrorAction Stop
  $installDirectory = [System.IO.Path]::GetFullPath($installed.InstallLocation.Trim('"'))
  $programFilesDirectory = [System.IO.Path]::GetFullPath($env:ProgramFiles).TrimEnd('\') + '\'
  if (-not $installDirectory.StartsWith(
    $programFilesDirectory,
    [System.StringComparison]::OrdinalIgnoreCase
  )) {
    throw "Unsafe service installation directory: $installDirectory"
  }

  $uninstaller = Join-Path $installDirectory "uninstall.exe"
  $engineCandidates = @(
    (Join-Path $installDirectory "resources\engine\Everything.exe"),
    (Join-Path $installDirectory "engine\Everything.exe")
  )
  $enginePath = $engineCandidates |
    Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } |
    Select-Object -First 1
  if (-not $enginePath) {
    throw "The installed Everything runtime is missing."
  }

  Wait-ServicePipe $true

  $engineDataDirectory = Join-Path $env:LOCALAPPDATA "EverythingModern\InstallerSmoke"
  New-Item -ItemType Directory -Path $engineDataDirectory -Force | Out-Null
  $engineConfig = Join-Path $engineDataDirectory "Everything.ini"
  $engineDatabase = Join-Path $engineDataDirectory "Everything.db"
  @"
[Everything]
run_in_background=1
show_in_taskbar=0
show_tray_icon=0
ipc_enabled=1
service_pipe_name=$pipeName
"@ | Set-Content -LiteralPath $engineConfig -Encoding utf8

  $engineProcess = Start-Process -FilePath $enginePath -ArgumentList @(
    "-instance", "EverythingModern",
    "-first-instance",
    "-startup",
    "-config", "`"$engineConfig`"",
    "-db", "`"$engineDatabase`""
  ) -PassThru

  $ipcPipe = "Everything IPC (EverythingModern)"
  $ipcDeadline = [DateTime]::UtcNow.AddSeconds(15)
  while (
    -not (Test-Path -LiteralPath "\\.\pipe\$ipcPipe") -and
    [DateTime]::UtcNow -lt $ipcDeadline
  ) {
    Start-Sleep -Milliseconds 100
  }
  if (-not (Test-Path -LiteralPath "\\.\pipe\$ipcPipe")) {
    throw "The private Everything IPC client did not start."
  }

  $upgrade = Start-Process -FilePath $installer.FullName -ArgumentList "/S" -Wait -PassThru
  if ($upgrade.ExitCode -ne 0) {
    throw "The in-place installer upgrade failed with exit code $($upgrade.ExitCode)."
  }
  $engineProcess.WaitForExit(15000)
  if (-not $engineProcess.HasExited) {
    throw "The in-place installer upgrade left the old Everything client running."
  }
  Wait-ServicePipe $true
} finally {
  if ($engineProcess -and -not $engineProcess.HasExited) {
    Stop-Process -Id $engineProcess.Id -Force -ErrorAction SilentlyContinue
  }
  if ($uninstaller -and (Test-Path -LiteralPath $uninstaller -PathType Leaf)) {
    $uninstall = Start-Process -FilePath $uninstaller -ArgumentList "/S" -Wait -PassThru
    if ($uninstall.ExitCode -ne 0) {
      throw "The uninstaller failed with exit code $($uninstall.ExitCode)."
    }
    Wait-ServicePipe $false
    if (Test-Path -LiteralPath $uninstallKey) {
      throw "The uninstaller left its machine-wide registration behind."
    }
  }
}

Write-Host "NSIS install/upgrade/service/uninstall smoke test completed." -ForegroundColor Green
