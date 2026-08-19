#Requires -Version 5.1
<#
.SYNOPSIS
  Build WinZSH release artifacts for GitHub Releases + winget.

.DESCRIPTION
  Produces under dist/:
    - WinZSH-Setup-x86_64.exe                 (PRIMARY: download, double-click / run)
    - winzsh-x86_64-pc-windows-msvc.exe       (self-update asset)
    - winzsh-vVERSION-x86_64-pc-windows-msvc.zip (optional archive)
    - SHA256SUMS.txt

.EXAMPLE
  ./scripts/release/package.ps1
  ./scripts/release/package.ps1 -Version 0.1.0
#>
param(
    [string]$Version = "",
    [string]$OutDir = ""
)

$ErrorActionPreference = "Stop"

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "../..")
Set-Location $RepoRoot

if (-not $Version) {
    $cargo = Get-Content -LiteralPath (Join-Path $RepoRoot "Cargo.toml") -Raw
    if ($cargo -match '(?ms)\[workspace\.package\].*?^version\s*=\s*"([^"]+)"') {
        $Version = $Matches[1]
    } else {
        throw "Could not read workspace version from Cargo.toml; pass -Version"
    }
}

if (-not $OutDir) {
    $OutDir = Join-Path $RepoRoot "dist"
}

$triple = "x86_64-pc-windows-msvc"
$setupName = "WinZSH-Setup-x86_64.exe"
$exeAssetName = "winzsh-$triple.exe"
$zipName = "winzsh-v$Version-$triple.zip"

# Cursor/agent sandboxes may inject CARGO_TARGET_DIR pointing at a stale cache.
# Always build into this repo's target/ so dist/ matches the sources you just edited.
$env:CARGO_TARGET_DIR = Join-Path $RepoRoot "target"

Write-Host "==> Building winzsh $Version ($triple)" -ForegroundColor Cyan
Write-Host "    CARGO_TARGET_DIR=$($env:CARGO_TARGET_DIR)"
cargo build -p winzsh --release
if ($LASTEXITCODE -ne 0) {
    throw "cargo build failed with exit $LASTEXITCODE"
}
$built = Join-Path $RepoRoot "target\release\winzsh.exe"
if (-not (Test-Path -LiteralPath $built)) {
    throw "Missing build output: $built"
}

# Guard against shipping an old binary that only runs `status` (no Setup UX).
$probe = [System.IO.File]::ReadAllBytes($built)
$ascii = [System.Text.Encoding]::ASCII.GetString($probe)
if ($ascii -notlike "*WinZSH Setup*") {
    throw @"
Built $built does not contain Setup strings — refusing to package a stale binary.
Delete target\ and rebuild, or unset any sandbox CARGO_TARGET_DIR override.
"@
}
Write-Host ("    Built {0:N0} bytes (expects Setup auto-run + pause/dialog)" -f (Get-Item -LiteralPath $built).Length)

if (Test-Path -LiteralPath $OutDir) {
    Remove-Item -LiteralPath $OutDir -Recurse -Force
}
New-Item -ItemType Directory -Path $OutDir -Force | Out-Null

$setupPath = Join-Path $OutDir $setupName
Copy-Item -LiteralPath $built -Destination $setupPath -Force
Write-Host "    Primary download: $setupName (run / double-click to install)"

$exeAsset = Join-Path $OutDir $exeAssetName
Copy-Item -LiteralPath $built -Destination $exeAsset -Force

$stage = Join-Path $OutDir "stage"
New-Item -ItemType Directory -Path $stage -Force | Out-Null
Copy-Item -LiteralPath $built -Destination (Join-Path $stage "winzsh.exe") -Force
Copy-Item -LiteralPath (Join-Path $RepoRoot "LICENSE") -Destination (Join-Path $stage "LICENSE") -Force

$installTxt = @"
WinZSH $Version
================

Easiest install (recommended):
  1. Download WinZSH-Setup-x86_64.exe from GitHub Releases
  2. Double-click it (or run it)
  3. Open a NEW PowerShell tab
  4. Run: zsh-for-win

From this zip (advanced):
  Put winzsh.exe on PATH, then: winzsh setup -y

Upgrade:
  Download a newer Setup.exe and run it again
  Or: winzsh update   (with [update].github_repo set)
  Or: winget upgrade WinZSH.WinZSH

Docs: https://github.com/winzsh/winzsh
"@
Set-Content -LiteralPath (Join-Path $stage "README-INSTALL.txt") -Value $installTxt -Encoding utf8

$zipPath = Join-Path $OutDir $zipName
if (Test-Path -LiteralPath $zipPath) { Remove-Item -LiteralPath $zipPath -Force }
Compress-Archive -Path (Join-Path $stage "*") -DestinationPath $zipPath -Force
Remove-Item -LiteralPath $stage -Recurse -Force

$sums = Join-Path $OutDir "SHA256SUMS.txt"
$lines = @()
foreach ($f in @($setupPath, $exeAsset, $zipPath)) {
    $hash = (Get-FileHash -LiteralPath $f -Algorithm SHA256).Hash.ToLowerInvariant()
    $name = Split-Path $f -Leaf
    $lines += "$hash  $name"
    Write-Host ("    {0}  {1}" -f $hash.Substring(0, 12), $name)
}
Set-Content -LiteralPath $sums -Value ($lines -join "`n") -Encoding ascii

Write-Host ""
Write-Host "Artifacts in $OutDir" -ForegroundColor Green
Get-ChildItem -LiteralPath $OutDir | ForEach-Object { Write-Host ("  - {0}" -f $_.Name) }
Write-Host ""
Write-Host "Share: WinZSH-Setup-x86_64.exe  (download + run)" -ForegroundColor Cyan
