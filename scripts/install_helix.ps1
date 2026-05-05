param(
  [string]$Repo = $(if ($env:HELIX_REPO) { $env:HELIX_REPO } else { "TheDarkLightX/Helix-A-Next-Gen-Event-Driven-Multi-Agent-Platform" }),
  [string]$Version = $(if ($env:HELIX_VERSION) { $env:HELIX_VERSION } else { "latest" }),
  [string]$InstallDir = $(if ($env:HELIX_INSTALL_DIR) { $env:HELIX_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA "Helix" })
)

$ErrorActionPreference = "Stop"

if (-not [Environment]::Is64BitOperatingSystem) {
  throw "Only 64-bit Windows release assets are currently published."
}

$asset = "helix-api-windows-x64.tar.gz"
if ($Version -eq "latest") {
  $url = "https://github.com/$Repo/releases/latest/download/$asset"
} else {
  $url = "https://github.com/$Repo/releases/download/$Version/$asset"
}

$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("helix-install-" + [System.Guid]::NewGuid())
New-Item -ItemType Directory -Force -Path $tmp | Out-Null

try {
  $archive = Join-Path $tmp $asset
  Write-Host "[install] downloading $url"
  Invoke-WebRequest -Uri $url -OutFile $archive

  Write-Host "[install] unpacking $asset"
  tar -xzf $archive -C $tmp
  $package = Get-ChildItem -Path $tmp -Directory -Filter "helix-api-*" | Select-Object -First 1
  if (-not $package) {
    throw "Archive did not contain a helix-api package directory."
  }

  New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
  Copy-Item -Recurse -Force -Path (Join-Path $package.FullName "*") -Destination $InstallDir

  $bin = Join-Path $InstallDir "bin\helix-api.exe"
  $helixLauncher = Join-Path $InstallDir "helix.cmd"
  $launcher = Join-Path $InstallDir "helix-api.cmd"
  $launcherBody = @"
@echo off
if "%HELIX_UI_DIST%"=="" set "HELIX_UI_DIST=%~dp0ui\dist"
"%~dp0bin\helix-api.exe" %*
"@
  Set-Content -Path $helixLauncher -Value $launcherBody -Encoding ASCII
  Set-Content -Path $launcher -Value $launcherBody -Encoding ASCII

  Write-Host "[install] installed helix-api to $bin"
  Write-Host "[install] launcher: $helixLauncher"
  Write-Host "[install] compatibility launcher: $launcher"
  Write-Host "[install] run: $helixLauncher"
} finally {
  Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $tmp
}
