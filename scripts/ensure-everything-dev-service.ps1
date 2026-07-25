param(
  [string]$InstanceName = "EverythingModernDev",
  [string]$SourceExecutable,
  [switch]$Elevated
)

$ErrorActionPreference = "Stop"

if ($InstanceName -notmatch "^[A-Za-z0-9._-]{1,64}$") {
  throw "Invalid Everything instance name. Use 1-64 letters, digits, dots, underscores, or hyphens."
}

$pipeName = "Everything Service ($InstanceName)"
$servicePipe = "\\.\PIPE\$pipeName"

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

if (Test-ServicePipe) {
  Write-Host "Everything development service '$InstanceName' is ready." -ForegroundColor Green
  return
}

if (-not $SourceExecutable) {
  $projectRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
  $SourceExecutable = if ($env:EVERYTHING_ENGINE_EXE) {
    $env:EVERYTHING_ENGINE_EXE
  } else {
    Join-Path $projectRoot "src-tauri\engine\Everything.exe"
  }
}

$source = Get-Item -LiteralPath $SourceExecutable -ErrorAction Stop
if ($source.PSIsContainer) {
  throw "Everything runtime is not a file: $SourceExecutable"
}

$destinationDirectory = Join-Path $env:ProgramFiles "Everything Modern Dev\$InstanceName"
$destination = Join-Path $destinationDirectory "Everything.exe"

if (-not $Elevated) {
  $powershell = (Get-Process -Id $PID).Path
  $arguments = @(
    "-NoProfile",
    "-ExecutionPolicy", "Bypass",
    "-File", "`"$PSCommandPath`"",
    "-Elevated",
    "-InstanceName", "`"$InstanceName`"",
    "-SourceExecutable", "`"$($source.FullName)`""
  )
  $process = Start-Process `
    -FilePath $powershell `
    -ArgumentList $arguments `
    -Verb RunAs `
    -Wait `
    -PassThru `
    -WindowStyle Hidden
  if ($process.ExitCode -ne 0) {
    throw "The elevated Everything service setup failed with exit code $($process.ExitCode)."
  }
} else {
  New-Item -ItemType Directory -Path $destinationDirectory -Force | Out-Null
  Copy-Item -LiteralPath $source.FullName -Destination $destination -Force

  & $destination `
    -instance $InstanceName `
    -install-service `
    -install-service-pipe-name $servicePipe
  if ($LASTEXITCODE -ne 0) {
    throw "Everything service installation failed with exit code $LASTEXITCODE."
  }
}

$deadline = [DateTime]::UtcNow.AddSeconds(10)
while ([DateTime]::UtcNow -lt $deadline) {
  if (Test-ServicePipe) {
    Write-Host "Everything development service '$InstanceName' is ready." -ForegroundColor Green
    return
  }
  Start-Sleep -Milliseconds 100
}

throw "The Everything development service pipe did not become available: $servicePipe"
